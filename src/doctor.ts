import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import { join, resolve } from "node:path";
import { javaMajorFor } from "./catalog.js";
import { ensureJava, javaMajorOf } from "./java.js";
import { latestVersion, primaryFile } from "./api/modrinth.js";
import { c } from "./ui.js";
import type { ServerConfig } from "./types.js";

/**
 * Doctor: la "pequeña IA" de OpenBlock. Hace un dry-run real del server
 * (arranca, vigila el log, apaga), clasifica cada línea de error/warning con
 * reglas data-driven, aplica los fixes seguros automáticamente y pregunta
 * antes de tocar nada considerable. Score de salud al final.
 */

export type Severity = "error" | "warning" | "info";

export interface Fix {
  code: string;
  description: string;
  /** true = pregunta al usuario antes de aplicar */
  considerable: boolean;
  apply: () => Promise<void>;
}

export interface Finding {
  severity: Severity;
  code: string;
  message: string;
  source: string;
  fix?: Fix;
}

export interface DoctorReport {
  findings: Finding[];
  applied: string[];
  pending: string[];
  score: number;
  grade: string;
  bootTested: boolean;
  bootOk: boolean;
}

interface LogRule {
  code: string;
  severity: Severity;
  pattern: RegExp;
  message: string;
  /** se comprueba sobre el agregado, no por línea */
  minCount?: number;
  fix?: (m: RegExpMatchArray, ctx: DoctorContext) => Fix | undefined;
}

interface StaticRule {
  code: string;
  check: (ctx: DoctorContext) => Promise<Finding | null>;
}

export interface DoctorContext {
  cfg: ServerConfig;
  dir: string;
  logLines: string[];
  bootTested: boolean;
  bootOk: boolean;
}

// ---------- reglas sobre el log del boot ----------

/**
 * Evalúa las reglas de log sobre lo capturado (`runDoctor`, paso 3).
 *
 * Está separada de `runDoctor` para poder compararla con el port: es pura —texto
 * de log y la config que leen las descripciones— y es donde vive el detalle que
 * un puerto se salta: se usa **el primer match** de cada regla, no el último ni la
 * suma, así que dos líneas que disparen `JAVA_VERSION` dan un hallazgo y el número
 * de la primera.
 *
 * El `minCount` de `LogRule` está declarado y **no se usa**: una sola línea que
 * case ya dispara el hallazgo. Se reproduce igual.
 */
export function evaluateLogRules(lines: string[], ctx: DoctorContext): Finding[] {
  const out: Finding[] = [];
  for (const rule of LOG_RULES) {
    const matches = lines.map((l) => l.match(rule.pattern)).filter(Boolean) as RegExpMatchArray[];
    if (matches.length === 0) continue;
    out.push({
      severity: rule.severity,
      code: rule.code,
      source: "log",
      message: rule.message,
      fix: rule.fix ? rule.fix(matches[0], ctx) : undefined,
    });
  }
  return out;
}

