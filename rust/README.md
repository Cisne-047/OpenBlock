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
| `doctor` (reglas de log) | portado, con sus descripciones |
| `doctor` (reglas estáticas) | portado, con la vista y los hechos inyectados |
| `doctor` (informe, `forge doctor --json`) | portado y comparado con el TS en vivo |
| `doctor` (boot test) | portado: arranca, captura el log y apaga por RCON |
| `doctor` (los 12 fixes) | portado: informa, arregla y arranca el server |
| `net::downloadTo` (descarga con hash y reintento) | portado, con servidor falso en los tests |
| `java::ensureJava` (JDK del sistema o Temurin) | portado; ver la nota de abajo |
| `apply` (`install`: proyecto + java + descargas + installer) | portado y comparado con el TS |
| `apply` (`pregenerate`, `stopServer`, `runBackup`) | portado y probado con un server de verdad |
| `self-test`: el veredicto | portado, con golden (es lo que decide si un CI pasa) |
| `self-test`: el ciclo | portado y ejecutado de verdad |
| `output::zipProject` (el zip portable) | portado y comparado: mismas entradas y mismo contenido |
| `advanced` (las flags y la config) | portado: flag > config previa > hardware |
| `hw::bench` (la medida, no solo la fórmula) | portado, con una divergencia que se explica |
| `guided` (el asistente) | portado, salvo la rama del modpack |
| CLI | `detect`, `bench`, `plan`, `dump`, `install`, `generate`, `info`, `guided`, `doctor`, `backup`, `stop`, `self-test`, `latest`, `rcon` y `help` |
| **falta** | la rama de modpack del asistente (`fetchMrpack` y el zip de CurseForge) |

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

El **doctor** se verifica igual, y con más motivo: dos de sus siete reglas
estáticas preguntan a la máquina (la versión de Java, si el puerto está libre) y
una a Modrinth, así que su salida no es reproducible ni con un directorio fijo.
Lo que sí es comparable es el informe entero de solo lectura, que es el que usan
los scripts:

```bash
# un server de verdad, generado por el propio Rust
cargo run -q -p forge-cli -- plan tests/fixtures/parity-config.json /tmp/plan.json
cargo run -q -p forge-cli -- dump tests/fixtures/parity-config.json /tmp/plan.json /tmp/server

# el TS imprime su banner antes del JSON: se corta desde la primera línea que es «{»
./node_modules/.bin/tsx src/index.ts doctor --dir /tmp/server --no-boot --json | sed -n '/^{/,$p' > ts.json
cargo run -q -p forge-cli -- doctor --dir /tmp/server --json > rs.json
diff ts.json rs.json
```

Salen idénticos byte a byte, con el `source` absoluto de `ENGINE_JAR_MISSING`
incluido. Ese `source` es además la prueba de que el `\r?` de
`jarNameFromStartScript` está bien puesto: el jar que sale en el mensaje
(`paper-1.20.4-499.jar`) se leyó del `start.bat` **con finales CRLF** que acaba
de generar `forge dump`.

El **boot test** se verifica arrancando un server de verdad. El camino completo
—`forge plan`, `forge dump`, bajar el jar del plan y `forge doctor --boot`— da
`bootTested: true`, `bootOk: true` y `SANO`, y con el jar corrompido da
`bootOk: false` en tres segundos (el bucle corta al ver que el proceso murió, sin
esperar los 180 s). Las dos cosas salen **idénticas al TS**, y en el arranque real
sale además un hallazgo de verdad —`TICK_BEHIND`, con la descripción
«Bajar view-distance de 8 a 6» calculada desde la config— y el `pending` con él.
Que los dos coincidan en un log de 2600 líneas de Paper es la prueba de que la
captura por stdout/stderr y por la cola del log hace lo mismo en los dos lados.

Y los **fixes** se verifican por el resultado, que es lo que le importa al usuario:
dos copias idénticas de un server roto, `doctor --yes --no-boot` en el TS y
`doctor --yes` en Rust, y después **todos los ficheros comparados hash a hash**.
Salen idénticos: `eula.txt`, el `server.properties` parcheado conservando sus
finales CRLF, y el `openblock.json` reescrito. El escenario lleva la contraseña
RCON larga a propósito: ese fix genera una contraseña aleatoria y no puede
compararse por el valor (solo por la forma, que sí tiene su test).

