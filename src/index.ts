import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { advanced, backupCommand, buildAdvancedConfig, parseAdvancedArgs, stopCommand } from "./advanced";
import { dirFrom, doctorMode, parseFlags } from "./cli";
import { runBenchmark } from "./hw/bench";
import { detectHardware } from "./hw/detect";
import { classifyHardware, recommendedRamMB } from "./hw/tiers";
import { generateProject } from "./generators";
import { writeProject, zipProject } from "./output";
import { guided } from "./guided";
import { HELP } from "./help";
import type { ServerConfig } from "./types";
import { c, kvTable, banner } from "./ui";

async function detectCmd(withBench: boolean): Promise<void> {
  const hw = await detectHardware();
  if (withBench) {
    console.log(c.dim("Midiendo CPU y disco..."));
    hw.bench = await runBenchmark();
  }
  const tier = classifyHardware(hw);
  console.log(c.bold("\n== Hardware =="));
  kvTable([
    ["Host", `${hw.hostname} (${hw.os} ${hw.osVersion}, ${hw.arch})`],
    ["CPU", `${hw.cpuModel} — ${hw.cores} hilos${hw.virtualized ? c.yellow(" [VM]") : ""}`],
    ["RAM", `${(hw.ramTotalMB / 1024).toFixed(1)} GB (libres ${(hw.ramFreeMB / 1024).toFixed(1)} GB) + swap ${(hw.swapTotalMB / 1024).toFixed(1)} GB`],
    ["Disco", `${hw.diskKind.toUpperCase()} (libres ${(hw.diskFreeMB / 1024).toFixed(0)} GB)`],
    ...(hw.bench
      ? ([
          ["Bench CPU", `${hw.bench.cpuScore}k ops/core/ms`],
          ["Bench disco", `${hw.bench.diskMBs} MB/s escritura`],
        ] as Array<[string, string]>)
      : []),
    [c.bold("Tier"), c.cyan(`${tier.label} — hasta ${tier.maxPlayers} jugadores`)],
    ["Recomendado", `${tier.recommendedEngine}, ${tier.gc.toUpperCase()}, vd ${tier.viewDistance}/${tier.simulationDistance}, -Xmx ~${recommendedRamMB(hw, true)} MB`],
  ]);
  if (process.argv.includes("--json")) console.log(JSON.stringify(hw, null, 2));
}

/**
 * Reconstruye los flags ya normalizados para `parseAdvancedArgs`.
 * `generate` comparte el parser con el resto de comandos (y gana su validación),
 * pero el modo avanzado necesita volver a ver sus propias flags: si se las
 * quedara el parser, `--dir` o `--yes` se perderían por el camino.
 */
function toArgv(values: Record<string, string | boolean>): string[] {
  const out: string[] = [];
  for (const [k, v] of Object.entries(values)) out.push(`--${k}`, ...(v === true ? [] : [String(v)]));
  return out;
}

async function generateCmd(argv: string[]): Promise<void> {
  const { values } = parseFlags(argv, {
    strings: ["zip", "dir", "name", "mc", "engine", "ram-mb", "players", "port", "vd", "sd", "profile", "pregen", "modpack", "build", "loader-build", "installer-build", "custom", "target", "config"],
    booleans: ["yes", "force", "dry-run", "no-service", "no-backup", "no-rcon", "no-pregen"],
  });
  const zipOut = typeof values.zip === "string" ? values.zip : undefined;
  const a = parseAdvancedArgs(toArgv(values));
  const cfg = await buildAdvancedConfig(a);
  const hw = await detectHardware();
  const { buildDownloadPlan } = await import("./plan");
  const plan = await buildDownloadPlan(cfg);
  console.log(c.bold(`\n== Plan == ${plan.engineResolved}`));
  for (const e of plan.entries) console.log(`  ${c.dim(`[${e.kind}]`)} ${e.label}`);
  const files = generateProject(cfg, plan, hw.cores);
  const dir = resolve(cfg.serverDir);
  writeProject(dir, files);
  console.log(c.green(`\n✓ Proyecto generado en ${dir} (${Object.keys(files).length} ficheros)`));
  if (zipOut) {
    const zip = await zipProject(files);
    writeFileSync(resolve(zipOut), zip);
    console.log(c.green(`✓ Zip portable: ${resolve(zipOut)}`));
  }
  console.log(c.dim("En otra máquina: ./install.sh (Linux) o install.ps1 (Windows) y luego start."));
}