export const LOG_RULES: LogRule[] = [
  {
    code: "JAVA_VERSION",
    severity: "error",
    pattern: /requires running the server with Java (\d+)/,
    message: "Versión de Java incorrecta para esta versión de MC.",
    fix: (m, ctx) => ({
      code: "install-java",
      description: `Descargar Temurin JDK ${m[1]} y reescribir runtime/java.path`,
      considerable: false,
      apply: async () => {
        const major = Number(m[1]) as 8 | 17 | 21 | 25;
        const runtime = await ensureJava(major, ctx.dir);
        await mkdir(join(ctx.dir, "runtime"), { recursive: true });
        await writeFile(join(ctx.dir, "runtime", "java.path"), runtime.path + "\n", "utf8");
      },
    }),
  },
  {
    code: "EULA_NOT_ACCEPTED",
    severity: "error",
    pattern: /You need to agree to the EULA/,
    message: "EULA sin aceptar: el server no arranca.",
    fix: (_m, ctx) => ({
      code: "accept-eula",
      description: "Aceptar EULA (eula=true)",
      considerable: false,
      apply: async () => {
        await writeFile(join(ctx.dir, "eula.txt"), "eula=true\n", "utf8");
      },
    }),
  },
  {
    code: "PORT_IN_USE",
    severity: "error",
    pattern: /Failed to bind to port|Address already in use/,
    message: "El puerto está ocupado por otro proceso.",
    fix: (_m, ctx) => ({
      code: "change-port",
      description: `Cambiar el puerto a uno libre (server.properties + openblock.json)`,
      considerable: true,
      apply: async () => {
        const port = await findFreePort(ctx.cfg.port + 1);
        await patchProperties(ctx.dir, { "server-port": String(port), "query.port": String(port), "rcon.port": String(port + 10) });
        ctx.cfg.port = port;
        await writeOpenblock(ctx.dir, ctx.cfg);
      },
    }),
  },
  {
    code: "OOM",
    severity: "error",
    pattern: /OutOfMemoryError|There is insufficient memory for the Java Runtime/,
    message: "La JVM se quedó sin memoria con la RAM asignada.",
    fix: (_m, ctx) => ({
      code: "raise-ram",
      description: "Subir -Xmx un 50% (jvm-args.txt + user_jvm_args.txt)",
      considerable: true,
      apply: async () => {
        await patchRam(ctx.dir, Math.round(ctx.cfg.ramMB * 1.5));
        ctx.cfg.ramMB = Math.round(ctx.cfg.ramMB * 1.5);
        await writeOpenblock(ctx.dir, ctx.cfg);
      },
    }),
  },
  {
    code: "PLUGIN_CRASH",
    severity: "error",
    pattern: /Error occurred while enabling ([\w-]+)/,
    message: "Un plugin revienta al cargar.",
    fix: (_m, ctx) => {
      const name = _m[1];
      return {
        code: "disable-plugin",
        description: `Mover ${name}.jar a plugins_disabled/`,
        considerable: true,
        apply: async () => {
          const disabledDir = join(ctx.dir, "plugins_disabled");
          await mkdir(disabledDir, { recursive: true });
          for (const f of await readdir(join(ctx.dir, "plugins"))) {
            if (f.toLowerCase().includes(name.toLowerCase()) && f.endsWith(".jar")) {
              await rename(join(ctx.dir, "plugins", f), join(disabledDir, f));
            }
          }
        },
      };
    },
  },
  {
    code: "TICK_BEHIND",
    severity: "warning",
    pattern: /Can't keep up!|Running \d+ms or \d+ ticks behind/,
    message: "El server no da los 20 TPS durante el arranque.",
    fix: (_m, ctx) => ({
      code: "lower-vd",
      description: `Bajar view-distance de ${ctx.cfg.viewDistance} a ${Math.max(4, ctx.cfg.viewDistance - 2)} (gameplay visible)`,
      considerable: true,
      apply: async () => {
        const vd = Math.max(4, ctx.cfg.viewDistance - 2);
        const sd = Math.max(4, ctx.cfg.simulationDistance - 1);
        await patchProperties(ctx.dir, { "view-distance": String(vd), "simulation-distance": String(sd) });
        ctx.cfg.viewDistance = vd;
        ctx.cfg.simulationDistance = sd;
        await writeOpenblock(ctx.dir, ctx.cfg);
      },
    }),
  },
  {
    code: "WORLD_UPGRADE",
    severity: "warning",
    pattern: /Upgrading level|Data fixer for/,
    message: "El mundo se va a migrar a la nueva versión (proceso largo e irreversible).",
  },
  {
    code: "PLUGIN_API_OLD",
    severity: "info",
    pattern: /Unsupported API version/,
    message: "Un plugin fue compilado para una API más antigua; suele funcionar igual.",
  },
  {
    code: "DATAPACK_FAIL",
    severity: "error",
    pattern: /Failed to (?:load|parse) (?:data )?pack/,
    message: "Un datapack del mundo está roto.",
  },
];

// ---------- reglas estáticas (ficheros + entorno) ----------

