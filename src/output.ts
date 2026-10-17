import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { FileMap } from "./generators";

/**
 * E/S del proyecto generado: escribir a disco y empaquetar en zip.
 *
 * Vive **fuera** de `generators/` a propósito. DistroForge mantiene su núcleo
 * (`forge-core`) sin una sola referencia a `std::fs`, `std::io`, `std::process`
 * ni `std::env`: construir contenido es puro y toda la E/S ocurre en los bordes
 * (su CLI y su binding wasm). Aquí el reparto es el mismo: `generators/` produce
 * el `FileMap` y este módulo lo materializa, para que el día del port el núcleo
 * se pueda compilar a wasm o correr en el navegador sin arrastrar Node.
 *
 * `tests/purity.test.ts` comprueba que ningún fichero de `generators/` importa
 * `node:*`, así que la regla no depende de que alguien se acuerde de ella.
 */

/** Escribe el FileMap en un directorio (LF en todo; CRLF solo en .bat) */
export function writeProject(dir: string, files: FileMap): void {
  mkdirSync(dir, { recursive: true });
  for (const [name, content] of Object.entries(files)) {
    const parent = name.split("/").slice(0, -1).join("/");
    if (parent) mkdirSync(join(dir, parent), { recursive: true });
    const eol = name.endsWith(".bat") ? "\r\n" : "\n";
    writeFileSync(join(dir, name), content.replace(/\n/g, eol));
  }
}

/** Zip del proyecto (para compartir/portar a otra máquina) */
export async function zipProject(files: FileMap): Promise<Buffer> {
  const JSZip = (await import("jszip")).default;
  const zip = new JSZip();
  for (const [name, content] of Object.entries(files)) zip.file(name, content);
  return zip.generateAsync({ type: "nodebuffer" });
}