async function infoCmd(dir: string): Promise<void> {
  const p = resolve(dir, "openblock.json");
  if (!existsSync(p)) throw new Error(`No existe ${p}: instala el server primero`);
  const cfg = JSON.parse(readFileSync(p, "utf8")) as ServerConfig;
  console.log(c.bold(`\n== ${cfg.name} ==`));
  kvTable([
    ["Engine", `${cfg.engine} ${cfg.mcVersion}`],
    ["RAM", `${cfg.ramMB} MB`],
    ["Puerto", String(cfg.port)],
    ["Jugadores", String(cfg.players)],
    ["Perfil", cfg.performance],
    ["Pregen", String(cfg.pregenRadius)],
    ["Directorio", cfg.serverDir],
    ["Servicio", cfg.service.enabled ? "sí" : "no"],
  ]);
}

async function doctorCmd(rest: string[]): Promise<void> {
  const { runDoctor, printReport } = await import("./doctor");
  const { values } = parseFlags(rest, {
    strings: ["dir"],
    booleans: ["json", "yes", "no-boot"],
  });
  const dirArg = typeof values.dir === "string" ? values.dir : ".";
  const p = resolve(dirArg, "openblock.json");
  if (!existsSync(p)) throw new Error(`No existe ${p}: instala el server primero`);
  const cfg = JSON.parse(readFileSync(p, "utf8")) as ServerConfig;
  // serverDir embebido puede ser de otro SO (zip generado en Windows, corrido en
  // Linux o WSL): si no existe aquí, el doctor opera sobre --dir
  if (!existsSync(cfg.serverDir)) cfg.serverDir = resolve(dirArg);
  const json = values.json === true;
  const mode = doctorMode({
    json,
    yes: values.yes === true,
    boot: values["no-boot"] !== true,
    tty: process.stdout.isTTY === true,
  });
  console.log(c.bold(`\n== Doctor: ${cfg.name} (${cfg.engine} ${cfg.mcVersion}) ==`));
  const report = await runDoctor(cfg, {
    boot: mode.boot,
    interactive: mode.interactive,
    readOnly: mode.readOnly,
  });
  if (json) {
    console.log(
      JSON.stringify(
        {
          ...report,
          findings: report.findings.map(({ fix, ...f }) => ({
            ...f,
            fix: fix ? { code: fix.code, description: fix.description, considerable: fix.considerable } : undefined,
          })),
        },
        null,
        2
      )
    );
  } else {
    printReport(report);
  }
  if (report.score < 40) process.exitCode = 1;
}

async function selfTestCmd(rest: string[]): Promise<void> {
  const { values } = parseFlags(rest, { strings: ["mc", "port"], booleans: ["keep"] });
  const { runSelfTest } = await import("./selftest");
  const code = await runSelfTest({
    mc: typeof values.mc === "string" ? values.mc : undefined,
    port: typeof values.port === "string" ? Number(values.port) : undefined,
    keep: values.keep === true,
  });
  process.exitCode = code;
}

async function main(): Promise<void> {
  banner();
  const argv = process.argv.slice(2);
  // Sin subcomando (o directamente con flags) se va al modo avanzado con TODOS
  // los argumentos. Antes el ternario tenía las dos ramas iguales y descartaba
  // argv[0], así que `openblock --engine paper --yes` abría el wizard sin engine.
  const first = argv[0];
  // Sin subcomando, TODOS los argumentos son del modo avanzado: `argv[0]` ya es
  // una flag, así que no hay nada que recortar. Con subcomando sí se recorta.
  if (first === undefined || first.startsWith("-")) {
    if (first === "--help" || first === "-h") console.log(HELP);
    else await advanced(argv);
    return;
  }
  const cmd = first;
  const rest = argv.slice(1);
  switch (cmd) {
    case "guided":
      await guided();
      break;
    case "advanced":
    case "install":
      await advanced(rest);
      break;
    case "detect":
      await detectCmd(false);
      break;
    case "bench":
      await detectCmd(true);
      break;
    case "generate":
      await generateCmd(rest);
      break;
    case "info":
      await infoCmd(dirFrom(rest));
      break;
    case "backup":
      await backupCommand(dirFrom(rest));
      break;
    case "stop":
      await stopCommand(dirFrom(rest));
      break;
    case "doctor": {
      await doctorCmd(rest);
      break;
    }
    case "self-test":
    case "smoke": {
      await selfTestCmd(rest);
      break;
    }
    case "help":
    case "--help":
    case "-h":
      console.log(HELP);
      break;
    default:
      console.error(c.red(`Comando desconocido: ${cmd}`));
      console.log(HELP);
      process.exitCode = 1;
  }
}

main().catch((err) => {
  console.error(c.red(`\n✗ ${(err as Error).message}`));
  process.exit(1);
});