export const STATIC_RULES: StaticRule[] = [
  {
    code: "EULA_FILE",
    check: async (ctx) => {
      const p = join(ctx.dir, "eula.txt");
      if (existsSync(p) && readFileSync(p, "utf8").includes("eula=true")) return null;
      return {
        severity: "error" as const,
        code: "EULA_FILE",
        source: "eula.txt",
        message: "Falta eula=true: el server se apagaría al primer arranque.",
        fix: {
          code: "accept-eula",
          description: "Aceptar EULA (eula=true)",
          considerable: false,
          apply: async () => writeFile(p, "eula=true\n", "utf8"),
        },
      };
    },
  },
  {
    code: "SYNC_CHUNK_WRITES",
    check: async (ctx) => {
      const p = join(ctx.dir, "server.properties");
      if (!existsSync(p) || !/sync-chunk-writes=true/m.test(await readFile(p, "utf8"))) return null;
      return {
        severity: "warning" as const,
        code: "SYNC_CHUNK_WRITES",
        source: "server.properties",
        message: "sync-chunk-writes=true mata el rendimiento del IO (default de Mojang).",
        fix: {
          code: "sync-chunk-writes-off",
          description: "Poner sync-chunk-writes=false",
          considerable: false,
          apply: async () => patchProperties(ctx.dir, { "sync-chunk-writes": "false" }),
        },
      };
    },
  },
  {
    code: "RCON_PASSWORD",
    check: async (ctx) => {
      const p = join(ctx.dir, "server.properties");
      if (!existsSync(p)) return null;
      const txt = await readFile(p, "utf8");
      if (!/enable-rcon=true/m.test(txt)) return null;
      const pw = /rcon.password=(.*)/m.exec(txt)?.[1] ?? "";
      if (pw.length >= 8) return null;
      return {
        severity: "error" as const,
        code: "RCON_PASSWORD",
        source: "server.properties",
        message: "RCON habilitado con contraseña débil o vacía.",
        fix: {
          code: "rcon-password",
          description: "Generar contraseña RCON aleatoria",
          considerable: false,
          apply: async () => {
            const np = Math.random().toString(36).slice(2, 10) + Math.random().toString(36).slice(2, 8);
            await patchProperties(ctx.dir, { "rcon.password": np });
            ctx.cfg.rcon.password = np;
            await writeOpenblock(ctx.dir, ctx.cfg);
          },
        },
      };
    },
  },
  {
    code: "JAVA_MAJOR",
    check: async (ctx) => {
      const pathFile = join(ctx.dir, "runtime", "java.path");
      const javaPath = existsSync(pathFile) ? readFileSync(pathFile, "utf8").trim() : "java";
      const major = await javaMajorOf(javaPath);
      const required = javaMajorFor(ctx.cfg.mcVersion);
      if (major === required) return null;
      return {
        severity: "error" as const,
        code: "JAVA_MAJOR",
        source: "runtime/java.path",
        message: `Java ${major ?? "?"} en uso; ${ctx.cfg.mcVersion} requiere Java ${required}.`,
        fix: {
          code: "install-java",
          description: `Instalar Temurin JDK ${required} en runtime/`,
          considerable: false,
          apply: async () => {
            const runtime = await ensureJava(required, ctx.dir);
            await mkdir(join(ctx.dir, "runtime"), { recursive: true });
            await writeFile(pathFile, runtime.path + "\n", "utf8");
          },
        },
      };
    },
  },
  {
    code: "PORT_BUSY",
    check: async (ctx) => {
      const free = await isPortFree(ctx.cfg.port);
      if (free) return null;
      return {
        severity: "error" as const,
        code: "PORT_BUSY",
        source: `puerto ${ctx.cfg.port}`,
        message: `El puerto ${ctx.cfg.port} ya está ocupado (¿otro server?).`,
        fix: {
          code: "change-port",
          description: "Cambiar a un puerto libre",
          considerable: true,
          apply: async () => {
            const port = await findFreePort(ctx.cfg.port + 1);
            await patchProperties(ctx.dir, { "server-port": String(port), "query.port": String(port) });
            ctx.cfg.port = port;
            await writeOpenblock(ctx.dir, ctx.cfg);
          },
        },
      };
    },
  },
  {
    code: "ENGINE_JAR_MISSING",
    check: async (ctx) => {
      const jar = jarNameFromStartScript(ctx.dir);
      if (jar && existsSync(join(ctx.dir, jar))) return null;
      if (existsSync(join(ctx.dir, "run.sh")) || existsSync(join(ctx.dir, "run.bat"))) return null;
      return {
        severity: "error" as const,
        code: "ENGINE_JAR_MISSING",
        source: ctx.dir,
        message: `Falta el jar del engine${jar ? ` (${jar})` : ""}: instala de nuevo.`,
      };
    },
  },
  {
    code: "PLUGIN_UPDATE",
    check: async (ctx) => {
      // compara los plugins de Modrinth instalados contra la última versión publicada
      const pluginsDir = join(ctx.dir, "plugins");
      if (!existsSync(pluginsDir)) return null;
      const installed = await readdir(pluginsDir);
      for (const f of await installed) {
        if (!f.endsWith(".jar")) continue;
        const slug = KNOWN_PLUGIN_SLUGS.find((s) => f.toLowerCase().includes(s.toLowerCase().replace(/-/g, "")));
        if (!slug) continue;
        try {
          const v = await latestVersion(slug, { gameVersions: [ctx.cfg.mcVersion], loaders: [modrinthLoaderOf(ctx.cfg.engine)] });
          const latest = primaryFile(v).filename;
          if (latest && latest !== f) {
            return {
              severity: "warning" as const,
              code: "PLUGIN_UPDATE",
              source: `plugins/${f}`,
              message: `${f} tiene versión más nueva: ${latest}.`,
              fix: {
                code: "update-plugin",
                description: `Actualizar a ${latest}`,
                considerable: false,
                apply: async () => {
                  const file = primaryFile(v);
                  await rm(join(ctx.dir, "plugins", f), { force: true });
                  const { downloadTo } = await import("./net/http.js");
                  await downloadTo(file.url, join(ctx.dir, "plugins", file.filename), { sha512: file.hashes?.sha512 });
                },
              },
            };
          }
        } catch {
          // sin build para esa versión: nada que actualizar
        }
        break; // solo el primero: el doctor itera en siguientes pasadas
      }
      return null;
    },
  },
];

