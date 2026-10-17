/**
 * Parseo de flags de la CLI.
 *
 * Reemplaza los tres parsers que convivían: el `parseArgs` del modo avanzado y
 * el escaneo manual a base de `indexOf`/`includes` de `info`, `doctor`, `backup`
 * y `stop`. El manual tenía dos fallos que se notaban al usarlo:
 *
 * - `rest[rest.indexOf("--dir") + 1]` devuelve el índice 0 cuando no hay
 *   `--dir`, así que `openblock info --json` tomaba `"--json"` por directorio.
 * - Un `--dir` sin valor colaba como directorio el flag siguiente, en silencio.
 *
 * Aquí un flag sin su valor y un flag desconocido son errores explícitos. La
 * política es la de DistroForge: argumento desconocido → parar y decir cuáles
 * valen, no seguir con un default que el usuario no pidió.
 */

export interface FlagSpec {
  /** flags con valor: `--dir ./server` */
  strings?: readonly string[];
  /** flags sin valor: `--json` */
  booleans?: readonly string[];
}

export interface ParsedFlags {
  /** valor de cada flag presente; ausente = no se pasó */
  values: Record<string, string | boolean>;
  /** argumentos que no son flags (para `openblock --engine paper`, etc.) */
  positionals: string[];
}

export function parseFlags(argv: readonly string[], spec: FlagSpec): ParsedFlags {
  const strings = new Set(spec.strings ?? []);
  const booleans = new Set(spec.booleans ?? []);
  const values: Record<string, string | boolean> = {};
  const positionals: string[] = [];

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    if (!arg.startsWith("--")) {
      positionals.push(arg);
      continue;
    }
    const name = arg.slice(2);
    if (strings.has(name)) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error(`A --${name} le falta el valor (ej. --${name} ./server)`);
      }
      values[name] = value;
      i++;
    } else if (booleans.has(name)) {
      values[name] = true;
    } else {
      const validos = [...strings, ...booleans].map((f) => `--${f}`).sort().join(" ");
      throw new Error(`Opción desconocida: --${name}\n  Opciones válidas: ${validos}`);
    }
  }

  return { values, positionals };
}

/**
 * Directorio de trabajo de los comandos que operan sobre un server ya instalado.
 * `--dir X` si se pasó; si no, el directorio actual.
 */
export function dirFrom(argv: readonly string[]): string {
  const { values } = parseFlags(argv, { strings: ["dir"] });
  return typeof values.dir === "string" ? values.dir : ".";
}
