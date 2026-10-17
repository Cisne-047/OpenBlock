# OpenBlock

**Forja servidores de Minecraft según tu hardware.** Hermano de [DistroForge](https://github.com/cisne-047/DistroForge) (que forja distros Linux): OpenBlock forja servidores de MC.

> **Dónde está cada cosa.** El **producto es el binario de Rust** de [`rust/`](rust/README.md): ahí están el plan, el generador, el doctor y toda la CLI (`forge`). El **TypeScript vive en [`legacy/`](legacy/)** como referencia y **espejo de paridad**: el golden lo sigue generando él, así que cada byte de lo que produce el Rust se sigue comparando contra el original. El paquete de npm quedó **congelado en la v0.1.0**.
>
> Lo de abajo describe el comportamiento del CLI, que es el mismo en los dos: `forge <comando>` en Rust (lo que se distribuye) y `openblock <comando>` en el TypeScript de `legacy/`.

- **Requiere**: el binario (no necesita Node ni Rust). El TypeScript de `legacy/`, si quieres usarlo, pide Node.js 20 o superior. Java se gestiona solo (OpenBlock se descarga el JDK que pida la versión de MC), aunque para el `self-test` conviene tener uno a mano
- **Historial de cambios**: [CHANGELOG.md](CHANGELOG.md)
- **Licencia**: MIT para el código; el banner y el nombre quedan fuera ([LICENSE](LICENSE))
- **Estado**: v0.2.0 — el binario se instala desde [Releases](https://github.com/Cisne-047/OpenBlock/releases) (Windows y Linux x86_64); el paquete de npm se quedó en la v0.1.0, congelado

Detecta tu máquina (Windows/Linux), elige engine, build exacta, RAM, GC, distancias y plugins/mods de rendimiento, instala todo y **deja el server arrancable con un script**. Y cuando está listo, el **Doctor** hace un dry run: cuenta errores y warnings, arregla lo seguro y te pregunta antes de tocar nada considerable.

Misma arquitectura que DistroForge: núcleo puro + generadores deterministas (`FileMap`), proyecto con `openblock.json` de round-trip, zip portable, y una suite que compara la implementación nueva contra la vieja byte a byte. Presentación estilo Minecraft (splash texts, logros, barras de cofre).

## Uso

El binario se instala desde los **Releases de GitHub**: no necesita Rust ni Node.

**Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/Cisne-047/OpenBlock/main/install.sh | sh
```

**Windows** (PowerShell, sin permisos de administrador)

```powershell
irm https://raw.githubusercontent.com/Cisne-047/OpenBlock/main/install.ps1 | iex
```

Deja `forge` en `~/.local/bin` (Linux) o en `%LOCALAPPDATA%\OpenBlock` con el PATH
del usuario (Windows). Si prefieres bajarte el binario a mano, está en
[Releases](https://github.com/Cisne-047/OpenBlock/releases):
`forge-x86_64-unknown-linux-gnu` y `forge-x86_64-pc-windows-msvc.exe`. Los dos
instaladores aceptan la versión (`OPENBLOCK_VERSION=0.2.0`, `-Version 0.2.0`), el
destino (`OPENBLOCK_BIN`) y el de Windows un `-DryRun` que dice qué haría sin bajar nada.

```bash
forge                              # sin subcomando imprime la ayuda; no instala nada
forge guided                       # modo guiado: pregunta y instala
forge install --config mi.json     # o por flags: forge install --engine paper --mc 1.21.4
forge detect                       # hardware + tier + recomendación
forge bench                        # + benchmark real de CPU y disco
forge generate --zip server.zip    # proyecto portable sin instalar
forge doctor --dir ./server        # mini-IA: dry run + fixes
forge backup --dir ./server
forge stop --dir ./server          # apagado limpio por RCON
forge self-test                    # instala un Paper real, lo arranca y lo diagnostica
forge --version
```

Todo por flags, scriptable (`advanced` sigue existiendo como alias de `install`, que
es el nombre que le daba el TypeScript):

```bash
forge install --engine paper --mc 1.21.4 --build 217 --ram-mb 8192 --players 20 \
  --profile balanced --dir ./server --yes
```

El modo avanzado no pregunta nada —`--yes` se acepta por compatibilidad—, que es lo
que permite usarlo desde un script. `--pregen 4000` añade la pregeneración, que
arranca el server de verdad y tarda lo suyo; sin esa flag no se pregenera.

El **TypeScript vive en [`legacy/`](legacy/)** y ya no se distribuye: el paquete de
npm está congelado en la v0.1.0. Sigue siendo la referencia de paridad —el golden lo
genera él— y si quieres usarlo, `npm install && npm run openblock` desde el clon.

Si trabajas sobre el código:

```bash
git clone https://github.com/Cisne-047/OpenBlock.git
cd OpenBlock
cargo build --manifest-path rust/Cargo.toml --release   # → rust/target/release/forge
cargo test  --manifest-path rust/Cargo.toml             # 135 tests, con el golden del TS
npm install && npm test                                 # 101 del TypeScript, el original
npm run parity                                          # regenera el golden y corre el Rust
```

## Builds exactas por engine

Por defecto: última estable. Si quieres clavar una compilación concreta:

- **Paper/Folia/Purpur**: `--build 217` (nº de build)
- **Forge/NeoForge**: `--build 54.0.28` (build del loader)
- **Fabric/Quilt**: `--loader-build 0.16.9 --installer-build 1.0.1`

El modo guiado lista las builds disponibles y eliges por número (Enter = última).

## Plugins/mods custom

```bash
--custom https://ejemplo.com/mi-plugin.jar,C:\ruta\mi-mod.jar
```

O en el guiado, pregunta a pregunta. Los de URL se bajan al server; los locales se copian. Quedan en `openblock.json` (round-trip).

## El Doctor (mini-IA de diagnóstico)

```bash
forge doctor --dir ./server            # boot test real + fixes
forge doctor --dir ./server --no-boot  # solo análisis estático
forge doctor --dir ./server --json     # reporte JSON (scripts/port Rust)
```

1. **Análisis estático**: EULA, `sync-chunk-writes`, contraseña RCON, versión de Java vs MC, puerto ocupado, jar del engine, actualizaciones de plugins en Modrinth.
2. **Dry run**: arranca el server de verdad, captura el log, espera `Done`, apaga limpio por RCON.
3. **Reglas sobre el log**: Java incorrecto, puerto en uso, OOM, plugin roto, `Can't keep up`, migración de mundo, datapacks rotos...
4. **Fixes**: los seguros se aplican solos (EULA, Java correcto, `sync-chunk-writes`, contraseña RCON, actualizar plugin). Los considerables **preguntan** (cambiar puerto, subir RAM, bajar view-distance, desactivar plugin).
5. **Score de salud** 0-100 con barra: SANO / OK CON AVISOS / NECESITA ATENCIÓN / CRÍTICO. Los fixes ya aplicados no restan.

El guiado ofrece el doctor al terminar la instalación (cuando la pregeneración no dejó el server corriendo).

## Qué hace la instalación

1. **Detecta hardware**: CPU, RAM, tipo de disco (NVMe/SSD/HDD), swap, VM; `bench` añade score de CPU multithread y MB/s de disco.
2. **Clasifica en tiers** (patata → entusiasta): engine recomendado (Paper/Purpur/**Folia** con 16+ cores), `-Xmx`, view/simulation distance, radio de pregeneración, GC (Aikar G1 o **ZGC generacional**).
3. **Instala**: engine desde sus APIs oficiales, Java correcto por versión de MC (JDK 8/17/21/25 — MC 26.x exige Java 25), plugins/mods del catálogo (spark, Chunky, Lithium, C2ME, Krypton, FerriteCore, ModernFix...), modpacks **Modrinth** (.mrpack) y CurseForge (zip exportado).
4. **Tunea**: `server.properties`, `paper-global.yml`, `paper-world-defaults.yml`, `spigot.yml`, `bukkit.yml` según perfil conservador/equilibrado/agresivo.
5. **Deja listo**: `start.sh`/`start.bat` (o `run.sh` de Forge con `user_jvm_args.txt`), EULA, firewall, servicio systemd/WinSW, backups rotativos, **pregeneración con Chunky vía RCON**.

## Estructura

```
rust/crates/
  forge-core/   PURO: types, catalog, plan, doctor, rcon, hw/ (detección, tiers,
                bench) y generators/ (jvm, properties, paper, service, scripts, ops)
  forge-api/    red: Mojang, Paper/Purpur/Folia, Fabric/Quilt, Forge/NeoForge,
                Modrinth (+mrpack y CurseForge) y la descarga con hash y reintento
  forge-cli/    sistema: detect, apply (instalar), java, doctor, guided, bench,
                selftest, rcon y el despacho de subcomandos
legacy/
  src/          el TypeScript original (30 ficheros): el espejo de paridad
  tests/        vitest (101) y parity-golden.ts, que es quien genera el golden
tests/fixtures/ config y respuestas de las APIs grabadas, compartidas por los dos
.github/workflows/  ci.yml (3 jobs) y release.yml (los binarios, al publicar un tag)
```

`forge-core` no toca disco, ni red, ni procesos: construye contenido y nada más. La
E/S está en `forge-cli` y la red en `forge-api`, y hay tests que lo comprueban, así
que la regla no depende de que alguien se acuerde. Los 23 ficheros del proyecto
—`openblock.json` incluido— salen idénticos a los del TypeScript.

## Verificación

Dos suites y una comparación:

```bash
cargo test --manifest-path rust/Cargo.toml   # 135: núcleo, APIs y CLI
npm test                                     # 101 del TypeScript, que es el original
npm run parity                               # regenera el golden y vuelve a correr el Rust
cargo run --release -- self-test             # instala un Paper real, lo arranca y lo diagnostica
```

El núcleo Rust se compara **byte a byte** contra el TypeScript: el golden lo genera
el TS y lo consume Rust, en dos sitios (`forge-core/tests/golden`, 51 ficheros, y
`forge-api/tests/golden`, 2). Si alguien cambia un generador en un lado y no en el
otro, el test falla diciendo **el fichero, la línea y el byte**. El CI regenera el
golden y comprueba con `git diff --exit-code` que está al día, así que la paridad no
puede derivar en silencio. El detalle —estado por módulo, divergencias conocidas y lo
que está portado pero no probado— está en [`rust/README.md`](rust/README.md).

`self-test` existe por un motivo concreto: la suite pasa sin tocar un JAR, y por ahí
se coló un bug que 77 tests daban por bueno (`doctor --yes` dejó de aplicar fixes).
El self-test comprueba lo que los tests no pueden: instalación real, arranque real
(`Done`), diagnóstico limpio sobre un server recién forjado, que el modo de solo
lectura no escriba nada, y apagado limpio por RCON. Falla con código de salida
distinto de cero, así que sirve de gate en CI (`.github/workflows/ci.yml`, job
`smoke`). Es el equivalente del smoke test de QEMU que DistroForge corre sobre su ISO.

Opciones: `--mc 1.20.4` (su Java debe existir en la máquina; por defecto 1.20.4, que
va con Java 17), `--port 25599` y `--keep` para no borrar el directorio.

## Por qué existe

DistroForge te forja la distro; OpenBlock te forja el server que corre en ella. Ambos
comparten idea: **config declarativa + generadores puros → proyecto ejecutable**.

DistroForge **ya completó su port a Rust** (v0.4.0): su núcleo es `forge-core`, y la
app React/TS original quedó en `legacy/` como referencia y suite de paridad. Su
paridad no la sostiene un fichero de versión —no lo tiene— sino tres cosas:

1. **Serialización byte-estable**: el orden de campos del struct replica el de la
   interfaz TS, porque `JSON.stringify` y serde serializan en orden de declaración.
2. **Suite espejo**: 77 tests en Rust frente a los 74 del TS, con los mismos
   invariantes (reimportar `distroforge.json`, validar la config por defecto).
3. **Un `diff -r` documentado**: generar el mismo proyecto con las dos
   implementaciones y comparar; byte a byte idénticos.

OpenBlock hizo el mismo camino y la v0.2.0 es el port completo: el binario `forge`
sustituye al CLI de TypeScript, que se queda en `legacy/` generando el golden. Con
una diferencia: aquí la paridad no depende de un `diff -r` manual, la mide la suite
—51 ficheros de golden del núcleo y 2 de las APIs, regenerados en cada CI— y lo que
no puede tener golden (las builds que devuelve PaperMC ese día) se verifica
ejecutando las dos implementaciones contra la misma API en el mismo minuto.