El fix de `update-plugin` no se puede provocar a voluntad —haría falta que
Modrinth publique una versión nueva de un plugin conocido justo cuando se ejecuta
el test—, así que se prueba con un **servidor HTTP local**: se le da al hallazgo
la url, el nombre y el sha512, y se comprueba que borra el jar viejo, escribe el
nuevo y **rechaza** el fichero si el hash no es el suyo. Es el mismo patrón que el
servidor falso de RCON.

### Lo que está portado pero no probado, y se dice

`install-java` se verificó de punta a punta contra el TS: con una config de 1.21.4
en una máquina que tiene Java 17 en el PATH, los dos bajan a buscar un JDK 21, los
dos encuentran el **mismo** (`jdk-21.0.11.10-hotspot`, no el `jre-21` que también
reporta 21) y escriben el mismo `runtime/java.path`. Los directorios salen
idénticos.

Lo que **no** está probado es la rama de descarga: el camino que baja un Temurin de
Adoptium y lo descomprime con `tar`. Haría falta una máquina sin ese JDK, y bajarse
200 MB en cada `cargo test` no es una opción. Lo que sí está probado de esa rama:
la lectura de la respuesta de Adoptium (con un recorte de la respuesta **real** en
`tests/fixtures/adoptium-21.json`) y la detección de la raíz tras extraer. Que el
`tar` extraiga y que el JDK extraído reporte la versión pedida solo se sabrá el día
que se ejecute en una máquina sin JDK; queda anotado como lo que es.

Y `javaBinDir` no se portó: el TS la exporta y **no la usa nadie**.

### `install`, con los dos servers instalados

`apply` se verifica por el resultado, que es lo que el usuario tiene delante:

```bash
./node_modules/.bin/tsx tests/apply-live.ts tests/fixtures/parity-config.json /tmp/a 16
cargo run -q -p forge-cli -- install tests/fixtures/parity-config.json --dir /tmp/b --cores 16
diff -r /tmp/a /tmp/b
```

Salen **dos servers idénticos**: los 23 ficheros del proyecto, el jar de Paper
(42 MB), el plugin de Modrinth y el `runtime/java.path`. La única línea que
difiere es la del `serverDir` dentro de `openblock.json`, y tiene que diferir: cada
server guarda dónde está. Las `cores` van fijas a 16 en los dos lados porque el TS
las saca de su detección de hardware y el Rust de `num_cpus`, y ahí no se está
comparando la detección — eso tiene su propio test.

La pregeneración va apagada en la comparación: arranca el server y lanza Chunky por
RCON durante 20 minutos.

### La pregeneración, arrancando un server de verdad

`forge install --pregen` sobre un Paper 1.20.4 recién instalado, de punta a punta:

```
== Pregeneración: arrancando server en background ==
Esperando arranque (mira logs/latest.log)...
✓ Server arriba.
chunky radius 4000
  [Chunky] Radius changed to 4000.
chunky start
  [Chunky] Task started in world for the square region centered at 0, 0 with radius 4000.
  TPS: Unknown command. Type "/help" for help.
```

Chunky se puso a generar chunks de verdad (ETA 28 minutos, 110 chunks/s), y el
`spark tps` respondió «Unknown command» porque spark no tiene build para 1.20.4 —que
es justo el caso que el TS se traga con un `try/catch`, y que aquí se imprime sin
fallar—. Después, `forge stop` mandó el `stop` por RCON y el server se apagó solo:
«All dimensions are saved», «Closing Server», y cero procesos `java` vivos.

`forge backup` lanza el `backup.ps1` que genera OpenBlock y hereda su salida. En la
máquina donde se probó, Windows tiene la ejecución de scripts deshabilitada, así que
el script no corre y el comando sale con «Backup salió con código 1» — exactamente lo
que diría el TS, que tampoco fuerza `-ExecutionPolicy`. Lo que queda probado es el
camino de error, no el backup en sí.

Y `extractCfOverrides` **no** se portó: el TS la exporta y no la llama nadie, igual
que `javaBinDir`.

### El self-test, que es la prueba de todo lo demás

`forge self-test` es el smoke test del CI del TS llevado al Rust: instala un Paper
en un directorio temporal, lo arranca de verdad, lo diagnostica en solo lectura y
decide. Ejecutado en esta máquina:

