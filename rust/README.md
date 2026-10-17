# OpenBlock en Rust

Port del núcleo a Rust, **en curso**. El TypeScript sigue siendo la
implementación de referencia y se queda en `src/` como espejo, igual que
DistroForge mantiene su `legacy/`.

## Reparto

```
crates/forge-core   núcleo generador: puro y sin E/S
crates/forge-api    resolución contra APIs de terceros: toca la red
crates/forge-cli    CLI: detección de hardware y escritura a disco
```

`forge-core` no tiene una sola referencia a `std::fs`, `std::io`, `std::process`
ni `std::env`: construir contenido es puro y toda la E/S vive en el borde. No es
una convención escrita en un documento, es la misma que el TS cumple a rajatabla
desde que se sacó la E/S fuera de `generators/`, y `tests/purity.test.ts` la
vigila al otro lado. `forge-api` es el único que hace red, igual que en el TS solo
`api/` y `net/` tienen `fetch`.

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
| `plan` (modelo + resolver) | portado, y comparado contra el TS en vivo |
| `generators::scripts` | portado |
| `generators` (raíz, `generateProject`) | portado |
| `hw::tiers` | portado |
| `hw::detect` | portado, en el CLI (toca el sistema) |
| `rcon` (códec) | portado |
| `rcon` (cliente, socket) | portado, en el CLI |
| `hw::bench` | portado: el score, el xorshift y la medición |
| `api::mojang` | portado (red + parte pura con fixture) |
| `modrinth` (parsers de modpack) | portado |
| `api::engines` | portado (con `url` y `js` de apoyo) |
| cliente de Modrinth | portado: query, primera versión y fichero primario |
| `.mrpack` (zip) | pendiente: hace falta leer el zip, no solo el índice |
| `plan` (resolver el manifiesto) | pendiente |
| `doctor` (aritmética: score, nota, dedupe) | portado |
| `doctor` (reglas, boot, arreglos) | pendiente |
| CLI | `detect`, `plan`, `dump`, `latest` y `rcon` funcionan |

**El núcleo generador está completo y con paridad byte a byte**: los 23 ficheros
del proyecto salen idénticos al TypeScript, `openblock.json` incluido. Los tiers
también, con 21 perfiles de hardware comparados contra el TS.
## Paridad

El núcleo Rust se compara **byte a byte** contra el TypeScript. El golden lo
genera el TS y lo consume Rust, en dos sitios: `forge-core/tests/golden` (lo que
produce el núcleo: ficheros, catálogo, tiers, rcon, doctor) y
`forge-api/tests/golden` (lo que mapea las respuestas de las APIs).

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

### Lo que no puede tener golden

El resolver del plan (`src/plan.ts`) lleva el número de build y el sha256 que
devuelve la API **ese día**: un golden sería una foto de PaperMC, no una prueba.
Se verifica como DistroForge verifica sus APIs: ejecutando las dos
implementaciones contra la misma API en el mismo minuto y comparando.

```bash
./node_modules/.bin/tsx tests/plan-live.ts tests/fixtures/parity-config.json > ts-plan.json
cargo run -q --manifest-path rust/Cargo.toml -p forge-cli -- \
  plan tests/fixtures/parity-config.json > rust-plan.json
diff ts-plan.json rust-plan.json
```

No está en el CI —depende de que PaperMC y Modrinth estén de pie—, así que es un
paso manual, y por eso el orden de las claves y los `dest` están escritos para
que salga limpio a la primera. La primera ejecución salió idéntica, aviso
incluido (`[plan] spark: sin build para 1.20.4 (Modrinth: sin versión de spark
para esos filtros)`), que es lo que pasa cuando el resolver se limita a
orquestar piezas que ya tenían paridad.

Lo que la paridad ha cazado, y que ninguna lectura del código habría visto:

- **Los acrónimos de serde**: `ram_mb` sale `ramMb` y el JSON dice `ramMB`;
  `target_os` sale `targetOs` y dice `targetOS`; `disk_m_bs` sale `diskMbS` y dice
  `diskMBs`. Tres `rename` explícitos. Sin esto, `openblock.json` no reimporta.
