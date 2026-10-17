import { describe, expect, it } from "vitest";
import { dirFrom, doctorMode, parseFlags } from "../src/cli.js";

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

describe("doctorMode", () => {
  it("--json nunca escribe, ni con terminal", () => {
    const m = doctorMode({ json: true, tty: true });
    expect(m.readOnly).toBe(true);
    expect(m.interactive).toBe(false);
  });

  it("sin terminal y sin --yes (tubería, CI): informa y no toca nada", () => {
    const m = doctorMode({ tty: false });
    expect(m.readOnly).toBe(true);
    expect(m.interactive).toBe(false);
  });

  it("--yes escribe lo seguro aunque no haya terminal", () => {
    // Regresión: la primera versión metía --yes en solo lectura, así que el flag
    // no hacía nada. Significa "no preguntes, aplica lo seguro": debe escribir.
    const m = doctorMode({ yes: true, tty: false });
    expect(m.readOnly).toBe(false);
    // pero sigue sin preguntar: lo considerable queda pendiente
    expect(m.interactive).toBe(false);
  });

  it("con terminal y sin --yes: interactivo y escribe", () => {
    const m = doctorMode({ tty: true });
    expect(m.readOnly).toBe(false);
    expect(m.interactive).toBe(true);
  });

  it("--json gana sobre --yes: si pides informe, no se escribe", () => {
    const m = doctorMode({ json: true, yes: true, tty: true });
    expect(m.readOnly).toBe(true);
  });

  it("arranca el server salvo que se pase boot:false", () => {
    expect(doctorMode({ tty: true }).boot).toBe(true);
    expect(doctorMode({ tty: true, boot: false }).boot).toBe(false);
  });
});
