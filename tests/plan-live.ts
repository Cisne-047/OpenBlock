/**
 * Imprime el plan de descargas del TypeScript para una config, tal cual.
 *
 * Es la mitad de la verificación del resolver: el plan **no puede tener golden**
 * (lleva el número de build y el sha256 que devuelve la API ese día), así que se
 * comparan las dos implementaciones contra las mismas APIs en el mismo minuto:
 *
 * ```bash
 * ./node_modules/.bin/tsx tests/plan-live.ts tests/fixtures/parity-config.json > ts-plan.json
 * cargo run -q --manifest-path rust/Cargo.toml -p forge-cli -- \
 *   plan tests/fixtures/parity-config.json > rust-plan.json
 * diff ts-plan.json rust-plan.json
 * ```
 *
 * Sin argumentos coge `tests/fixtures/parity-config.json`. Uso: `tsx tests/plan-live.ts [config]`.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { buildDownloadPlan } from "../src/plan.js";
import type { ServerConfig } from "../src/types.js";

const raiz = new URL("..", import.meta.url);
const ruta = process.argv[2] ?? fileURLToPath(new URL("tests/fixtures/parity-config.json", raiz));
const cfg = JSON.parse(readFileSync(ruta, "utf8")) as ServerConfig;
const plan = await buildDownloadPlan(cfg);
process.stdout.write(JSON.stringify(plan, null, 2) + "\n");
