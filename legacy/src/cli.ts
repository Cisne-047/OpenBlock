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

export interface DoctorMode {
  /** informe JSON en stdout: nunca escribe */
  json: boolean;
  /** `--yes`: no preguntar, aplicar solo lo seguro */
  yes: boolean;
  /** hay terminal interactiva */
  tty: boolean;
  /** se arranca el server de verdad (dry run) */
  boot: boolean;
  /** se pregunta antes de lo considerable */
  interactive: boolean;
  /** no se escribe nada: ni ficheros, ni plugins, ni RCON */
  readOnly: boolean;
}

/**
 * Política de escritura del doctor, en un solo sitio y sin E/S para poder
 * probarla. Equivocarse aquí es fácil y caro: la primera versión metía `--yes`
 * en solo lectura y dejaba el flag sin efecto, porque `--yes` significa
 * exactamente lo contrario ("no preguntes, aplica lo seguro").
 *
 * - `--json` es un informe: nunca escribe.
 * - Sin terminal y sin `--yes` (tubería, CI): informa y no toca nada.
 * - `--yes` escribe lo seguro aunque no haya terminal.
 * - Con terminal y sin `--yes`: pregunta antes de lo considerable.
 */
export function doctorMode(opts: { json?: boolean; yes?: boolean; boot?: boolean; tty: boolean }): DoctorMode {
  const json = opts.json === true;
  const yes = opts.yes === true;
  const tty = opts.tty;
  return {
    json,
    yes,
    tty,
    boot: opts.boot !== false,
    interactive: !json && !yes && tty,
    readOnly: json || (!tty && !yes),
  };
}