const KNOWN_PLUGIN_SLUGS = ["chunky", "spark", "chunkyborder", "luckperms"];

function modrinthLoaderOf(engine: ServerConfig["engine"]): string {
  return engine === "folia" ? "folia" : "paper";
}

// ---------- infra ----------

function isPortFree(port: number): Promise<boolean> {
  return new Promise((res) => {
    const srv = net.createServer();
    srv.once("error", () => res(false));
    srv.once("listening", () => srv.close(() => res(true)));
    srv.listen(port, "0.0.0.0");
  });
}

export function findFreePort(start: number): Promise<number> {
  return (async () => {
    for (let p = start; p < start + 50; p++) if (await isPortFree(p)) return p;
    throw new Error("Sin puertos libres");
  })();
}

async function patchProperties(dir: string, patch: Record<string, string>): Promise<void> {
  const p = join(dir, "server.properties");
  let txt = existsSync(p) ? await readFile(p, "utf8") : "";
  for (const [k, v] of Object.entries(patch)) {
    const re = new RegExp(`^${k}=.*$`, "m");
    txt = re.test(txt) ? txt.replace(re, `${k}=${v}`) : `${txt}${k}=${v}\n`;
  }
  await writeFile(p, txt, "utf8");
}

/**
 * Reescribe el heap en un fichero de flags JVM.
 *
 * `-Xms` y `-Xmx` van por separado y en ese orden. Antes había un solo
 * `/-Xm[sx]\d+[MG]/g` que sustituía por `-Xmx` y por tanto **también reescribía
 * `-Xms`**, dejando muerto el `replace` siguiente y rompiendo en silencio el heap
 * simétrico que genera `aikarFlags` (`-Xms` igual a `-Xmx`: evita que la JVM
 * redimensione el heap en caliente). Solo toca las flags que ya existen, que es
 * lo que pide el fix: subir la RAM asignada, no inventar flags.
 *
 * Es puro a propósito, para poder comprobarlo sin arrancar un server.
 */
export function rewriteHeapFlags(txt: string, ramMB: number): string {
  return txt.replace(/-Xms\d+[MG]/g, `-Xms${ramMB}M`).replace(/-Xmx\d+[MG]/g, `-Xmx${ramMB}M`);
}

async function patchRam(dir: string, ramMB: number): Promise<void> {
  for (const f of ["jvm-args.txt", "user_jvm_args.txt"]) {
    const p = join(dir, f);
    if (existsSync(p)) await writeFile(p, rewriteHeapFlags(await readFile(p, "utf8"), ramMB), "utf8");
  }
}

async function writeOpenblock(dir: string, cfg: ServerConfig): Promise<void> {
  await writeFile(join(dir, "openblock.json"), JSON.stringify(cfg, null, 2) + "\n", "utf8");
}

/** nombre del jar del engine extraído del start.sh generado (JAR="...") */
export function jarNameFromStartScript(dir: string): string | null {
  for (const f of ["start.sh", "start.bat"]) {
    const p = join(dir, f);
    if (!existsSync(p)) continue;
    const m = /^set "JAR=(.+)"$/m.exec(readFileSync(p, "utf8")) ?? /^JAR="(.+)"$/m.exec(readFileSync(p, "utf8"));
    if (m?.[1]) return m[1];
  }
  return null;
}

