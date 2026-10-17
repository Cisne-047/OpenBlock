import readline from "node:readline/promises";
import { shortVersion } from "./version.js";

const enabled = process.stdout.isTTY && !process.env.NO_COLOR;

/** Paleta estilo Minecraft: verde/negrita de textos "done", dorado de títulos */
export const c = {
  bold: (s: string) => (enabled ? `\x1b[1m${s}\x1b[0m` : s),
  dim: (s: string) => (enabled ? `\x1b[2m${s}\x1b[0m` : s),
  green: (s: string) => (enabled ? `\x1b[92m${s}\x1b[0m` : s),
  yellow: (s: string) => (enabled ? `\x1b[93m${s}\x1b[0m` : s),
  gold: (s: string) => (enabled ? `\x1b[33m${s}\x1b[0m` : s),
  red: (s: string) => (enabled ? `\x1b[91m${s}\x1b[0m` : s),
  cyan: (s: string) => (enabled ? `\x1b[96m${s}\x1b[0m` : s),
  gray: (s: string) => (enabled ? `\x1b[90m${s}\x1b[0m` : s),
};

const SPLASHES = [
  "¡20 TPS o devolvemos tus chunks!",
  "100% libre de creepers*",
  "*no garantizado",
  "¡Forjado bloque a bloque!",
  "Hermano pequeño de DistroForge",
  "Ahora con Java correcto™",
  "Aikar aprueba estos flags",
  "No mires directamente al tick",
  "¡Sin lag (probablemente)!",
  "Pregenera antes de que lleguen",
  "Los chunks no se van a picar solos",
  "Hecho con boss_fight_contract: server_admin",
  "Villagers approved",
  "También soporta patatas térmicas",
  "Folia corre en tu Ryzen, créele",
];

/**
 * Línea central del banner, alineada al ancho interior de la caja (42 columnas).
 * Se calcula en vez de copiarse a mano: la versión salía duplicada aquí y en el
 * user agent, y las dos se habían desincronizado.
 */
function brandLine(): string {
  const text = `      B L O C K  ·  v${shortVersion()}`;
  return `  ║${text}${" ".repeat(42 - text.length)}║`;
}

export function banner(): void {
  const splash = SPLASHES[Math.floor(Math.random() * SPLASHES.length)];
  console.log(c.green(`
  ╔══════════════════════════════════════════╗
  ║  ██████╗ ██████╗ ███████╗███╗   ██╗      ║
  ║ ██╔═══██╗██╔══██╗██╔════╝████╗  ██║      ║
  ║ ██║   ██║██████╔╝█████╗  ██╔██╗ ██║      ║
  ║ ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║      ║
  ║ ╚██████╔╝██║     ███████╗██║ ╚████║      ║
  ║  ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝      ║
${brandLine()}
  ╚══════════════════════════════════════════╝`));
  console.log(`                 ${c.yellow(splash)}\n`);
}

/** Popup de logro estilo Minecraft */
export function achievement(title: string, desc: string): void {
  const width = Math.max(title.length, desc.length) + 4;
  const line = "─".repeat(width);
  console.log(`
  ┌${line}┐
  │${c.gold(`  ${title.padEnd(width - 2)}  `)}│
  │${c.green(`  ${desc.padEnd(width - 2)}  `)}│
  └${line}┘`);
}

/** Barra de progreso estilo cofre: [███████░░░░░] */
export function progressBar(done: number, total: number, width = 22): string {
  const pct = total > 0 ? Math.min(1, done / total) : 0;
  const filled = Math.round(pct * width);
  const bar = "█".repeat(filled) + "░".repeat(width - filled);
  return c.green(`[${bar}]`);
}

let rlInstance: readline.Interface | null = null;

function rl(): readline.Interface {
  if (!rlInstance) {
    rlInstance = readline.createInterface({ input: process.stdin, output: process.stdout });
    process.on("exit", () => rlInstance?.close());
  }
  return rlInstance;
}

export async function ask(question: string, def: string): Promise<string> {
  const answer = await rl().question(`${c.gold(question)} ${c.gray(`[${def}]`)} `);
  return answer.trim() || def;
}

export async function askNumber(question: string, def: number): Promise<number> {
  for (;;) {
    const raw = await ask(question, String(def));
    const n = Number(raw);
    if (Number.isFinite(n) && n > 0) return Math.round(n);
    console.log(c.red("  Número inválido."));
  }
}

export async function askChoice<T extends string>(
  question: string,
  options: Array<{ id: T; label: string; hint?: string }>,
  def: T
): Promise<T> {
  console.log();
  options.forEach((o, i) => {
    console.log(`  ${c.green(String(i + 1).padStart(2))}. ${c.bold(o.label)}${o.hint ? c.gray(` — ${o.hint}`) : ""}`);
  });
  for (;;) {
    const raw = await ask(question, def);
    const byIndex = options[Number(raw) - 1];
    const byId = options.find((o) => o.id === raw.toLowerCase());
    if (byIndex) return byIndex.id;
    if (byId) return byId.id;
    console.log(c.red("  Opción inválida."));
  }
}

export async function confirm(question: string, def = true): Promise<boolean> {
  const raw = await ask(`${question} (s/n)`, def ? "s" : "n");
  return /^(s|si|sí|y|yes)$/i.test(raw);
}

export function kvTable(rows: Array<[string, string]>): void {
  const width = Math.max(...rows.map(([k]) => k.length));
  for (const [k, v] of rows) console.log(`  ${c.gray(k.padEnd(width))}  ${v}`);
}
