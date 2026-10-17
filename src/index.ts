import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { advanced, backupCommand, buildAdvancedConfig, parseAdvancedArgs, stopCommand } from "./advanced";
import { runBenchmark } from "./hw/bench";
import { detectHardware } from "./hw/detect";
import { classifyHardware, recommendedRamMB } from "./hw/tiers";
import { generateProject, writeProject, zipProject } from "./generators";
import { guided } from "./guided";
import type { ServerConfig } from "./types";
import { c, kvTable, banner } from "./ui";

const HELP = `
OpenBlock — forja servidores de Minecraft según tu hardware (Windows/Linux)

  openblock                      Modo guiado (wizard)
  openblock guided               Igual
  openblock advanced [...]       Modo avanzado por flags
      --config openblock.json    base de config reimportable (round-trip)
      --mc 1.21.4 --engine paper|purpur|folia|fabric|quilt|forge|neoforge|vanilla
      --ram-mb 8192 --players 20 --port 25565 --vd 8 --sd 6
      --profile conservative|balanced|aggressive --pregen 4000
      --modpack <url-mrpack> --dir ./server --target linux|windows|both
      --no-service --no-backup --no-rcon --no-pregen
      --dry-run --yes --force
  openblock detect               Hardware + tier + recomendación
  openblock bench [--detect]     Benchmark CPU/disco (~5s)
  openblock generate [...]       Solo genera el proyecto (sin instalar); --zip out.zip
  openblock install [...]        advanced con apply (alias)
  openblock doctor [--dir X]     Mini-IA: dry run del server, errores/warnings,
                                 fixes automáticos, pregunta lo considerable
      --no-boot                  análisis estático sin arrancar
      --yes                      no preguntar (aplica solo lo seguro)
      --json                     reporte JSON (para scripts/port Rust)
  openblock info --dir ./server  Config del server instalado
  openblock backup --dir ./server   Ejecuta backup.sh/.ps1 ahora
  openblock stop --dir ./server  Apaga por RCON

Advanced extras: --build N (paper/purpur/folia/forge/neoforge), --loader-build N,
  --installer-build N (fabric/quilt), --custom url1,ruta2 (plugins/mods propios)

Project format: openblock.json round-trip, como DistroForge.
`;

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

async function generateCmd(argv: string[]): Promise<void> {
  const zipIdx = argv.indexOf("--zip");
  const zipOut = zipIdx >= 0 ? argv[zipIdx + 1] : undefined;
  const rest = argv.filter((_x, i) => !(zipIdx >= 0 && (i === zipIdx || i === zipIdx + 1)));
  const a = parseAdvancedArgs(rest);
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
  const dirIdx = rest.indexOf("--dir");
  const dirArg = dirIdx >= 0 ? rest[dirIdx + 1] : ".";
  const p = resolve(dirArg, "openblock.json");
  if (!existsSync(p)) throw new Error(`No existe ${p}: instala el server primero`);
  const cfg = JSON.parse(readFileSync(p, "utf8")) as ServerConfig;
  // serverDir embebido puede ser de otro SO (zip generado en Windows, corrido en
  // Linux o WSL): si no existe aquí, el doctor opera sobre --dir
  if (!existsSync(cfg.serverDir)) cfg.serverDir = resolve(dirArg);
  const json = rest.includes("--json");
  const yes = rest.includes("--yes");
  const boot = !rest.includes("--no-boot");
  console.log(c.bold(`\n== Doctor: ${cfg.name} (${cfg.engine} ${cfg.mcVersion}) ==`));
  const report = await runDoctor(cfg, {
    boot,
    interactive: !json && !yes && process.stdout.isTTY === true,
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

async function main(): Promise<void> {
  banner();
  const argv = process.argv.slice(2);
  const cmd = argv[0] && !argv[0].startsWith("-") ? argv[0] : "guided";
  const rest = cmd === "guided" ? argv.slice(1) : argv.slice(1);
  switch (cmd) {
    case "guided":
    case undefined:
      if (rest.length) {
        // guided no acepta flags: cae a advanced si hay
        await advanced(rest);
      } else {
        await guided();
      }
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
    case "info": {
      const dir = rest[rest.indexOf("--dir") + 1] ?? ".";
      await infoCmd(dir);
      break;
    }
    case "backup": {
      const dir = rest[rest.indexOf("--dir") + 1] ?? ".";
      await backupCommand(dir);
      break;
    }
    case "stop": {
      const dir = rest[rest.indexOf("--dir") + 1] ?? ".";
      await stopCommand(dir);
      break;
    }
    case "doctor": {
      await doctorCmd(rest);
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
