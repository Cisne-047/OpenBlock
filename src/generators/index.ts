import type { DownloadPlan } from "../plan";
import type { ServerConfig } from "../types";
import { engineById, javaMajorFor } from "../catalog";
import { jvmArgsLine } from "./jvm";
import { serverProperties } from "./properties";
import { bukkitYml, paperGlobalYml, paperWorldDefaultsYml, spigotYml } from "./paper";
import { installPs1, installSh, jvmArgsTxt, startBat, startSh, userJvmArgsTxt } from "./scripts";
import { installServicePs1, installServiceSh, removeServicePs1, systemdUnit, winswConfig } from "./service";
import { backupPs1, backupSh, backupTimer, firewallPs1, firewallSh } from "./ops";

export type FileMap = Record<string, string>;

export function validate(cfg: ServerConfig): string[] {
  const out: string[] = [];
  if (!/^[a-z0-9][a-z0-9_-]{1,31}$/.test(cfg.name))
    out.push("Nombre: minúsculas, números y guiones (2-32 caracteres).");
  if (cfg.port < 1024 || cfg.port > 65535) out.push("Puerto fuera de rango (1024-65535).");
  if (cfg.ramMB < 512) out.push("RAM asignada demasiado baja (mínimo 512 MB).");
  if (cfg.players < 1 || cfg.players > 1000) out.push("Jugadores: 1-1000.");
  if (cfg.viewDistance < 3 || cfg.viewDistance > 32) out.push("view-distance: 3-32.");
  if (cfg.simulationDistance < 5 || cfg.simulationDistance > 32)
    out.push("simulation-distance: 5-32.");
  if (cfg.pregenRadius < 0) out.push("pregenRadius no puede ser negativo.");
  if (cfg.rcon.enabled && cfg.rcon.password.length < 8)
    out.push("RCON habilitado: contraseña de al menos 8 caracteres.");
  if (!cfg.motd) out.push("MOTD vacío.");
  return out;
}

export function readmeMd(cfg: ServerConfig, plan: DownloadPlan): string {
  const engine = engineById(cfg.engine);
  const items = plan.entries.filter((e) => e.kind === "plugin" || e.kind === "mod").map((e) => `- ${e.label} (${e.kind})`);
  return `# ${cfg.name}

Server de Minecraft forjado con **OpenBlock** — ${engine.label} ${cfg.mcVersion}
${plan.engineResolved !== cfg.mcVersion ? `(${plan.engineResolved})` : ""}

## Arrancar

- **Windows:** \`start.bat\`
- **Linux:** \`./start.sh\`

Si copiaste este proyecto a otra máquina sin instalar nada, ejecuta antes:

- **Windows:** \`install.ps1\` (PowerShell)
- **Linux:** \`./install.sh\`

## Qué lleva dentro

- Engine: ${engine.label} (${engine.desc})
- RAM asignada: ${cfg.ramMB} MB — flags: \`jvm-args.txt\`
- Puerto: ${cfg.port} — jugadores: ${cfg.players} — view-distance: ${cfg.viewDistance} / simulation: ${cfg.simulationDistance}
- Perfil de rendimiento: ${cfg.performance}
${items.length ? `\n### Plugins/mods de rendimiento\n\n${items.join("\n")}\n` : ""}
## Servicio

- **Linux:** \`sudo ./install-service.sh\` (systemd) — \`systemctl status openblock-${cfg.name}\`
- **Windows:** \`install-service.ps1\` como Administrador (WinSW)

## Firewall

- **Linux:** \`sudo ./firewall.sh\`
- **Windows:** \`firewall.ps1\` como Administrador

## Backups

- Manual: \`./backup.sh\` / \`backup.ps1\` — guarda mundos + configs en \`backups/\`, rota ${cfg.backup.keep}.
- Linux: instala el timer con \`sudo cp openblock-backup.timer /etc/systemd/system/ && sudo systemctl enable --now openblock-backup.timer\`.

## Config round-trip

Este proyecto lleva \`openblock.json\`: reimporta la config en OpenBlock o
regenera el server tras cambiarla.
`;
}

/**
 * Proyecto completo del server: mismos ficheros que apply escribe en disco.
 * targetOS controla qué scripts van (por defecto, ambos).
 *
 * Este módulo y todo `generators/` es **puro**: no importa `node:*`, no toca el
 * disco ni la red, y no lanza. La E/S vive en `src/output.ts`, como el núcleo de
 * DistroForge, que no tiene una sola referencia a `std::fs`/`std::env`.
 */
export function generateProject(cfg: ServerConfig, plan: DownloadPlan, cores: number): FileMap {
  const engine = engineById(cfg.engine);
  const gc = cfg.ramMB >= 16 * 1024 && cores >= 8 ? "zgc" : "g1";
  const argsLine = jvmArgsLine(cfg, gc, cores);
  const files: FileMap = {
    "openblock.json": JSON.stringify(cfg, null, 2) + "\n",
    "eula.txt": "eula=true\n",
    "server.properties": serverProperties(cfg),
    "start.sh": startSh(cfg, plan),
    "start.bat": startBat(cfg, plan),
    "jvm-args.txt": jvmArgsTxt(argsLine),
    "README.md": readmeMd(cfg, plan),
  };
  if (cfg.targetOS !== "windows") {
    files["install.sh"] = installSh(cfg, plan, javaMajorFor(cfg.mcVersion));
    files["firewall.sh"] = firewallSh(cfg);
    files["backup.sh"] = backupSh(cfg);
  }
  if (cfg.targetOS !== "linux") {
    files["install.ps1"] = installPs1(cfg, plan, javaMajorFor(cfg.mcVersion));
    files["firewall.ps1"] = firewallPs1(cfg);
    files["backup.ps1"] = backupPs1(cfg);
  }
  if (cfg.service.enabled) {
    if (cfg.targetOS !== "windows") {
      files["openblock.service"] = systemdUnit(cfg, cfg.service.user ?? "minecraft");
      files["install-service.sh"] = installServiceSh(cfg, cfg.service.user ?? "minecraft");
    }
    if (cfg.targetOS !== "linux") {
      files["winsw.xml"] = winswConfig(cfg);
      files["install-service.ps1"] = installServicePs1(cfg);
      files["remove-service.ps1"] = removeServicePs1(cfg);
    }
  }
  if (cfg.backup.enabled && cfg.targetOS !== "windows") {
    files["openblock-backup.timer"] = backupTimer(cfg);
  }
  // familia paper: configs dedicadas
  if (engine.pluginApi) {
    files["paper-global.yml"] = paperGlobalYml(cfg);
    files["paper-world-defaults.yml"] = paperWorldDefaultsYml(cfg);
    files["spigot.yml"] = spigotYml(cfg);
    files["bukkit.yml"] = bukkitYml(cfg);
  }
  // forge/neoforge: el launcher del installer lee las flags de aquí
  if (engine.installer) {
    files["user_jvm_args.txt"] = userJvmArgsTxt(argsLine);
  }
  return files;
}
