/**
 * Fuente única de la versión.
 *
 * Igual que DistroForge alineó `forge-core`, `forge-cli`, `forge-wasm`, `web-ui`
 * y `server-rs` en la 0.5.0, aquí la versión no se repite por el código: se
 * declara una vez y la consumen el banner, el user agent de las APIs y los
 * scripts generados. `tests/version.test.ts` comprueba que sigue coincidiendo
 * con `package.json`, que es lo que se rompió la última vez.
 *
 * Debe seguir siendo puro: `generators/scripts.ts` importa `USER_AGENT` para
 * escribir el user agent dentro del `install.ps1` que emite, así que este módulo
 * no puede arrastrar efectos ni E/S a los generadores.
 */

/** Versión completa, tal cual en `package.json`. */
export const VERSION = "0.1.0";

/** Versión corta para pantallas: 0.1.0 → `0.1`; 0.1.2 → `0.1.2`. */
export function shortVersion(version: string = VERSION): string {
  const m = /^(\d+)\.(\d+)\.(\d+)/.exec(version);
  if (!m) return version;
  return m[3] === "0" ? `${m[1]}.${m[2]}` : `${m[1]}.${m[2]}.${m[3]}`;
}

/** Identificador con el que nos presentamos a las APIs de terceros. */
export const USER_AGENT = `OpenBlock/${VERSION} (+https://github.com/cisne-047)`;
