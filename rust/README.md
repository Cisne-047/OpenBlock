# OpenBlock en Rust

Port del núcleo a Rust, **en curso**. El TypeScript sigue siendo la
implementación de referencia y se queda en `src/` como espejo, igual que
DistroForge mantiene su `legacy/`.

## Reparto

```
crates/forge-core   núcleo generador: puro y sin E/S
crates/forge-cli    CLI (en construcción)
```

`forge-core` no tiene una sola referencia a `std::fs`, `std::io`, `std::process`
ni `std::env`: construir contenido es puro y toda la E/S vive en el borde. No es
una convención escrita en un documento, es la misma que el TS cumple a rajatabla
desde que se sacó la E/S fuera de `generators/`, y `tests/purity.test.ts` la
vigila al otro lado.

## Estado

| módulo | estado |
|---|---|
| `types` | portado (orden de campos idéntico al TS) |
| `generators::jvm` | portado |
| `generators::properties` | portado |
| `generators::paper` | portado |
| `generators::ops` | portado |
| `generators::service` | portado |
| `catalog` | portado |
| `plan` (modelo) | portado |
| `generators::scripts` | portado |
| `generators` (raíz, `generateProject`) | portado |
| `hw::tiers` | portado |
| `hw::detect` | portado, en el CLI (toca el sistema) |
| `hw::bench` | pendiente |
| `plan`/`api` (resolver contra las APIs) | pendiente |
| doctor | pendiente |
| CLI | `detect` y `dump` funcionan |

**El núcleo generador está completo y con paridad byte a byte**: los 23 ficheros
del proyecto salen idénticos al TypeScript, `openblock.json` incluido. Los tiers
también, con 21 perfiles de hardware comparados contra el TS.
## Paridad

El núcleo Rust se compara **byte a byte** contra el TypeScript. El golden lo
genera el TS y lo consume Rust:

```bash
npm run parity          # regenera el golden y corre los tests de Rust
cargo test              # solo los tests, con el golden ya generado
forge dump <cfg.json> <plan.json> <dir> [cores]   # proyecto a disco, para diff -r
```

Es el `diff -r` que DistroForge documenta como procedimiento manual, pero
automatizado y dentro de la suite: si alguien cambia un generador en un lado y no
en el otro, `cargo test` falla diciendo **el fichero, la línea y el byte**:

```
paridad rota en «jvm-args.txt» (línea 1, byte 305)
  TS     (1): "…G1ReservePercent=20…"
  Rust   (1): "…G1ReservePercent=21…"
  longitudes: TS 612 / Rust 612
```

Se comprueban las dos cosas: los generadores de hoja por separado (para que el
fallo señale cuál) y el `generateProject` completo fichero a fichero (para que
nada se quede sin comparar). El CI regenera el golden y comprueba con
`git diff --exit-code` que está al día, así que la paridad no puede derivar en
silencio.

Lo que la paridad ha cazado, y que ninguna lectura del código habría visto:

- **Los acrónimos de serde**: `ram_mb` sale `ramMb` y el JSON dice `ramMB`;
  `target_os` sale `targetOs` y dice `targetOS`; `disk_m_bs` sale `diskMbS` y dice
  `diskMBs`. Tres `rename` explícitos. Sin esto, `openblock.json` no reimporta.
- **El regex de `parseMc`**, tres veces. Mis tres lecturas eran falsas.
- **Dos líneas en blanco de diferencia** en `install.sh` e `install.ps1`. No era
  lógica: cuando **no** hay installer, la línea de apertura del ternario del
  template literal de TS (`` `${plan.installer ``) consume un salto, así que la
  salida lleva una línea en blanco de más. Un byte de diferencia en un script que
  el usuario va a ejecutar.

### La detección de hardware es otro tipo de paridad

`hw::detect` no se puede comparar con un golden: el hardware de la máquina de CI
no es el de nadie. Se compara **en la misma máquina**, que es más fuerte: `forge
detect --raw-bytes` expone los bytes sin redondear y `tests/detect.test.ts` los
compara contra `os.totalmem()` y `os.release()` de Node. Solo los valores
deterministas, porque la RAM y el disco libres cambian entre dos ejecuciones
seguidas sin que nada esté mal.

Ese test encontró dos fallos reales de redondeo, en direcciones opuestas:

- **RAM total**: el TS hace `Math.round(os.totalmem() / 1048576)` y 34266234880 B
  son 32678.83 MB, que redondean a **32679**. Yo truncaba y daba 32678, 1 MB de
  diferencia consistente en cada ejecución.
- **RAM libre**: lo mismo al revés. Yo redondeaba hacia arriba (`div_ceil`) donde
  el TS trunca.
- **`os.release()` en Windows** quita el último componente: `10.0.26300`, no
  `10.0.26300.0`.

## Divergencias conocidas

Deliberadas y anotadas, como la lista que DistroForge mantiene en su
`rust/README.md`:

1. **Enums cerrados donde el TS tiene uniones de strings.** Un engine inventado
   no compila ni deserializa, en vez de propagarse hasta el generador y fallar
   allí. El valor en JSON es idéntico.
2. **`CustomItem.url` y `path` siempre se serializan.** En el TS, un item de URL
   se construye con spread y **no lleva la clave `path`**, mientras que uno local
   no lleva `url`; aquí los dos van y el ausente sale `null`. Solo afecta a
   `customItems` en `openblock.json`, no a ningún fichero generado.
3. **`diskMBs` y `ramMB` llevan `#[serde(rename)]` explícito.** El `camelCase` de
   serde convierte `disk_m_bs` en `diskMbS`, que no es lo que dice el JSON del TS:
   serde no adivina acrónimos, así que se declaran a mano.
4. **`parseMc` está escrito a mano, sin regex.** Traer una librería de expresiones
   regulares a un núcleo que quiere ser portable a wasm no compensa por un patrón
   de tres grupos, así que la lógica está reimplementada. La paridad se comprueba
   con 13 casos límite generados por el TS (`1.2b.3`, `1.20.b`, `1.2.`, `1.2.3.4`,
   `26w14a`, `10.20.30`…), **no** con razonamiento: mis tres primeras lecturas del
   regex eran falsas y el golden las cazó una a una. Si algún día aparece una
   entrada rara, el sitio para añadirla es `tests/parity-golden.ts`.

## Por qué Rust

No arregla el reparto —ese problema ya está resuelto publicando en npm, y
`cargo install` sería *peor* para el usuario porque le obligaría a tener Rust—.
Se hace porque un binario único sin dependencia de Node es un final mejor para
una CLI que la gente va a ejecutar en su máquina, y porque el núcleo ya es puro y
tiene 86 tests como contrato. El reparto en npm sigue funcionando durante todo el
port: no hay una fase en la que OpenBlock esté roto.
