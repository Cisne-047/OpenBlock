# Historial de cambios

Las etiquetas del repositorio siguen `vX.Y.Z`. La v0.2.0 es el port a Rust: el binario `forge` pasa a ser lo que se distribuye —desde los Releases de GitHub— y el TypeScript se queda en `legacy/` como espejo de paridad, generando el golden contra el que se compara cada byte. El paquete de npm se queda congelado en la v0.1.0.

OpenBlock es el hermano de [DistroForge](https://github.com/cisne-047/DistroForge) (que forja distros Linux): comparten arquitectura —config declarativa, generadores puros que producen un `FileMap`, round-trip del proyecto y comparación byte a byte— pero **no comparten código**. Aquí se registra solo lo que cambia en OpenBlock.

## v0.3.0 (31/10/2026)

### La tanda QoL: doce comandos para el día a día

- **`forge search` + `forge plugins`/`forge mods`** — búsqueda en Modrinth e instalación por slug o URL: resuelve la versión para TU MC y loader, verifica el sha512 de Modrinth y avisa de que hace falta reiniciar. `list`/`remove` y `update` (catálogo + custom con URL) completan el ciclo.
- **`forge pregen <dir>`** — la pregeneración como comando suelto, con **porcentaje real** leído del log de Chunky (el RCON de Chunky no expone progreso; el log sí).
- **`forge logs <dir> [-f] [--errors] [--player X]`** — tail del log con filtros y re-apertura cuando rota.
- **`forge world <dir> info|trim`** — tamaños reales (KB/MB/GB), seed, dificultad y borde; `trim` fija el borde con ChunkyBorder y encoge el mundo (`chunky trim`).
- **`forge list`** — registro de servers en `~/.openblock/servers.json`: cada install deja ficha, y la tabla enseña quién está arriba. `--forget` quita fichas.
- **`forge status --wait-healthy [s]`** — para scripts: espera el arranque y sale 0/1. **`--webhook <URL>`** avisa (formato Discord) si caído o TPS<15 — una pasada, para cron/timer.
- **`forge backup --rclone <remote:ruta>`** — el backup local, y copia del último a S3/B2/Drive/SFTP con rclone.
- **`forge export <dir> [--world]`** — el server instalado como zip portable (plugins incluidos; mundos y privados de sesión fuera, salvo `--world`).
- **`forge doctor <dir> --schedule`** — escribe el timer de systemd (o el comando de schtasks en Windows) para el doctor semanal.
- **`forge completions bash|zsh|fish|powershell`** — generadas estáticas; `tests/dispatch.rs` obliga a que cubran la superficie real.
- **`forge new`** — alias del guided, que ahora abre con **perfiles** (vanilla con amigos / modpack ligero / público / a medida): tres teclas y las decisiones gruesas tomadas.
- **`forge install --import-world <ruta>`** — mundos de otra instalación copiados al final (mundo ya existente no se toca).

### El constructor web (el cierre con DistroForge)

- **`crates/forge-wasm`** — `forge-core` (puro) compilado a WASM con wasm-bindgen: `generate_project_json`/`validate_json`. **`web/index.html`** es el constructor en el navegador, sin backend: config en `localStorage`, proyecto generado client-side, zip con JSZip. El plan de descargas no se resuelve en el navegador (CORS): `forge plan` lo produce y la página lo pega. `wasm-pack build` ya deja el `pkg/` compilado.

### Calidad

- **Fuzz del códec RCON**: bytes aleatorios al decodificador nunca panean, y el roundtrip fija por escrito el quirk latin1 del puerto (ASCII exacto; no-ASCII viaja con relleno, como el TS).
- 52 tests en el CLI (13 nuevos), clippy `-D warnings` y fmt limpios, y E2E de los comandos nuevos contra APIs reales (search) y fixtures (export/logs/world/pregen-fallido/wait-healthy).

## v0.2.1 (31/10/2026)

### El día a día del server ya instalado

- **`forge status <dir>`**: sondea el puerto, saca jugadores y TPS (spark) por RCON y enseña la IP local para entrar. Hasta ahora, leer el log.
- **`forge console <dir>`**: la consola del server por RCON desde donde ya estás (`exit`/Ctrl+D para salir).
- **`forge update <dir>`**: sube de build (o de versión con `--mc`) re-resolviendo el plan con la config instalada — baja el engine, refresca scripts/flags/README/`openblock.json`, borra el jar viejo y re-ejecuta el installer en forge/neoforge. **Mundo, properties, eula, whitelist, ops, plugins y mods no se tocan.** `--build latest` despina la build fijada, que antes no tenía vuelta; y actualizar a lo ya puesto avisa en vez de re-descargar.
- **`forge restore <dir>`**: restaura un backup —apaga si hace falta, aparta los mundos actuales a `pre-restore-<ts>/` (backup malo no pierde nada) y descomprime (zip con librería, tar.gz con el tar del sistema). Sin `--file`, el más reciente de `backups/`.
- **`forge whitelist` / `forge op`** (`add|remove|list`): resuelve el UUID —Mojang con online-mode, offline v3 (md5 de `OfflinePlayer:<nombre>`) sin él, fórmula cruzada byte a byte contra un cálculo independiente— y escribe los JSON bonitos; con el server arriba también se lo dice por RCON.
- **`forge versions` / `forge builds`**: las releases de Mojang (marcando `latest.release`) y las builds de cualquier engine, live, antes de instalar nada. Es lo que acepta `--build`/`--loader-build`.

### Los scripts generados ya no se rinden al primer crash

- **Bucle anti-crash en `start.sh`/`start.bat`**: un `stop` limpio (código 0) sale; un crash reinicia a los 5 s. `OPENBLOCK_AUTORESTART=0` lo desactiva para esa ejecución. El `|| CODE=$?` del sh es para que `set -e` no mate el script justo en el crash que captura. En los **dos lados de la paridad** (Rust y TS legacy) con el golden regenerado.
- Validado contra un Paper 26.3 real: boot con el bucle, `Done`, stop por RCON → el script sale solo y sin reintentos espurios; `update --build latest` lo subió de la 175 a la 231 dejando el mundo intacto.

## v0.2.0 (17/10/2026)

### El port a Rust, completo

- **El binario es el producto.** `rust/` tiene el port entero —núcleo, APIs y CLI— en tres crates: `forge-core` (puro, sin disco ni red), `forge-api` (la red) y `forge-cli` (el sistema). La CLI `forge` sustituye a `openblock`, y el TypeScript se movió a `legacy/` sin borrarse: sigue generando el golden.
- **Paridad byte a byte**: los 23 ficheros del proyecto salen idénticos al TS, `openblock.json` incluido, más los tiers, el catálogo, el códec RCON y el doctor. 51 ficheros de golden en el núcleo y 2 en las APIs; si alguien cambia un generador en un lado y no en el otro, el test falla diciendo **el fichero, la línea y el byte**. El CI regenera el golden y comprueba con `git diff --exit-code` que está al día.
- **135 tests en Rust** (101 en el TS), `clippy -D warnings` y `fmt` limpios, y el self-test de verdad —instala un Paper, lo arranca y lo diagnostica— como gate del CI.
- **El TS se partió en dos** para que el port tuviera qué comparar: cada función de red tiene al lado su mapeo puro (`paperBuilds` / `paperBuildsFrom`), con 14 tests nuevos en `legacy/tests/engines.test.ts` que fijan las rarezas que el port reproduce a propósito.
- **Divergencias anotadas, no silenciosas**: el benchmark mide con `u64` en vez del `BigInt` sin límite del TS (el score no es comparable entre los dos, y se dice), Rust pide el tipo de las respuestas ajenas en vez de coaccionar, y los enums son cerrados. La lista completa está en [`rust/README.md`](rust/README.md).
- **La paridad cazó el bump de versión**: el `User-Agent` va dentro de los `install.ps1` que se generan, así que subir la versión en Rust y no en el TS rompió el golden —con fichero, línea y byte— hasta alinearlas.

### Distribución

- **GitHub Releases** (`release.yml`): al empujar un tag `v*` compila `x86_64-pc-windows-msvc` y `x86_64-unknown-linux-gnu`, comprueba que cada binario arranca y publica el Release con los dos assets y las notas generadas.
- **Instaladores**: `install.sh` (Linux, a `~/.local/bin`) e `install.ps1` (Windows, a `%LOCALAPPDATA%\OpenBlock` y al PATH del usuario, sin permisos de administrador). Los dos bajan a un temporal y mueven al final, así que una descarga cortada no deja un `forge` a medias. El de Windows tiene `-DryRun` y los dos aceptan la versión a mano.
- **`--version`** (y `-V`) en la CLI, que es lo que los instaladores usan para confirmar que el binario quedó bien.
- El paquete de npm **no** se republica: `package.json` es privado y la 0.1.0 se queda como está.

### La superficie de la CLI, que no cubría nadie

- **`advanced` y `smoke` vuelven a existir**, como alias de `install` y de `self-test`: los tenía el TS y el port los había perdido sin que ningún test se enterara.
- **`forge install --help` imprime la ayuda**: el `--help` se parseaba y se ignoraba, así que el comando se ponía a instalar con los defaults del hardware.
- **Sin subcomando no se instala nada**: `forge` a secas imprime la ayuda, y `forge --engine paper` dice que eso es una flag y no un comando. El TS se iba al modo avanzado con todo `argv`; aquí se pide el subcomando, porque instalar por un flag mal escrito y sin preguntar es un regalo envenenado.
- **`crates/forge-cli/tests/dispatch.rs`**: 8 tests que ejecutan el binario y fijan la superficie entera —alias, `--help`, sin subcomando, comando desconocido— sin tocar red ni disco. Es el hueco por el que se colaron los alias.
- **La ayuda dice la verdad**: daba `guided` por pendiente cuando ya estaba, y no listaba ni `guided` ni `--version`.

### Documentación

- **`README.md` reescrito** para lo que se distribuye: instalación desde Releases, comandos `forge`, estructura de `rust/` y `legacy/`, y la verificación con las dos suites.
- **`rust/README.md`** deja de ser un parte de obra: estado por módulo, divergencias, lo que está portado pero no probado, cómo se saca una versión y qué queda.

## v0.1.0 (publicada en npm el 17/10/2026)

### Añadido

- **Detección de hardware y tiers** (`src/hw/`): CPU, RAM, tipo de disco (NVMe/SSD/HDD), swap y virtualización; CIM/PowerShell en Windows y `/proc` + `lsblk` en Linux. `bench` añade medida real de CPU multithread y MB/s de disco. Clasificación de patata a entusiasta, con engine recomendado, `-Xmx`, distancias, radio de pregeneración y GC (Aikar G1 o ZGC generacional).
- **Motores y builds exactas** (`src/api/`): PaperMC fill v3, Purpur, Folia, Fabric/Quilt meta, Forge maven+promos, NeoForge maven, Mojang piston-meta y Modrinth v2 (incluye `.mrpack` y zip exportado de CurseForge). Se puede clavar la build: `--build` en Paper/Purpur/Folia/Forge/NeoForge, `--loader-build` y `--installer-build` en Fabric/Quilt.
- **Generadores puros** (`src/generators/`): `jvm.ts` (Aikar/ZGC), `properties.ts`, `paper.ts`, `scripts.ts` (`start.sh`/`start.bat`/`install.sh`/`install.ps1`), `service.ts` (systemd/WinSW), `ops.ts` (firewall + backups rotativos) e `index.ts` (`FileMap` + zip portable).
- **Instalación real** (`src/apply.ts`): escribe, descarga, instala plugins/mods del catálogo de rendimiento (spark, Chunky, Lithium, C2ME, Krypton, FerriteCore, ModernFix…), resuelve el JDK correcto por versión de MC (8/17/21/25) y pregenera con Chunky por RCON.
- **El Doctor** (`src/doctor.ts`): análisis estático (EULA, `sync-chunk-writes`, contraseña RCON, Java vs MC, puerto ocupado, jar del engine, actualizaciones en Modrinth), dry run con arranque real y apagado limpio por RCON, reglas sobre el log (Java incorrecto, puerto en uso, OOM, plugin roto, `Can't keep up`, migración de mundo, datapacks rotos), fixes seguros automáticos y considerables con confirmación, más score de salud 0-100.
- **Tres modos de uso**: guiado estilo Minecraft (`src/guided.ts`, con splash texts, logros y barras de cofre), avanzado por flags (`src/advanced.ts`, con `--config` de round-trip) y CLI directa (`src/index.ts`).
- **Proyecto portable**: `openblock.json` con round-trip, zip para llevar el server a otra máquina y scripts de instalación por SO.
- **Suite vitest** (48 tests en 5 ficheros): tiers y flags Aikar exactos, properties, codec RCON, `.mrpack`/CurseForge y el doctor end-to-end sin arranque.
- **Andamiaje de repositorio heredado de DistroForge** (17/10/2026), adaptado a un CLI TypeScript y no al stack Rust:
  - `.github/workflows/ci.yml`: type-check + suite en Node 20. DistroForge corre cuatro jobs porque tiene cuatro artefactos (core Rust, servidor, UI y legacy); aquí hay uno solo.
  - `LICENSE` MIT con la **excepción de identidad** que DistroForge estrenó: la marca (aquí el banner ASCII y el nombre) queda fuera de la licencia. Se añade el aviso de no afiliación con Mojang.
  - `CHANGELOG.md` con el mismo formato en español (Añadido / Cambiado / Arreglado / Pendiente) y la aclaración, que DistroForge ya hace, de que lo listado aún **no está etiquetado**.
  - `.gitattributes` y `.editorconfig` nuevos. El primero **no es cosmético**: los `start.sh` generados llevan shebang y se rompen con CRLF, así que `*.sh` queda forzado a LF y `*.ps1`/`*.bat` a CRLF.
  - `.gitignore` ampliado: además de `node_modules/` y `dist/`, ignora `server/`, `openblock-work/` y `.env*` para que no se versionen mundos, EULA ni contraseñas de RCON.
  - `package.json`: `engines.node >=20`, `os` limitado a lo que el código soporta (win32/linux) y `license`. Sin `repository`: el repositorio es privado y aún no tiene remoto.
- **Suite de invariantes** (`tests/purity.test.ts`, `tests/helpers.ts`): pureza del núcleo, determinismo de los generadores, barrido de `undefined` sobre 11 configuraciones y factorío de configs compartido. Total: **86 tests en 8 ficheros**.
- **`openblock` existe como comando, y se publica compilado.** El README lo documentaba desde el primer día, pero no había `bin` ni shebang: solo funcionaba con `npm run openblock`, así que nadie de fuera podía instalarlo. Ahora `npm run build` emite `dist/` y el `bin` apunta ahí, de modo que el paquete instalado **no necesita `tsx` ni ningún cargador**: se ejecuta con Node a secas.

  El primer intento fue al revés —lanzador `bin/openblock.mjs` que registraba `tsx` y arrancaba el TypeScript— y **no era instalable**: al instalar desde fuera del repo, npm deja el paquete como symlink y no resuelve sus dependencias, así que el lanzador moría con `Cannot find module 'tsx/esm/api'`. Se vio instalando de verdad a un prefijo limpio, no razonando sobre el papel. La consecuencia útil: los imports relativos llevan extensión `.js` (aunque el fichero sea `.ts`), que es lo que exige Node ESM para resolver el JS emitido.
- **El paquete deja de ser `private`.** Ese flag de npm impedía publicar. A cambio entra el guarda que estaba haciendo de facto: `prepublishOnly` corre tipos y tests antes de publicar, así que un `npm publish` distraído no puede sacar una versión que no compila. El repositorio sigue privado en GitHub, que es otra cosa distinta y no depende de este flag. `npm pack` da 80 kB, y solo entra `dist/` con la documentación: sin `src/`, sin tests.
- **`openblock self-test`, y un job de CI que lo ejecuta.** Instala un Paper de verdad en un directorio temporal, lo arranca (dry run real), lo diagnostica y falla con código de salida distinto de cero si algo no cuadra: instalación, jar descargado, `Done` del arranque, cero findings, que el modo de solo lectura no haya escrito nada, y score 100. El job `smoke` (`.github/workflows/ci.yml`) lo corre con Java 21 en cada push.

  Existe por un motivo concreto, no por completitud: la suite pasa sin tocar un JAR, y por ahí se coló un bug que 77 tests daban por bueno. El veredicto vive en una función pura (`evaluateSelfTest`) precisamente para poder probar el caso **malo**: un verificador del que solo se comprueba el "OK" no verifica nada. Es el equivalente del smoke test de QEMU que DistroForge corre sobre su ISO, y cierra el mismo hueco que él tiene abierto ("ninguna ISO se hornea en CI").

### Cambiado

- **Publicado en npm**: `openblock@0.1.0` está en el registro desde el 17/10/2026 y se instala con `npm install -g openblock`. Verificado desde el registro y no solo desde el tarball local: 14 paquetes, el comando queda en el PATH, genera proyecto y zip, y su `self-test` arranca un Paper real. La publicación necesitó un paso manual de autenticación en dos pasos que npm exige y que ningún token puede saltarse — que es exactamente para lo que está.

- **El núcleo de generadores es puro de verdad, no por casualidad.** `src/generators/index.ts` importaba `node:fs` y `node:path` para escribir en disco, así que "generadores puros" era cierto de los generadores de hoja y falso del módulo que los orquesta. La E/S (`writeProject`, `zipProject`) se muda a `src/output.ts`, y `tests/purity.test.ts` comprueba que ningún fichero de `generators/` vuelve a importar `node:*`. Es el reparto de DistroForge: su `forge-core` no tiene una sola referencia a `std::fs`, `std::io`, `std::process` ni `std::env`, y toda la E/S vive en su CLI y su binding wasm. Importa para el port: el núcleo se podrá compilar a wasm o correr en el navegador sin arrastrar Node.
- **La versión deja de estar copiada por el código.** Estaba cinco veces y dos de ellas desincronizadas: el user agent decía `OpenBlock/0.1` mientras `package.json` decía `0.1.0`, y el banner llevaba la versión a mano dentro del ASCII. Ahora hay una fuente única (`src/version.ts`) que alimenta el banner, el user agent de las APIs y el de los `install.ps1` generados. Es lo que DistroForge hizo al alinear sus cinco crates en la 0.5.0.
- **Los tests dejan de duplicar la config.** El literal de `ServerConfig` (30 líneas) estaba copiado entero en dos suites; ahora hay un factorío compartido (`tests/helpers.ts`), como los helpers `cfg_for`/`app_cfg` de DistroForge. Sus convenciones se mantienen: sin ficheros de fixture, sin snapshots, cada caso se construye en el test.

### Arreglado

- **Fuga de `undefined` en ficheros generados.** Se añade el barrido que hace DistroForge sobre todas sus bases × ficheros (`tests/generators.rs:73-79`): genera 11 configuraciones distintas y falla si la subcadena `undefined` aparece en algún fichero. Aquí es más grave que allí, porque lo que se genera son scripts de shell y YAML que se ejecutan en la máquina del usuario, y un `undefined` en una ruta de `start.sh` no da error: hace algo peor.
- **`doctor --json` ya no muta el server que inspecciona.** Aplicaba los fixes seguros —reescribir `eula.txt`, `server.properties`, reinstalar plugins— sin preguntar y también con `--json`, mientras el README vendía ese JSON como el contrato de verificación del port. Un contrato que cambia justo lo que mide no sirve para comparar dos implementaciones. Ahora `runDoctor` acepta `readOnly` y el CLI lo activa para los informes: en ese modo no se escribe nada, todo lo arreglable consta como pendiente y el score refleja el estado **inspeccionado**, no uno ya parcheado. La política de escritura es una función pura (`doctorMode` en `src/cli.ts`) con test propio, porque equivocarse ahí es fácil y caro.
- **`doctor --yes` volvía a no arreglar nada.** Lo encontró la prueba contra un server real, no los tests: la primera versión de `readOnly` metía `--yes` en solo lectura —"sin terminal y con `--yes`" no era interactivo, así que caía del lado de no escribir— y dejaba el flag sin efecto, justo cuando `--yes` significa lo contrario ("no preguntes, aplica lo seguro"). Ahora `--json` nunca escribe, `--yes` sí escribe lo seguro aunque no haya terminal, y solo el modo interactivo pregunta antes de lo considerable.
- **Subir la RAM ya no rompe el heap simétrico.** El patrón `/-Xm[sx]\d+[MG]/g` sustituía siempre por `-Xmx`, así que reescribía `-Xms` también, dejaba muerto el `replace` siguiente y convertía el `-Xms512M -Xmx512M` de Aikar en `-Xmx8192M -Xmx8192M`: la JVM redimensiona el heap en caliente, que es exactamente lo que la simetría evita. Ahora van por separado, en ese orden, y la transformación es una función pura (`rewriteHeapFlags`) con test propio.
- **`openblock --engine paper --yes` ya no pierde `--engine`.** El ternario de enrutado tenía las dos ramas idénticas, así que cuando `argv[0]` era una flag se descartaba y el comando caía al wizard. Sin subcomando ahora se pasan los argumentos enteros al modo avanzado.
- **Los flags de `info`, `doctor`, `backup` y `stop` se validan.** `rest[rest.indexOf("--dir") + 1]` devolvía el índice 0 si no había `--dir`, así que `openblock info --json` usaba `"--json"` como directorio; y un `--dir` sin valor colaba el flag siguiente como ruta. Los cuatro comandos comparten ahora un parser (`src/cli.ts`) que falla con mensaje y lista las opciones válidas en vez de seguir con un default que nadie pidió. `openblock advanced --help` también funciona: el parseo de `advanced` ignoraba los flags desconocidos en silencio (`strict: false`).

- **Port a Rust, en marcha** (`rust/`). **El núcleo está completo y con paridad byte a byte verificada** contra el TypeScript: `types`, `catalog`, `plan` (modelo), `hw::tiers` y los generadores `jvm`, `properties`, `paper`, `ops`, `service`, `scripts`, más el `generateProject` entero. Los **23 ficheros** del proyecto salen idénticos, `openblock.json` incluido, y los tiers se comparan con 21 perfiles de hardware.
  - El golden lo genera el TS (`npm run parity:golden`) desde un fixture compartido y lo consume Rust (`cargo test`). Es el `diff -r` que DistroForge documenta como procedimiento manual —su README lo llama "el hueco de verificación más grande que queda"— pero automatizado y dentro de la suite: si alguien cambia un generador en un lado y no en el otro, el test falla diciendo **el fichero, la línea y el byte**.
  - La CLI ya tiene `forge dump <config> <plan> <dir>`: escribe el proyecto en disco para poder compararlo a mano, como el `forge dump` de DistroForge.
  - Comprobado que la paridad detecta de verdad: sabotee un flag de Aikar y el test lo cazó en el byte 305.
  - Y encontró cuatro fallos que leer el código no habría visto: serde no adivina acrónimos (`ram_mb` → `ramMb` cuando el JSON dice `ramMB`, y lo mismo con `targetOS` y `diskMBs`); mis tres lecturas del regex de `parseMc` eran falsas; y `install.sh` e `install.ps1` salían con **una línea en blanco de menos** porque la línea de apertura del ternario del template literal de TS consume un salto cuando no hay installer. Sin la paridad, eso habría sido un `openblock.json` que no reimporta y dos scripts con un byte de diferencia.
  - El CI gana un job `rust` con `cargo test`, `clippy -D warnings`, `cargo fmt --check` y una comprobación de que el golden está al día. Faltan `hw` (detección y tiers), resolver el plan contra las APIs, y la CLI y el doctor completos.
  - El TS sigue siendo la implementación de referencia y sigue publicado en npm: **no hay una fase en la que OpenBlock esté roto**.

### Pendiente

- **Terminar el port a Rust**: falta `hw` (detección de hardware y tiers), resolver el plan contra las APIs (PaperMC/Modrinth/Adoptium) y la CLI y el doctor completos. La parte difícil —el núcleo generador con paridad byte a byte— ya está.
- **El modo avanzado sigue con `strict: false`**: los flags desconocidos se ignoran ahí (el resto de la CLI ya falla con mensaje y lista las opciones válidas). Unificarlo pide decidir antes si `--motd`, `--seed` y compañía entran como flags de primera.
- **El self-test cubre un solo engine y una sola versión**: Paper 1.20.4 por defecto (Java 17) y Java 21 en CI. No arranca Fabric, Forge ni Folia, que son caminos distintos (`run.sh` del installer, `user_jvm_args.txt`). El self-test acota el hueco de CI, no lo cierra del todo.
- **Los avisos del plan no son fallo**: al instalar se ve `[plan] spark: sin build para 1.20.4`. Es correcto (ese plugin no publica para esa versión y se salta), pero el self-test no lo distingue de un fallo real si algún día deja de ser intencionado.

## Versiones anteriores

- Ninguna: v0.1.0 es la primera.