- **`URLSearchParams` escapa `~` y `encodeURIComponent` no.** Escribí el
  codificador de la query de Modrinth con el conjunto «seguro» de memoria y el
  caso de `~` lo cantó: `a b~*` sale `a+b%7E*`, no `a+b~*`. Dos codificadores
  distintos en el mismo fichero del TS, y ninguno es el que uno recuerda.
- **El regex de `parseMc`**, tres veces. Mis tres lecturas eran falsas.
- **Dos líneas en blanco de diferencia** en `install.sh` e `install.ps1`. No era
  lógica: cuando **no** hay installer, la línea de apertura del ternario del
  template literal de TS (`` `${plan.installer ``) consume un salto, así que la
  salida lleva una línea en blanco de más. Un byte de diferencia en un script que
  el usuario va a ejecutar.
- **El códec RCON arrastra un quirk del TS y hay que reproducirlo.** El TS hace
  `Buffer.alloc(4 + 4 + Buffer.byteLength(payload) + 2)`, que mide el payload en
  **UTF-8**, y luego `write(payload, 8, "ascii")`, que escribe **latin1** (más
  corto). Los bytes que sobran quedan a cero: con `"acentué ñ"` declara 21 y
  escribe 9 bytes de payload más 4 ceros. Con payload ASCII —el 100% de los
  comandos reales— los dos cómputos coinciden y no se nota. Se reproduce igual
  porque un puerto o es byte a byte o no es un puerto.
- **El benchmark del TS usa `BigInt`, que crece sin límite, y hay que crecer con
  él.** `x ^= x << 13n` **no desborda**: tras 1000 pasos el estado tiene 168 bits,
  no 64. Con `u64` la divergencia aparece ya en el primer paso, así que el núcleo
  lleva una implementación mínima de enteros de precisión arbitraria (las cuatro
  operaciones que hacen falta, sin traer una librería).
  - De paso deja un dato sobre el TS: dentro del bucle de 1.8 s el estado no para
    de crecer, así que el coste por iteración aumenta y el `ops/core/ms` que
    produce el benchmark no es una medida estable de la CPU. Se reproduce tal cual
    —el puerto no es el sitio para arreglar el original— y `classifyHardware` ya lo
    usa con un tope de 10 puntos, así que el efecto en la clasificación es acotado.

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
5. **`forge-api` sí usa `regex`, y no es un capricho.** `forgeBuilds` del TS
   construye el patrón con el prefijo de la MC **sin escapar**, así que con
   `1.20.1` los puntos son comodines y entra `1x20y1-47.2.0`. Reproducir eso a
   mano es escribir un motor de expresiones regulares: se trae `regex` (cuatro
   crates, sin C) y **solo** en `forge-api`, que ya no es puro. El núcleo sigue sin
   regex, y hay un caso en el golden que verifica el comodín.
6. **Al leer respuestas ajenas, Rust pide el tipo del contrato.** El TS lee con
   `as` y coacciona. Los casos con golden sí se reproducen —`String(undefined)`,
   `Number()`, el orden de inserción de las claves, el `id` que desaparece del
   JSON—, pero una respuesta con otro tipo (un `promos` que no es un objeto, un
   `sha256: null` donde se espera texto) da error en vez de seguir. Copiar el
   intérprete de JavaScript no es el objetivo del puerto.

## Cómo continuar

### Qué falta, en orden

| fichero TS | líneas | a dónde va | por qué |
|---|---|---|---|
| `src/api/modrinth.ts` | 126 | `forge-api` (la query y la elección) | falta la descarga del `.mrpack` y descomprimir el zip |
| `src/doctor.ts` | 629 | `forge-cli` (la aritmética ya está en el núcleo) | 16 reglas, boot test y fixes; necesita red, disco y RCON |
| `src/apply.ts`, `java.ts` | 360 | `forge-cli` | escritura, descargas, JDK |
| `src/advanced.ts`, `guided.ts`, `index.ts` | 729 | `forge-cli` | la CLI completa |

**El doctor es el trozo grande y el que más valor tiene.** El orden que menos
sufrimiento da: doctor → `apply`/`java` → el resto de la CLI.

Con el plan resuelto, `forge plan <cfg>` + `forge dump <cfg> <plan> <dir>` ya
hacen lo que hace `openblock generate`, que es el camino crítico del producto.

La pauta de `engines` —cada función de red con su parte pura al lado— es la que
hay que repetir: `paper_builds` hace la petición y `paper_builds_from` mapea la
respuesta ya parseada, que es lo único que el golden puede comparar. En el TS se
hizo el mismo corte (`paperBuilds` / `paperBuildsFrom`) precisamente para que el
puerto tenga contra qué compararse. Los ayudantes puros que salieron de ahí están
en `forge-api::url` (`encodeURIComponent`, con su conjunto de caracteres exacto) y
`forge-api::js` (`String()`, `Number()`, el `"undefined"` de un campo ausente).

### La receta para portar un módulo con paridad

1. **Decide dónde va.** ¿Toca disco, red, procesos o entorno? Si no, va al núcleo y
   tiene paridad. Si sí, va a `forge-api` (red) o `forge-cli` (el resto), y lo que
   se pueda separar en funciones puras se separa para poder compararlo.
2. **Porta con el orden de campos idéntico** en los structs que se serialicen:
   `JSON.stringify` y `serde_json` escriben en orden de declaración, y ahí es donde
   se cuela una diferencia que luego no reimporta.
3. **Añade los casos al golden** en `tests/parity-golden.ts` —incluidos los bordes
   que el TS no prueba— y regenera con `npm run parity:golden`.
4. **Escribe el test de Rust** en `rust/crates/forge-core/tests/`, comparando contra
   el golden. Para las APIs ajenas no hay golden posible: se comparan las dos
   implementaciones contra la API real, como `forge latest`.
5. **`npm run parity`** y **`cargo clippy --all-targets -- -D warnings`**. El clippy
   del CI es más nuevo que el local y encuentra cosas que aquí no saltan; han sido
   tres veces ya.
6. **`git diff --exit-code -- rust/crates/*/tests/golden`** para confirmar
   que el golden es reproducible. Tiene que serlo: si guarda algo que depende de la
   versión de Node, el CI falla —pasó con el mensaje de `JSON.parse`—.
7. **Commit y espera al CI de verdad.** Tres fallos de este port los encontró el CI
   y no la máquina local.

### Lo que no hay que "arreglar" al portar

El puerto reproduce el original **aunque el original esté discutible**. Ya han
salido seis casos y los seis están anotados arriba: el quirk UTF-8/latin1 del RCON,
el `BigInt` sin límite del benchmark, las dos líneas en blanco del ternario, las
entradas sin `env` de los modpacks, el `reduce` de paper v3 que **no** devuelve el
máximo (con `[5, 6, 7, 6]` sale la 5) y el prefijo de Forge metido en el regex sin
escapar. Si algo parece un fallo del TS, se reproduce y se anota; arreglarlo es
otra conversación, y `diff -r` deja de servir para verificar el port en cuanto las
dos versiones divergen.

Los dos últimos tienen algo en común: **el TS casi siempre hace lo correcto**. La
API devuelve las builds en orden ascendente y el `reduce` acierta; las versiones de
Minecraft no llevan comodines y el regex acierta. El puerto no puede apoyarse en
eso, porque el día que el orden cambie, el TS elegirá una build vieja y el puerto
tiene que elegir la misma.

### Qué NO mover a `legacy/`

El objetivo hablaba de dejar el TS en `legacy/`, como hizo DistroForge. **Aquí no
procede todavía**: el TS sigue siendo la implementación que está publicada en npm y
la referencia contra la que se verifica la paridad. Moverlo ahora rompería el
golden, el CI y la publicación a la vez. Se moverá cuando el Rust sea el que se
distribuye, y entonces sí tendrá sentido.
## Por qué Rust

No arregla el reparto —ese problema ya está resuelto publicando en npm, y
`cargo install` sería *peor* para el usuario porque le obligaría a tener Rust—.
Se hace porque un binario único sin dependencia de Node es un final mejor para
una CLI que la gente va a ejecutar en su máquina, y porque el núcleo ya es puro y
tiene 101 tests como contrato. El reparto en npm sigue funcionando durante todo el
port: no hay una fase en la que OpenBlock esté roto.
