# Historial de cambios

Las etiquetas del repositorio siguen `vX.Y.Z`. La v0.1.0 todavía **no está etiquetada**: este es el trabajo acumulado en `main` desde el primer commit.

OpenBlock es el hermano de [DistroForge](https://github.com/cisne-047/DistroForge) (que forja distros Linux): comparten arquitectura —config declarativa, generadores puros que producen un `FileMap`, round-trip del proyecto y suite vitest— pero **no comparten código**. Aquí se registra solo lo que cambia en OpenBlock.

## v0.1.0 (sin publicar)

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
- **Suite de invariantes** (`tests/purity.test.ts`, `tests/helpers.ts`): pureza del núcleo, determinismo de los generadores, barrido de `undefined` sobre 11 configuraciones y factorío de configs compartido. Total: **62 tests en 6 ficheros**.

### Cambiado

- **El núcleo de generadores es puro de verdad, no por casualidad.** `src/generators/index.ts` importaba `node:fs` y `node:path` para escribir en disco, así que "generadores puros" era cierto de los generadores de hoja y falso del módulo que los orquesta. La E/S (`writeProject`, `zipProject`) se muda a `src/output.ts`, y `tests/purity.test.ts` comprueba que ningún fichero de `generators/` vuelve a importar `node:*`. Es el reparto de DistroForge: su `forge-core` no tiene una sola referencia a `std::fs`, `std::io`, `std::process` ni `std::env`, y toda la E/S vive en su CLI y su binding wasm. Importa para el port: el núcleo se podrá compilar a wasm o correr en el navegador sin arrastrar Node.
- **La versión deja de estar copiada por el código.** Estaba cinco veces y dos de ellas desincronizadas: el user agent decía `OpenBlock/0.1` mientras `package.json` decía `0.1.0`, y el banner llevaba la versión a mano dentro del ASCII. Ahora hay una fuente única (`src/version.ts`) que alimenta el banner, el user agent de las APIs y el de los `install.ps1` generados. Es lo que DistroForge hizo al alinear sus cinco crates en la 0.5.0.
- **Los tests dejan de duplicar la config.** El literal de `ServerConfig` (30 líneas) estaba copiado entero en dos suites; ahora hay un factorío compartido (`tests/helpers.ts`), como los helpers `cfg_for`/`app_cfg` de DistroForge. Sus convenciones se mantienen: sin ficheros de fixture, sin snapshots, cada caso se construye en el test.

### Arreglado

- **Fuga de `undefined` en ficheros generados.** Se añade el barrido que hace DistroForge sobre todas sus bases × ficheros (`tests/generators.rs:73-79`): genera 11 configuraciones distintas y falla si la subcadena `undefined` aparece en algún fichero. Aquí es más grave que allí, porque lo que se genera son scripts de shell y YAML que se ejecutan en la máquina del usuario, y un `undefined` en una ruta de `start.sh` no da error: hace algo peor.

### Pendiente

- **Port a Rust**: el hito compartido con DistroForge. DistroForge **ya lo completó** (`rust/crates/forge-core`, 77 tests espejo de los 74 del TS): la paridad no la sostiene un fichero de versión sino serialización byte-estable, suite espejo y un `diff -r` documentado. OpenBlock sigue en TypeScript y va preparando el terreno: núcleo puro, generadores deterministas y `doctor --json` como contrato.
- **Endurecer la CLI**: el parseo de flags de `info`/`backup`/`stop`/`doctor` es manual (`rest.indexOf("--dir")`), no comparte el `parseArgs` del modo avanzado y falla en silencio. Tres parsers distintos conviven.
- **El doctor no tiene modo de solo lectura**: aplica los fixes seguros incluso con `--json` o `--no-boot`, así que el "contrato para el port" reescribe ficheros y reinstala plugins mientras dice que solo informa.
- **Etiquetar la v0.1.0** y decidir si se publica en npm (`build` no emite nada y `bin`/`files` están sin definir, así que hoy no hay artefacto publicable).

## Versiones anteriores

- Ninguna: v0.1.0 es la primera.
