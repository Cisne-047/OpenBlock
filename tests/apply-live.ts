/**
 * Ejecuta el `apply` del TypeScript: escribe el proyecto, deja el Java, baja el
 * engine, los plugins y los mods. Es la mitad de la verificación de `install`.
 *
 * ```bash
 * ./node_modules/.bin/tsx tests/apply-live.ts tests/fixtures/parity-config.json /tmp/a 16 > /dev/null
 * cargo run -q -p forge-cli -- install tests/fixtures/parity-config.json --dir /tmp/b --cores 16
 * diff -r /tmp/a /tmp/b
 * ```
 *
 * La pregeneración va apagada: arranca el server y lanza Chunky por RCON, y aquí
 * lo que se compara son los ficheros.
 *
 * Uso: `tsx tests/apply-live.ts <config> <destino> [cores]`.
 */
import { readFileSync } from "node:fs";
import { applyConfig, ensureRconPassword } from "../src/apply.js";
import type { ServerConfig } from "../src/types.js";

const [ruta, destino, cores] = process.argv.slice(2);
if (!ruta || !destino) {
  console.error("uso: tsx tests/apply-live.ts <config.json> <destino> [cores]");
  process.exit(2);
}
const cfg = JSON.parse(readFileSync(ruta, "utf8")) as ServerConfig;
// el serverDir de la config puede ser de otra máquina
cfg.serverDir = destino;
ensureRconPassword(cfg);
await applyConfig(cfg, {
  cores: cores ? Number(cores) : 16,
  pregen: false,
  onLog: (l) => console.log(l),
});