```
  ✓ instalación — Paper 1.20.4, 16 hilos
  ✓ eula aceptado — eula.txt
  ✓ jar del engine descargado — paper-1.20.4-499.jar
  ✓ arranque real (Done) — el server completó el arranque
  ✓ sin findings — diagnóstico limpio
  ✓ solo lectura respetada — applied: 0
  ✓ score 100 — 100/100 — SANO
```

El **veredicto** —lo único que decide si un CI pasa o falla— es puro, y va con
golden en `tests/golden/selftest.json`: cinco casos, cuatro de ellos malos (no
arranca, con un finding, aplicó algo en solo lectura, varios fallos a la vez). Un
verificador del que solo se prueba el «OK» no verifica nada; por eso el veredicto
está separado de la función que hace E/S, en el núcleo y no en el CLI.

### El zip portable, que no puede ser byte a byte

`forge generate --zip` empaqueta el proyecto para llevarlo a otra máquina, y el zip
**no puede compararse byte a byte**: JSZip le pone a cada entrada la fecha en que se
generó, y aquí también. Lo que sí se compara es lo que importa:

```bash
./node_modules/.bin/tsx src/index.ts generate --config tests/fixtures/parity-config.json --dir /tmp/a --zip /tmp/a.zip
cargo run -q -p forge-cli -- generate tests/fixtures/parity-config.json --dir /tmp/b --zip /tmp/b.zip --cores 16
tar -tf /tmp/a.zip | sort > a.txt && tar -tf /tmp/b.zip | sort > b.txt && diff a.txt b.txt
```

Mismas **23 entradas** y, al extraerlos, mismo contenido fichero a fichero: la única
diferencia es el `serverDir` dentro de `openblock.json`, que tiene que ser distinto
porque los dos proyectos están en directorios distintos.

Las entradas van **sin comprimir**, como las del TS: `generateAsync` sin
`compression` usa `STORE`. Comprimirlas daría un zip más pequeño y ya no sería lo
que genera el original.

### Las flags del modo avanzado, y las dos contraseñas

`forge generate` acepta las mismas flags que el TS (`--engine`, `--mc`, `--ram-mb`,
`--vd`, `--sd`, `--profile`, `--pregen`, `--no-backup`, `--custom`…), con la misma
precedencia: **flag > config previa > hardware**. Comparado con el TS con un juego
de flags de verdad, salen 17 ficheros idénticos salvo tres líneas: el `serverDir`
(que tiene que diferir) y la contraseña de RCON, que es aleatoria.

Y ese diff encontró un fallo del port: **el TS tiene dos generadores de contraseña
distintos**, y yo había portado uno solo.

| dónde | cómo | forma |
|---|---|---|
| `ensureRconPassword` (apply) | `randomBytes(8).toString("hex")` | 16 hexadecimales |
| fix `rcon-password` del doctor | `Math.random().toString(36)` | 14 en base 36 |

Los dos se reproducen ahora por separado, y la entropía del primero sale del
sistema operativo (`getrandom`), no del reloj: el TS usa `crypto.randomBytes` y una
contraseña de RCON no es sitio para un xorshift.

### La medida del benchmark, que sí diverge y se dice

La **fórmula** del score está portada y con golden. La **medida** no puede estarlo, y
al portarla se vio por qué:

| | cpuScore | duración |
|---|---|---|
| TS (BigInt de V8) | ~8 000 | ~1,9 s |
| Rust con `BigUint` propio | **1** | **43 s** |
| Rust con `u64` | ~107 000 | 1,8 s |

El original mezcla con `BigInt` y ese estado crece sin límite, así que su número
mide, en buena parte, **lo rápido que sea el BigInt de su motor**. Reproducir el
quirk con un bignum propio da un score mil veces menor y el tier se cae a la patata;
medir con `u64` da uno trece veces mayor. Ninguno de los dos es «el del TS», porque
el del TS no es una medida de la CPU.

Se mide con `u64` —que es lo que un benchmark de CPU quiere medir— y queda dicho que
el número **no es comparable**: lo que `classifyHardware` hace con él está topado a
10 puntos, así que la clasificación no cambia.

### El asistente, y lo que no se pudo comparar

`forge guided` está portado y probado alimentándolo con respuestas por la entrada
estándar: recorre las preguntas, construye la config y llega al paso de instalar.

Lo que **no** se pudo hacer es compararlo con el del TS con las mismas respuestas: su
`readline` no se lleva bien con una entrada por tubería —imprime el hardware y las
opciones de la primera pregunta y se queda ahí—, así que haría falta una terminal de
verdad para los dos. Queda dicho como lo que es: probado por un lado, no comparado.

