import { existsSync } from "node:fs";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { applyConfig, ensureRconPassword } from "./apply.js";
import { javaMajorFor } from "./catalog.js";
import { runDoctor } from "./doctor.js";
import type { DoctorReport } from "./doctor.js";
import { detectHardware } from "./hw/detect.js";
import type { ServerConfig } from "./types.js";
import { c } from "./ui.js";

/**
 * `openblock self-test`: instala un server real, lo arranca, lo diagnostica y
 * **falla con código de salida distinto de cero** si algo no cuadra.
 *
 * Existe porque la suite no arranca servers: 77 tests pasan sin tocar un JAR de
 * verdad, y por eso un bug como el de `--yes` (que dejaba el flag sin efecto) se
 * coló entero. Esta es la parte que los tests no pueden cubrir: la instalación
 * real, el arranque real y el apagado limpio. Es el equivalente del smoke test
 * de QEMU que DistroForge ejecuta en CI sobre la imagen, y cubre su mismo hueco:
 * "ninguna ISO se hornea en CI" / aquí, "ningún server se arranca en CI".
 *
 * No forma parte de la suite; se invoca a mano o desde CI:
 *
 *   npm run smoke                       # CI: MC 1.20.4, puerto 25599
 *   npm run openblock -- self-test --mc 1.21.4
 *   npm run openblock -- self-test --keep   # deja el directorio para mirarlo
 */

export interface SelfTestOptions {
  /** versión de MC a instalar; su Java debe existir en la máquina */
  mc?: string;
  /** puerto del server de prueba (evita chocar con uno de verdad) */
  port?: number;
  /** no borrar el directorio temporal al terminar */
  keep?: boolean;
}

export interface SelfTestResult {
  ok: boolean;
  dir: string;
  steps: Array<{ name: string; ok: boolean; detail: string }>;
}

/** Config mínima que ejercita el camino completo sin nada que estorbe. */
export function selfTestConfig(dir: string, opts: { mc?: string; port?: number; cores: number }): ServerConfig {
  const cfg: ServerConfig = {
    name: "openblock-smoke",
    mcVersion: opts.mc ?? "1.20.4",
    engine: "paper",
    serverDir: dir,
    ramMB: 2048,
    players: 5,
    viewDistance: 5,
    simulationDistance: 5,
    port: opts.port ?? 25599,
    motd: "openblock self-test",
    difficulty: "peaceful",
    gamemode: "survival",
    worldType: "flat",
    seed: "openblock",
    onlineMode: false,
    whitelist: false,
    pvp: false,
    hardcore: false,
    modpack: { kind: "none", url: "" },
    performance: "balanced",
    // 0 = sin pregeneración: el smoke test mide arranque, no rendimiento
    pregenRadius: 0,
    backup: { enabled: false, intervalHours: 24, keep: 1 },
    service: { enabled: false },
    rcon: { enabled: true, password: "openblock-smoke-test", port: (opts.port ?? 25599) + 10 },
    chunkyborder: false,
    targetOS: process.platform === "win32" ? "windows" : "linux",
    customItems: [],
  };
  ensureRconPassword(cfg);
  return cfg;
}

/**
 * Veredicto del self-test, como función pura sobre el informe del doctor.
 *
 * Está separada de `selfTest` a propósito: el veredicto es lo único que decide
 * si CI pasa o falla, y dentro de la función que hace E/S no había forma de
 * probar el caso malo —solo se podía comprobar el camino feliz ejecutando el
 * self-test entero, que es justo lo que no se puede permitir en un test unitario.
 * Un verificador del que solo se prueba el "OK" no verifica nada.
 */
export function evaluateSelfTest(report: DoctorReport): Array<{ name: string; ok: boolean; detail: string }> {
  const codes = report.findings.map((f) => f.code);
  return [
    {
      name: "arranque real (Done)",
      ok: report.bootOk,
      detail: report.bootOk ? "el server completó el arranque" : "no llegó a Done",
    },
    {
      name: "sin findings",
      ok: codes.length === 0,
      detail: codes.length ? codes.join(", ") : "diagnóstico limpio",
    },
    {
      name: "solo lectura respetada",
      ok: report.applied.length === 0,
      detail: `applied: ${report.applied.length}`,
    },
    {
      name: "score 100",
      ok: report.score === 100,
      detail: `${report.score}/100 — ${report.grade}`,
    },
  ];
}

export async function selfTest(opts: SelfTestOptions = {}): Promise<SelfTestResult> {
  const dir = await mkdtemp(join(tmpdir(), "openblock-smoke-"));
  const steps: SelfTestResult["steps"] = [];
  const push = (name: string, ok: boolean, detail: string) => {
    steps.push({ name, ok, detail });
    console.log(`  ${ok ? c.green("✓") : c.red("✗")} ${name} — ${c.dim(detail)}`);
    return ok;
  };

  try {
    const hw = await detectHardware();
    const cfg = selfTestConfig(dir, { mc: opts.mc, port: opts.port, cores: hw.cores });
    console.log(c.bold(`\n== Self-test: Paper ${cfg.mcVersion} en ${dir} ==`));

    // 1) instalación real: descarga el jar, resuelve Java, escribe el proyecto
    await applyConfig(cfg, { cores: hw.cores, pregen: false });
    push("instalación", true, `Paper ${cfg.mcVersion}, ${hw.cores} hilos`);

    push("eula aceptado", existsSync(join(dir, "eula.txt")), "eula.txt");
    const jars = (await readdir(dir)).filter((f) => f.endsWith(".jar"));
    push("jar del engine descargado", jars.length > 0, jars.join(", ") || "ningún .jar en el directorio");

    // 2) diagnóstico con dry run REAL: arranca el server, espera "Done", apaga
    // por RCON. readOnly para que el doctor no arregle lo que debe encontrar.
    const report = await runDoctor(cfg, { boot: true, interactive: false, readOnly: true });

    // 3) veredicto: con un server recién instalado y limpio, cero findings. Un
    // finding aquí es un bug: o el generador escribe algo mal, o el doctor tiene
    // un falso positivo.
    for (const s of evaluateSelfTest(report)) push(s.name, s.ok, s.detail);

    const ok = steps.every((s) => s.ok);
    return { ok, dir, steps };
  } finally {
    if (!opts.keep) await rm(dir, { recursive: true, force: true });
    else console.log(c.yellow(`\n  Directorio conservado: ${dir}`));
  }
}

/** Ejecuta el self-test e imprime el veredicto. Devuelve el código de salida. */
export async function runSelfTest(opts: SelfTestOptions): Promise<number> {
  try {
    const r = await selfTest(opts);
    if (r.ok) console.log(c.green("\n✓ Self-test OK: instala, arranca y diagnostica.\n"));
    else {
      const fallos = r.steps.filter((s) => !s.ok).map((s) => s.name);
      console.error(c.red(`\n✗ Self-test FALLÓ: ${fallos.join(", ")}\n`));
    }
    return r.ok ? 0 : 1;
  } catch (err) {
    // El Java de la máquina no sirve para la versión pedida, o no hay red.
    console.error(c.red(`\n✗ Self-test no pudo completarse: ${(err as Error).message}\n`));
    console.error(
      c.dim(
        `  Comprueba que tienes red y un JDK ${javaMajorFor(opts.mc ?? "1.20.4")}+ ` +
          `(o prueba otra versión: --mc 1.20.4).\n`
      )
    );
    return 1;
  }
}