// ---------- boot test ----------

async function bootTest(cfg: ServerConfig, dir: string, capture: (line: string) => void): Promise<boolean> {
  const pathFile = join(dir, "runtime", "java.path");
  const javaPath = existsSync(pathFile) ? readFileSync(pathFile, "utf8").trim() : "java";
  let args: string[] = [];
  const jvmFile = join(dir, "jvm-args.txt");
  if (existsSync(jvmFile)) args = readFileSync(jvmFile, "utf8").split(/\s+/).filter(Boolean);
  const runner = process.platform === "win32" ? "run.bat" : "run.sh";
  const useRunner = existsSync(join(dir, runner));

  const logPath = join(dir, "logs", "latest.log");
  const before = existsSync(logPath) ? readFileSync(logPath, "utf8").length : 0;

  const child = useRunner
    ? spawn(process.platform === "win32" ? "cmd.exe" : "bash", process.platform === "win32" ? ["/c", runner, "nogui"] : [runner, "nogui"], { cwd: dir, stdio: ["ignore", "pipe", "pipe"] })
    : spawn(javaPath, [...args, "-jar", jarNameFromStartScript(dir) ?? "server.jar", cfg.engine === "vanilla" ? "nogui" : "--nogui"], { cwd: dir, stdio: ["ignore", "pipe", "pipe"] });

  child.stdout!.on("data", (d: Buffer) => d.toString().split("\n").forEach((l) => l.trim() && capture(l)));
  child.stderr!.on("data", (d: Buffer) => d.toString().split("\n").forEach((l) => l.trim() && capture(l)));

  let done = false;
  const deadline = Date.now() + 180_000;
  while (Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 2500));
    if (child.exitCode !== null || child.signalCode) break;
    if (existsSync(logPath)) {
      const tail = readFileSync(logPath, "utf8").slice(before);
      for (const l of tail.split("\n")) if (l.trim()) capture(l.trim());
      if (/Done \([0-9.]+s\)!/.test(tail)) {
        done = true;
        break;
      }
      if (/Failed to start the minecraft server|Exception in thread "main"/.test(tail)) break;
    }
  }

  // apagado: limpio por RCON si está arriba, kill si no
  if (done && cfg.rcon.enabled) {
    try {
      const { withRcon } = await import("./rcon.js");
      await withRcon("127.0.0.1", cfg.rcon.port, cfg.rcon.password, (r) => r.command("stop"));
      await new Promise((r) => setTimeout(r, 8000));
    } catch {
      // sin rcon: kill directo
    }
  }
  if (child.exitCode === null) {
    try {
      child.kill("SIGTERM");
      await new Promise((r) => setTimeout(r, 3000));
      if (child.exitCode === null) child.kill("SIGKILL");
    } catch {
      // ya murió
    }
  }
  return done;
}

// ---------- orquestación ----------

/**
 * Diagnóstico del server.
 *
 * `readOnly` no toca nada: ni ficheros, ni plugins, ni RCON. Sin él, este
 * comando reescribía `eula.txt` y `server.properties` y reinstalaba plugins
 * incluso con `--json`, mientras el README vendía ese JSON como el contrato de
 * verificación del port a Rust. Un contrato que muta el server que inspecciona no
 * sirve para comparar dos implementaciones. El CLI lo activa siempre que no haya
 * terminal interactiva (`--json`, `--yes`, tubería o CI), y el modo interactivo
 * es el único que puede arreglar cosas.
 *
 * En readOnly el score refleja el estado real, sin fixes: si falta el EULA, el
 * diagnóstico lo dice y resta, en vez de arreglarlo y puntuar después un server
 * que ya no es el que se inspeccionó.
 */
