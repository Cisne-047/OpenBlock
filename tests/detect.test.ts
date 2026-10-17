import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";

/**
 * Paridad de la detección de hardware entre Rust y el TS.
 *
 * No se puede comparar con un golden —el hardware de la máquina de CI no es el de
 * nadie—, así que se compara **en la misma máquina**: los bytes que ve Rust contra
 * los que ve Node.
 *
 * Solo se comparan los valores **deterministas**: `os.totalmem()` y
 * `os.release()`. La RAM libre y el disco libre cambian entre dos ejecuciones
 * seguidas, así que compararlos daría fallos intermitentes sin que nada esté mal;
 * el redondeo de MB sí se comprueba, que es donde estaba el fallo.
 *
 * Este test encontró dos fallos reales: el TS redondea `os.totalmem()/1048576`
 * (34266234880 B son 32678.83 MB → 32679) y yo truncaba, perdiendo 1 MB de forma
 * consistente; y `os.release()` quita el último componente en Windows
 * (`10.0.26300`, no `10.0.26300.0`).
 *
 * El binario se lanza **una vez** y se cachea: `detect --json` consulta
 * PowerShell varias veces y cada arranque cuesta segundos.
 */

const RAIZ = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const BIN = join(RAIZ, "rust", "target", "debug", process.platform === "win32" ? "forge.exe" : "forge");

interface BytesCrudos {
  ramTotalBytes: number;
  ramFreeBytes: number;
  diskFreeBytes: number;
  osVersion: string;
}

interface Perfil {
  ramTotalMB: number;
  osVersion: string;
  cpuModel: string;
  cores: number;
}

let cache: { crudo: BytesCrudos; perfil: Perfil } | null = null;

function detectar(): { crudo: BytesCrudos; perfil: Perfil } {
  if (!existsSync(BIN)) {
    throw new Error(
      `Falta el binario de Rust (${BIN}). Compílalo con: cargo build --manifest-path rust/Cargo.toml`
    );
  }
  cache ??= {
    crudo: JSON.parse(execFileSync(BIN, ["detect", "--raw-bytes"], { encoding: "utf8" })) as BytesCrudos,
    perfil: JSON.parse(execFileSync(BIN, ["detect", "--json"], { encoding: "utf8" })) as Perfil,
  };
  return cache;
}

describe("detección de hardware: paridad Rust ↔ TS", () => {
  it("Rust ve el mismo hardware determinista que Node", () => {
    const { crudo, perfil } = detectar();

    // bytes crudos: lo que compara `os.totalmem()`
    expect(crudo.ramTotalBytes).toBe(os.totalmem());
    expect(crudo.osVersion).toBe(os.release());

    // y el perfil que imprime el CLI usa el mismo redondeo que el TS, que no es
    // truncar: 34266234880 B son 32678.83 MB, que redondean a 32679
    expect(perfil.ramTotalMB).toBe(Math.round(os.totalmem() / 1048576));
    expect(perfil.osVersion).toBe(os.release());
    expect(perfil.cores).toBe(os.cpus().length);
    expect(perfil.cpuModel).toBe(os.cpus()[0]?.model.trim());
  });
});