La rama del modpack (Modrinth y CurseForge) no está portada: pide la URL, resuelve el
`.mrpack` y saca de ahí la versión de MC y el loader. El asistente lo dice y sale, en
vez de fingir que lo ha hecho.

Lo que la paridad ha cazado, y que ninguna lectura del código habría visto:

- **Los acrónimos de serde**: `ram_mb` sale `ramMb` y el JSON dice `ramMB`;
  `target_os` sale `targetOs` y dice `targetOS`; `disk_m_bs` sale `diskMbS` y dice
  `diskMBs`. Tres `rename` explícitos. Sin esto, `openblock.json` no reimporta.
- **`URLSearchParams` escapa `~` y `encodeURIComponent` no.** Escribí el
  codificador de la query de Modrinth con el conjunto «seguro» de memoria y el
  caso de `~` lo cantó: `a b~*` sale `a+b%7E*`, no `a+b~*`. Dos codificadores
  distintos en el mismo fichero del TS, y ninguno es el que uno recuerda.
- **El `$` de JS casa antes de un `\r` y el de Rust no.** `jarNameFromStartScript`
  busca `^set "JAR=(.+)"$` en el `start.bat`, y un `.bat` se escribe con finales
  CRLF. En JS el `$` multilínea también casa antes de `\r`, así que funciona; en
  Rust el `\r` queda dentro de la línea y el patrón no casa. Sin el `\r?`, el
  doctor habría dicho «falta el jar del engine» en **todos** los proyectos
  generados en Windows. Lo cazó el test del propio port, no el golden: el golden
  escribe los fixtures con LF.
- **La clave de `patchProperties` va sin escapar, y renombra líneas.** El TS hace
  `new RegExp(\`^${k}=.*$\`, "m")` con `k = "query.port"`: el punto es un comodín,
  así que la expresión casa con `queryXport=9`… y como el reemplazo escribe la
  clave buena, esa línea acaba **renombrada** a `query.port=25566`. Comprobado
  ejecutando el JS de verdad (`node -e` con el mismo `new RegExp`), no de memoria.
  En Rust, además, el `.*` hay que escribirlo `[^\r\n]*`: el `.` de JS no cruza el
  `\r` y el de Rust sí, y de eso depende que un `server.properties` con finales
  CRLF los conserve al parchearlo.
- **El boot test solo funciona en el primer arranque de un directorio.** `antes` es
  el tamaño del log anterior y la cola se lee desde ahí, pero Paper **rota**
  `latest.log` al arrancar: el fichero nuevo es más corto, la cola sale vacía, no
  se ve el `Done` y el informe dice que el server no arrancó. Peor: como `done` es
  falso, tampoco se apaga por RCON — se mata, sin guardar el mundo. No es una
  sospecha: los dos CLI dicen **lo mismo byte a byte** en ese caso (verificado con
  `2026-10-17-1.log.gz` y `-2.log.gz` en `logs/`), que es lo que hace que se pueda
  afirmar sin adivinar. Se reproduce, y arreglarlo es otra conversación.
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
5. **`regex` está en `forge-api` y en el núcleo, y no es un capricho.** Dos sitios,
   por dos razones distintas:
   - En `forge-api`, por `forgeBuilds`: el TS construye el patrón con el prefijo de
     la MC **sin escapar**, así que con `1.20.1` los puntos son comodines y entra
     `1x20y1-47.2.0`. Reproducir eso a mano es escribir un motor de expresiones
     regulares. Hay un caso en el golden que verifica el comodín.
   - En el núcleo, por las **nueve reglas de log del doctor**: alternancias, un
     grupo opcional y clases de caracteres, sobre líneas de log ajenas. Escribirlas
     a mano es exactamente como se consigue un doctor que no ve un crash.
   `parseMc` sigue escrito a mano porque son tres grupos y no compensaba; esto no
   era lo mismo. Y ojo con `\w`: el del TS es ASCII y el de `regex` es Unicode, así
   que donde aparece (`([\w-]+)` de `PLUGIN_CRASH`, el slug de Modrinth) la clase va
   escrita a mano.
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
| `src/doctor.ts` | 629 | `forge-cli` (aritmética, reglas de log y estáticas ya en el núcleo) | quedan el boot test, los `apply` y el subcomando |
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
