import { execFile } from "node:child_process";
import { readFile, stat } from "node:fs/promises";
import os from "node:os";
import { promisify } from "node:util";
import type { HardwareProfile } from "../types";

const exec = promisify(execFile);
const TIMEOUT = 8000;

async function ps(script: string): Promise<string> {
  const { stdout } = await exec("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
    timeout: TIMEOUT,
  });
  return stdout.trim();
}

async function sh(cmd: string, args: string[]): Promise<string> {
  const { stdout } = await exec(cmd, args, { timeout: TIMEOUT });
  return stdout.trim();
}

export async function detectHardware(): Promise<HardwareProfile> {
  const isWin = process.platform === "win32";
  const cpus = os.cpus();
  const totalMB = Math.round(os.totalmem() / 1048576);
  const [swapMB, diskKind, diskFreeMB, virtualized] = isWin
    ? await Promise.all([
        winSwap(),
        winDiskKind(),
        winDiskFree(),
        winVirtualized(),
      ])
    : await Promise.all([nixSwap(), nixDiskKind(), nixDiskFree(), nixVirtualized()]);

  return {
    os: isWin ? "windows" : "linux",
    osVersion: os.release(),
    arch: os.arch(),
    hostname: os.hostname(),
    cpuModel: cpus[0]?.model?.trim() ?? "desconocido",
    cores: cpus.length,
    ramTotalMB: totalMB,
    ramFreeMB: Math.round(os.freemem() / 1048576),
    swapTotalMB: swapMB,
    diskKind,
    diskFreeMB,
    virtualized,
  };
}

async function winSwap(): Promise<number> {
  try {
    const out = await ps(
      "(Get-CimInstance Win32_PageFileUsage | Measure-Object -Sum AllocatedBaseSize).Sum"
    );
    return Number(out) || 0;
  } catch {
    return 0;
  }
}

async function winDiskKind(): Promise<HardwareProfile["diskKind"]> {
  try {
    // BusType distingue NVMe real; MediaType solo dice SSD/HDD (o "Unspecified" en VMs)
    const bus = await ps(
      "Get-PhysicalDisk | Sort-Object Size -Descending | Select-Object -First 1 -ExpandProperty BusType"
    );
    if (bus.toLowerCase().includes("nvme")) return "nvme";
    const out = await ps(
      "Get-PhysicalDisk | Sort-Object Size -Descending | Select-Object -First 1 -ExpandProperty MediaType"
    );
    const t = out.toLowerCase();
    if (t.includes("ssd")) return "ssd";
    if (t.includes("hdd")) return "hdd";
  } catch {
    // VMs sin Get-PhysicalDisk
  }
  return "unknown";
}

async function winDiskFree(): Promise<number> {
  try {
    const out = await ps("(Get-PSDrive -Name (Get-Location).Drive.Name).Free / 1MB");
    // locales con coma decimal (es-ES): normaliza antes de Number
    return Math.round(Number(String(out).replace(",", "."))) || 0;
  } catch {
    return 0;
  }
}

async function winVirtualized(): Promise<boolean> {
  try {
    const model = await ps("(Get-CimInstance Win32_ComputerSystem).Model");
    return /virtual|vmware|kvm|qemu|virtualbox|xen/i.test(model);
  } catch {
    return false;
  }
}

async function nixSwap(): Promise<number> {
  try {
    const info = await readFile("/proc/meminfo", "utf8");
    const m = /^SwapTotal:\s+(\d+) kB/m.exec(info);
    return m ? Math.round(Number(m[1]) / 1024) : 0;
  } catch {
    return 0;
  }
}

async function nixDiskKind(): Promise<HardwareProfile["diskKind"]> {
  try {
    const out = await sh("lsblk", ["-dn", "-o", "NAME,ROTA"]);
    // primera línea = primer disco por orden del kernel
    const first = out.split("\n")[0] ?? "";
    const rota = /rota$/m.test(first) ? first.trim().split(/\s+/).pop() : "1";
    const name = first.trim().split(/\s+/)[0] ?? "";
    if (name.startsWith("nvme")) return "nvme";
    if (rota === "0") return "ssd";
    if (rota === "1") return "hdd";
  } catch {
    // sin lsblk
  }
  return "unknown";
}

async function nixDiskFree(): Promise<number> {
  try {
    const out = await sh("df", ["-Pm", process.cwd()]);
    const line = out.split("\n")[1] ?? "";
    return Number(line.split(/\s+/)[3]) || 0;
  } catch {
    return 0;
  }
}

async function nixVirtualized(): Promise<boolean> {
  try {
    const info = await readFile("/proc/cpuinfo", "utf8");
    if (/hypervisor/i.test(info)) return true;
    await stat("/proc/xen");
    return true;
  } catch {
    return false;
  }
}