export async function runDoctor(
  cfg: ServerConfig,
  opts: { boot?: boolean; interactive?: boolean; readOnly?: boolean } = {}
): Promise<DoctorReport> {
  const dir = resolve(cfg.serverDir);
  const ctx: DoctorContext = { cfg, dir, logLines: [], bootTested: false, bootOk: false };
  const findings: Finding[] = [];
  const applied: string[] = [];
  const pending: string[] = [];

  const collect = async (f: Finding | null) => {
    if (f) findings.push(f);
  };

  const applySafe = async (fs: Finding[]): Promise<void> => {
    for (const f of fs) {
      if (!f.fix || f.fix.considerable) continue;
      console.log(c.cyan(`  ⚙ ${f.fix.description}...`));
      await f.fix.apply();
      applied.push(f.fix.code);
    }
  };

  // 1) estáticas pre-boot + fixes seguros YA (que el boot test arranque limpio).
  // En readOnly no se aplica nada: el diagnóstico solo informa.
  for (const rule of STATIC_RULES) await collect(await rule.check(ctx));
  const safeStatic = findings.filter((f) => f.fix && !f.fix.considerable);
  if (safeStatic.length && !opts.readOnly) await applySafe(safeStatic);

  // 2) boot test (dry run real)
  if (opts.boot) {
    console.log(c.gold("\n== Dry run: arrancando el server en modo test =="));
    ctx.bootTested = true;
    ctx.bootOk = await bootTest(cfg, dir, (line) => ctx.logLines.push(line));
    console.log(ctx.bootOk ? c.green("✓ El server arranca (Done).") : c.red("✗ El server no completó el arranque."));
  }

  // 3) reglas de log sobre lo capturado
  findings.push(...evaluateLogRules(ctx.logLines, ctx));

  // dedupe por código; el orden conserva estáticas primero
  const seen = new Set<string>();
  const all = findings.filter((f) => (seen.has(f.code) ? false : (seen.add(f.code), true)));

  // aplicar fixes: seguros al instante, considerables con confirmación.
  // En readOnly no se escribe nada: los seguros que se habrían aplicado solos se
  // listan como pendientes, para que el informe diga qué se puede arreglar sin
  // que el score mienta sobre el estado inspeccionado.
  for (const f of all) {
    if (!f.fix || applied.includes(f.fix.code)) continue;
    if (opts.readOnly) {
      pending.push(f.fix.considerable ? f.fix.description : `${f.fix.description} (automático)`);
    } else if (!f.fix.considerable) {
      console.log(c.cyan(`  ⚙ ${f.fix.description}...`));
      await f.fix.apply();
      applied.push(f.fix.code);
    } else if (opts.interactive && (await (await import("./ui.js")).confirm(c.yellow(`  ¿${f.fix.description}?`), false))) {
      await f.fix.apply();
      applied.push(f.fix.code);
    } else {
      pending.push(f.fix.description);
    }
  }

  // el score refleja el estado POST-fix: solo cuenta lo que sigue roto
  const unique = all.filter((f) => !(f.fix && applied.includes(f.fix.code)));

  // 5) score
  const errors = unique.filter((f) => f.severity === "error").length;
  const warnings = unique.filter((f) => f.severity === "warning").length;
  const score = Math.max(0, 100 - errors * 20 - warnings * 5);
  const grade = score >= 90 ? "SANO" : score >= 70 ? "OK CON AVISOS" : score >= 40 ? "NECESITA ATENCIÓN" : "CRÍTICO";

  return { findings: unique, applied, pending, score, grade, bootTested: ctx.bootTested, bootOk: ctx.bootOk };
}

/** resumen humano del reporte */
export function printReport(r: DoctorReport): void {
  const colors = { error: c.red, warning: c.yellow, info: c.gray } as const;
  console.log(c.gold("\n== Diagnóstico =="));
  if (r.findings.length === 0) {
    console.log(c.green("  ✓ Cero errores, cero warnings. Server fino."));
  } else {
    for (const f of r.findings) {
      const tag = { error: "ERROR", warning: "WARN ", info: "INFO " }[f.severity];
      console.log(`  ${colors[f.severity](`[${tag}]`)} ${c.dim(f.source)} — ${f.message}`);
    }
  }
  if (r.applied.length) console.log(c.green(`\n  ⚙ Arreglado automáticamente: ${r.applied.join(", ")}`));
  if (r.pending.length) console.log(c.yellow(`  ⏳ Pendiente (requiere tu decisión):\n    - ${r.pending.join("\n    - ")}`));
  const bar = "█".repeat(Math.round(r.score / 10)) + "░".repeat(10 - Math.round(r.score / 10));
  const scoreColor = r.score >= 70 ? c.green : r.score >= 40 ? c.yellow : c.red;
  console.log(`\n  Salud del server: ${scoreColor(`(${bar}) ${r.score}/100 — ${r.grade}`)}`);
  if (r.bootTested) {
    console.log(r.bootOk ? c.green("  Boot test: ✓ arranca y responde.") : c.red("  Boot test: ✗ no completó el arranque."));
  }
}
