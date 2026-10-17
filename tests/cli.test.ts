import { describe, expect, it } from "vitest";
import { dirFrom, parseFlags } from "../src/cli";

describe("parseFlags", () => {
  const spec = { strings: ["dir", "zip"], booleans: ["json", "yes"] } as const;

  it("lee flags con valor y sin valor", () => {
    const { values } = parseFlags(["--dir", "./server", "--json"], spec);
    expect(values.dir).toBe("./server");
    expect(values.json).toBe(true);
    expect(values.yes).toBeUndefined();
  });

  it("recoge como positionals los argumentos que no son flags", () => {
    const { values, positionals } = parseFlags(["./algo", "--json"], spec);
    expect(positionals).toEqual(["./algo"]);
    expect(values.json).toBe(true);
  });

  it("un flag desconocido es error y lista los válidos", () => {
    // antes se ignoraba en silencio: `info --json` tomaba "--json" por directorio
    expect(() => parseFlags(["--json"], { strings: ["dir"] })).toThrow(/Opción desconocida: --json/);
    expect(() => parseFlags(["--json"], { strings: ["dir"] })).toThrow(/Opciones válidas: --dir/);
  });

  it("un flag sin su valor es error", () => {
    expect(() => parseFlags(["--dir"], spec)).toThrow(/le falta el valor/);
  });

  it("un flag seguido de otro flag no cuela como valor", () => {
    // `doctor --dir --json`: antes el doctor usaba "--json" como directorio
    expect(() => parseFlags(["--dir", "--json"], spec)).toThrow(/le falta el valor/);
  });
});

describe("dirFrom", () => {
  it("usa --dir cuando se pasa", () => {
    expect(dirFrom(["--dir", "./server"])).toBe("./server");
  });

  it("cae al directorio actual cuando no hay --dir", () => {
    expect(dirFrom([])).toBe(".");
  });
});
