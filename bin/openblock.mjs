#!/usr/bin/env node
/**
 * Lanzador del comando `openblock`.
 *
 * `bin` no puede apuntar a `src/index.ts` directamente: Node no ejecuta
 * TypeScript. Este fichero registra `tsx` como cargador y luego importa la CLI,
 * así que `openblock` funciona recién instalado y sin paso de compilación —
 * coherente con que `build` sea solo `tsc --noEmit` y `src/` sea el artefacto,
 * igual que DistroForge mantiene su `legacy/` como fuente y no como `dist/`.
 *
 * `createRequire` es para poder registrar un cargador CommonJS desde un módulo
 * ESM, que es lo que el paquete declara ser (`"type": "module"`).
 */
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
require("tsx/esm/api").register();

// pathToFileURL y no la ruta a pelo: en Windows, `import("F:\\...")` revienta con
// ERR_UNSUPPORTED_ESM_URL_SCHEME porque el cargador ESM solo acepta URLs.
const here = dirname(fileURLToPath(import.meta.url));
await import(pathToFileURL(join(here, "..", "src", "index.ts")).href);
