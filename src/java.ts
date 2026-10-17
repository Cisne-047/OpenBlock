import { execFile } from "node:child_process";
import { chmod, mkdir, readdir } from "node:fs/promises";
import os from "node:os";
import { dirname, join } from "node:path";
import { downloadTo } from "./net/http.js";
import { promisify } from "node:util";

const exec = promisify(execFile);

/**
 * Gestión de Java: usa el JDK del sistema si cubre la versión que pide MC;
 * si no, baja Temurin (Adoptium) a <serverDir>/runtime/jdkN y queda
 * encapsulado: el server nunca toca el Java del sistema.
 */

export interface JavaRuntime {
  path: string;
  major: number;
  managed: boolean;
}

export async function javaMajorOf(javaPath: string): Promise<number | null> {
  try {
    const { stdout, stderr } = await exec(javaPath, ["-version"], { timeout: 15000 });
    const out = stderr || stdout;
    const m = /version "(\d+)(?:\.(\d+))?/.exec(out);
    if (!m) return null;
    const maj = Number(m[1]);
    // Java 8 reporta "1.8.0_xxx"
    return maj === 1 ? Number(m[2]) : maj;
  } catch {
    return null;
  }
}

async function findSystemJava(major: number): Promise<JavaRuntime | null> {
  const candidates: string[] = [];
  if (process.platform === "win32") {
    for (const base of ["C:\\Program Files\\Java", "C:\\Program Files\\Eclipse Adoptium", "C:\\Program Files\\Microsoft", "C:\\Program Files\\Zulu"]) {
      try {
        for (const dir of await readdir(base)) {
          candidates.push(join(base, dir, "bin", "java.exe"));
        }
      } catch {
        // base inexistente
      }
    }
  } else {
    try {
      for (const dir of await readdir("/usr/lib/jvm")) {
        candidates.push(join("/usr/lib/jvm", dir, "bin", "java"));
      }
    } catch {
      // sin /usr/lib/jvm
    }
  }
  // el PATH siempre es el último recurso
  candidates.push(process.platform === "win32" ? "java.exe" : "java");
  for (const c of candidates) {
    if ((await javaMajorOf(c)) === major) return { path: c, major, managed: false };
  }
  return null;
}

export async function ensureJava(major: number, serverDir: string): Promise<JavaRuntime> {
  const system = await findSystemJava(major);
  if (system) return system;
  return downloadJava(major, serverDir);
}

export async function downloadJava(major: number, serverDir: string): Promise<JavaRuntime> {
  const osName = process.platform === "win32" ? "windows" : "linux";
  const arch = os.arch() === "arm64" ? "aarch64" : "x64";
  const res = await fetch(
    `https://api.adoptium.net/v3/assets/latest/${major}/hotspot?architecture=${arch}&image_type=jdk&os=${osName}&vendor=eclipse`
  );
  if (!res.ok) throw new Error(`Adoptium HTTP ${res.status} buscando JDK ${major}`);
  const assets = (await res.json()) as Array<{
    binary: { package: { link: string; name: string } };
  }>;
  const pkg = assets[0]?.binary?.package;
  if (!pkg) throw new Error(`Adoptium sin paquete JDK ${major} para ${osName}/${arch}`);
  const runtimeDir = join(serverDir, "runtime");
  const extractDir = join(runtimeDir, `jdk${major}`);
  await mkdir(extractDir, { recursive: true });
  const archive = join(runtimeDir, pkg.name);
  await downloadTo(pkg.link, archive);
  // tar de Windows (bsdtar, System32) extrae zip; el tar de Git Bash (GNU) interpreta
  // "C:" como host remoto ("Cannot connect to C: resolve failed") → ruta absoluta
  const tarBin =
    process.platform === "win32"
      ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe")
      : "tar";
  await exec(tarBin, process.platform === "win32" ? ["-xf", archive, "-C", extractDir] : ["-xzf", archive, "-C", extractDir], {
    timeout: 300000,
  });
  // Adoptium mete todo en jdk-21.0.x+y: buscamos la raíz
  let root = extractDir;
  const inner = await readdir(extractDir);
  if (inner.length === 1) root = join(extractDir, inner[0]);
  const javaBin = process.platform === "win32" ? join(root, "bin", "java.exe") : join(root, "bin", "java");
  if (process.platform !== "win32") {
    await chmod(javaBin, 0o755).catch(() => {});
    await chmod(join(root, "bin", "keytool"), 0o755).catch(() => {});
  }
  const majorCheck = await javaMajorOf(javaBin);
  if (majorCheck !== major)
    throw new Error(`JDK extraído no reporta versión ${major}: ${javaBin} -> ${majorCheck}`);
  return { path: javaBin, major, managed: true };
}

/** bin dir del runtime (para PATH de los scripts de arranque) */
export function javaBinDir(runtime: JavaRuntime): string {
  return dirname(runtime.path);
}
