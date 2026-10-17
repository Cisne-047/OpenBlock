import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync } from "node:fs";
import { appendFile, copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { javaMajorFor } from "./catalog";
import { ensureJava } from "./java";
import { downloadTo, fmtBytes } from "./net/http";
import { buildDownloadPlan, type DownloadPlan } from "./plan";
import { withRcon } from "./rcon";
import { achievement, c, progressBar } from "./ui";
import { generateProject, validate } from "./generators";
import { writeProject } from "./output";
import type { ServerConfig } from "./types";

/**
 * apply: convierte config + plan en un server REAL en disco. Deja start.sh /
 * start.bat listos para ejecutar, Java encapsulado en runtime/, plugins/mods
 * descargados y (opcional) pregeneración lanzada por RCON.
 */

export interface ApplyOptions {
  cores: number;
  dryRun?: boolean;
  /** fuerza re-descarga aunque el fichero exista */
  force?: boolean;
  /** lanza el server y pregenera con Chunky al terminar */
  pregen?: boolean;
  onLog?: (line: string) => void;
}

export function ensureRconPassword(cfg: ServerConfig): void {
  if (cfg.rcon.enabled && !cfg.rcon.password) {
    cfg.rcon.password = randomBytes(8).toString("hex");
  }
}

export async function applyConfig(cfg: ServerConfig, opts: ApplyOptions): Promise<void> {
  const errors = validate(cfg);
  if (errors.length) throw new Error(`Config inválida:\n  - ${errors.join("\n  - ")}`);
  const dir = resolve(cfg.serverDir);
  const log = opts.onLog ?? ((s: string) => console.log(s));
  const plan = await buildDownloadPlan(cfg);

  log(c.bold(`\n== Plan == ${plan.engineResolved}`));
  for (const e of plan.entries) log(`  ${c.dim(`[${e.kind}]`)} ${e.label}`);
  if (plan.unresolvedCf === -1)
    log(
      c.yellow(
        "  ⚠ Modpack CurseForge: instala el modpack con el launcher (o define OPENBLOCK_CF_KEY) y copia mods/ + overrides al server."
      )
    );
  else if (plan.unresolvedCf)
    log(c.yellow(`  ⚠ ${plan.unresolvedCf} items del catálogo sin build para ${cfg.mcVersion}: se saltan.`));

  if (opts.dryRun) {
    log(c.yellow("\nDry-run: nada se ha escrito."));
    return;
  }

  mkdirSync(dir, { recursive: true });
  writeProject(dir, generateProject(cfg, plan, opts.cores));
  log(c.green(`\n✓ Proyecto escrito en ${dir}`));

  // 1) Java correcto para esta versión de MC
  const java = await ensureJava(javaMajorFor(cfg.mcVersion), dir);
  if (java.managed) log(c.green(`✓ JDK ${java.major} (Temurin) instalado en runtime/`));
  else log(c.dim(`✓ Java del sistema usado: ${java.path} (v${java.major})`));
  await mkdir(join(dir, "runtime"), { recursive: true });
  await writeFile(join(dir, "runtime", "java.path"), java.path + "\n", "utf8");

  // 2) Descargas engine + plugins/mods + modpack + custom
  for (const e of plan.entries) {
    const dest = join(dir, e.dest || basename(new URL(e.url).pathname));
    if (existsSync(dest) && !opts.force) {
      const st = await stat(dest);
      if (st.size > 0) {
        log(c.gray(`• ${e.dest || basename(dest)} ya existe, se salta`));
        continue;
      }
    }
    await mkdir(dirname(dest), { recursive: true });
    log(`↓ ${e.label} ${c.gray("...")}`);
    await downloadTo(e.url, dest, {
      sha256: e.sha256,
      sha512: e.sha512,
      onProgress: (done, total) => {
        if (total) process.stdout.write(`\r  ${progressBar(done, total)} ${fmtBytes(done)} / ${fmtBytes(total)}   `);
      },
    });
    process.stdout.write("\n");
  }

  // 2b) ficheros locales del usuario (custom items con path)
  for (const lc of plan.localCopies ?? []) {
    if (!existsSync(lc.src)) {
      log(c.yellow(`⚠ Custom local no existe: ${lc.src}`));
      continue;
    }
    await mkdir(dirname(join(dir, lc.dest)), { recursive: true });
    await copyFile(lc.src, join(dir, lc.dest));
    log(c.green(`✓ Copiado ${basename(lc.src)} a ${lc.dest}`));
  }

  // 3) installer de forge/neoforge (descarga librerías del loader)
  if (plan.installer) {
    const libsDir = join(dir, "libraries");
    if (existsSync(libsDir) && !opts.force) {
      log(c.dim("• installer ya ejecutado (libraries/ presente)"));
    } else {
      log(c.bold(`== Ejecutando installer de ${plan.installer.engine} (tarda) ==`));
      await runJava(java.path, ["-jar", plan.installer.jar, "--installServer"], dir, log);
    }
  }

  achievement("¡Logro conseguido!", "Server forjado y listo para arrancar");
  log(
    c.green(
      `✓ Server instalado. Arranca con: ${process.platform === "win32" ? "start.bat" : "./start.sh"} (en ${dir})`
    )
  );

  // 4) pregeneración opcional vía RCON + Chunky
  if (opts.pregen && cfg.pregenRadius > 0) {
    await pregenerate(cfg, dir, java.path, plan, log);
  }
}

async function runJava(javaPath: string, args: string[], cwd: string, log: (s: string) => void): Promise<void> {
  await new Promise<void>((res, rej) => {
    const p = spawn(javaPath, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    const pump = (buf: Buffer) => {
      for (const line of buf.toString().split("\n")) if (line.trim()) log(c.dim(`  ${line.trim()}`));
    };
    p.stdout.on("data", pump);
    p.stderr.on("data", pump);
    p.on("error", rej);
    p.on("exit", (code) => (code === 0 ? res() : rej(new Error(`Installer salió con código ${code}`))));
  });
}

/** Arranca el server en background, espera "Done", y lanza Chunky por RCON */
export async function pregenerate(
  cfg: ServerConfig,
  dir: string,
  javaPath: string,
  plan: DownloadPlan,
  log: (s: string) => void
): Promise<void> {
  if (!cfg.rcon.enabled) throw new Error("Pregeneración requiere RCON (cfg.rcon.enabled)");
  const logPath = join(dir, "logs", "latest.log");

  log(c.bold("\n== Pregeneración: arrancando server en background =="));
  let jvmArgs: string[] = [];
  try {
    jvmArgs = (await readFile(join(dir, "jvm-args.txt"), "utf8")).split(/\s+/).filter(Boolean);
  } catch {
    // sin jvm-args.txt: defaults de la JVM
  }
  if (plan.installer) {
    // forge/neoforge: usa su propio launcher
    const runner = process.platform === "win32" ? "run.bat" : "run.sh";
    if (!existsSync(join(dir, runner))) throw new Error("Installer pendiente: no hay run.sh/run.bat");
    spawn(
      process.platform === "win32" ? "cmd.exe" : "bash",
      process.platform === "win32" ? ["/c", runner, "nogui"] : [runner, "nogui"],
      { cwd: dir, detached: true, stdio: "ignore" }
    ).unref();
  } else {
    const guiFlag = cfg.engine === "vanilla" ? "nogui" : "--nogui";
    spawn(javaPath, [...jvmArgs, "-jar", plan.engineFilename, guiFlag], {
      cwd: dir,
      detached: true,
      stdio: "ignore",
    }).unref();
  }

  log("Esperando arranque (mira logs/latest.log)...");
  const deadline = Date.now() + 20 * 60_000;
  for (;;) {
    if (Date.now() > deadline) throw new Error("Timeout esperando 'Done' del server");
    await new Promise((r) => setTimeout(r, 3000));
    if (existsSync(logPath)) {
      const tail = (await readFile(logPath, "utf8")).slice(-4000);
      if (/Done \([0-9.]+s\)!/.test(tail)) break;
      if (/Failed to start the minecraft server|Exception in thread "main"/.test(tail))
        throw new Error("El server crasheó al arrancar; revisa logs/latest.log");
    }
  }
  log(c.green("✓ Server arriba."));

  await withRcon("127.0.0.1", cfg.rcon.port, cfg.rcon.password, async (rcon) => {
    log(`chunky radius ${cfg.pregenRadius}`);
    console.log(c.dim(`  ${await rcon.command(`chunky radius ${cfg.pregenRadius}`)}`));
    log("chunky start");
    console.log(c.dim(`  ${await rcon.command("chunky start")}`));
    try {
      console.log(c.dim(`  TPS: ${await rcon.command("spark tps")}`));
    } catch {
      // spark ausente o comando no disponible
    }
  });
  log(
    c.green(
      `\n✓ Pregeneración lanzada (${cfg.pregenRadius} de radio). Sigue el progreso en la consola del server.`
    )
  );
}

/** apagado limpio vía RCON (stop) */
export async function stopServer(cfg: ServerConfig): Promise<void> {
  await withRcon("127.0.0.1", cfg.rcon.port, cfg.rcon.password, (rcon) => rcon.command("stop"));
}

export async function runBackup(dir: string): Promise<void> {
  const script = process.platform === "win32" ? "backup.ps1" : "backup.sh";
  const path = join(resolve(dir), script);
  if (!existsSync(path)) throw new Error(`No existe ${path}: instala el server primero`);
  await new Promise<void>((res, rej) => {
    const p = spawn(
      process.platform === "win32" ? "powershell.exe" : "bash",
      process.platform === "win32" ? ["-NoProfile", "-File", path] : [path],
      { stdio: "inherit" }
    );
    p.on("exit", (code) => (code === 0 ? res() : rej(new Error(`Backup salió con código ${code}`))));
  });
}

/** copia overrides de un modpack CurseForge exportado (mods requieren API key) */
export async function extractCfOverrides(zipPath: string, dir: string, log: (s: string) => void): Promise<void> {
  const JSZip = (await import("jszip")).default;
  const zip = await JSZip.loadAsync(await readFile(zipPath));
  const prefix = "overrides/";
  let count = 0;
  for (const entry of Object.keys(zip.files)) {
    const f = zip.files[entry];
    if (!entry.startsWith(prefix) || f.dir) continue;
    const dest = join(dir, entry.slice(prefix.length));
    await mkdir(dirname(dest), { recursive: true });
    await writeFile(dest, await f.async("nodebuffer"));
    count++;
  }
  log(c.green(`✓ ${count} ficheros de overrides copiados del modpack`));
  await appendFile(join(dir, "CF-PENDING.txt"), "Los mods del manifest requieren OPENBLOCK_CF_KEY o instalación manual.\n").catch(() => {});
}
