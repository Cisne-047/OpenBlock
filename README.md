# OpenBlock

**Forja servidores de Minecraft según tu hardware.** Hermano de [DistroForge](https://github.com/cisne-047/DistroForge) (que forja distros Linux): OpenBlock forja servidores de MC.

> **Dónde está cada cosa.** El **producto es el binario de Rust** de [`rust/`](rust/README.md): ahí están el plan, el generador, el doctor y toda la CLI (`forge`). El **TypeScript vive en [`legacy/`](legacy/)** como referencia y **espejo de paridad**: el golden lo sigue generando él, así que cada byte de lo que produce el Rust se sigue comparando contra el original. El paquete de npm quedó **congelado en v0.1.0** (`legacy/package.json` es privado): la distribución del binario es el siguiente paso y está anotada en [`rust/README.md`](rust/README.md).
>
> Lo de abajo describe el comportamiento del CLI, que es el mismo en los dos: `forge <comando>` en Rust, `openblock <comando>` en el TypeScript de `legacy/`.

- **Requiere**: el binario (no necesita Node). El TypeScript de `legacy/`, si quieres usarlo, pide Node.js 20 o superior. Java se gestiona solo (OpenBlock se descarga el JDK que pida la versión de MC), aunque para el `self-test` conviene tener uno a mano
- **Historial de cambios**: [CHANGELOG.md](CHANGELOG.md)
- **Licencia**: MIT para el código; el banner y el nombre quedan fuera ([LICENSE](LICENSE))
- **Estado**: v0.1.0 publicada en npm (TypeScript); el port a Rust está completo y verificado

Detecta tu máquina (Windows/Linux), elige engine, build exacta, RAM, GC, distancias y plugins/mods de rendimiento, instala todo y **deja el server arrancable con un script**. Y cuando está listo, el **Doctor** hace un dry run: cuenta errores y warnings, arregla lo seguro y te pregunta antes de tocar nada considerable.

Misma arquitectura que DistroForge: TypeScript ESM + generadores puros (`FileMap`), proyecto con `openblock.json` de round-trip, zip portable, vitest. Presentación estilo Minecraft (splash texts, logros, barras de cofre).

## Uso

```bash
npm install -g openblock
openblock                    # modo guiado (wizard estilo MC)
```

Si trabajas sobre el código, en vez de instalar:

```bash
git clone https://github.com/Cisne-047/OpenBlock.git
cd OpenBlock
npm install
npm run openblock            # ejecuta el TS directamente, sin compilar
npm run build                # compila a dist/ (es lo que se publica)
```

**Instalar desde el clon no funciona con npm 12.** `npm install -g ./OpenBlock`
no produce el comando: el clon no trae `dist/` y npm 12 bloquea los scripts de
instalación (`install-scripts blocked`), así que nada lo compila. Por eso el
paquete se publica compilado: el tarball ya lleva `dist/`, no necesita `tsx` ni
compilar en tu máquina. Si vienes del clon, haz `npm run build` antes.

```bash
# Modo avanzado: todo por flags, scriptable
openblock advanced \
  --engine paper --mc 1.21.4 --build 217 --ram-mb 8192 --players 20 \
  --pregen 4000 --profile balanced --dir ./server --yes
```

```bash
openblock detect             # hardware + tier + recomendación
openblock bench              # + benchmark real de CPU y disco
openblock generate --zip server.zip    # proyecto portable sin instalar
openblock doctor --dir ./server       # mini-IA: dry run + fixes
openblock backup --dir ./server
openblock stop   --dir ./server       # apagado limpio por RCON
openblock self-test          # instala un Paper real, lo arranca y lo diagnostica
npm test
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
openblock doctor --dir ./server            # boot test real + fixes
openblock doctor --dir ./server --no-boot  # solo análisis estático
openblock doctor --dir ./server --json     # reporte JSON (scripts/port Rust)
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
src/
  catalog.ts        engines, plugins/mods por loader+versión, mapa Java/MC
  version.ts        fuente única de la versión (banner, user agent, scripts)
  hw/               detección (CIM/PowerShell en Windows, /proc+lsblk en Linux), tiers, bench
  api/              Mojang piston-meta, PaperMC fill v3, Purpur, Fabric/Quilt meta,
                    Forge maven+promos, NeoForge maven, Modrinth v2 (+mrpack, CurseForge)
  generators/       PURO: jvm.ts (Aikar/ZGC), properties.ts, paper.ts, scripts.ts,
                    service.ts, ops.ts (firewall+backup), index.ts (FileMap)
  output.ts         E/S: escribir el FileMap a disco y empaquetarlo en zip
  plan.ts           manifiesto de descargas (builds exactas + custom items) resuelto por API
  apply.ts          ejecuta: escribe, baja, instala, pregenera por RCON
  doctor.ts         mini-IA: reglas estáticas + de log, boot test, fixes, score
  selftest.ts       self-test: instala un Paper real, lo arranca y lo diagnostica
  cli.ts            parseo de flags y política de escritura del doctor
  guided.ts         wizard · advanced.ts flags · index.ts CLI · rcon.ts · ui.ts (estilo MC)
tests/              vitest (86): tiers, flags Aikar exactos, properties, rcon codec,
                    mrpack, doctor, pureza del núcleo, veredicto del self-test y
                    barrido de "undefined"
dist/               JS compilado (lo que se instala); no se versiona
```

`generators/` no importa `node:*` ni lanza: construye contenido y nada más. La E/S
está en `output.ts` y `apply.ts`. `tests/purity.test.ts` lo comprueba, así que la
regla no depende de que alguien se acuerde.

El código se escribe en TypeScript y **se publica compilado**: `npm run build`
emite `dist/` y el `bin` apunta ahí, así que el paquete instalado no necesita
`tsx` ni ningún cargador. Por eso los imports relativos llevan extensión `.js`
(aunque el fichero sea `.ts`): es lo que exige Node ESM y lo que hace que el JS
emitido se resuelva sin ayuda.

## Verificación

Dos niveles, porque la suite sola no basta:

```bash
npm test                # 86 tests: unitarios, sin arrancar ningún server
npm run smoke           # compila + self-test: instala, arranca y diagnostica un Paper real
```

`self-test` existe por un motivo concreto: la suite pasa sin tocar un JAR, y por
ahí se coló un bug que 77 tests daban por bueno (`doctor --yes` dejó de aplicar
fixes). El self-test comprueba lo que los tests no pueden: instalación real,
arranque real (`Done`), diagnóstico limpio sobre un server recién forjado, que el
modo de solo lectura no escriba nada, y apagado limpio por RCON. Falla con código
de salida distinto de cero, así que sirve de gate en CI
(`.github/workflows/ci.yml`, job `smoke`). Es el equivalente del smoke test de
QEMU que DistroForge corre sobre su ISO.

Opciones: `--mc 1.20.4` (su Java debe existir en la máquina; por defecto 1.20.4,
que va con Java 17), `--port 25599` y `--keep` para no borrar el directorio.

## Por qué existe

DistroForge te forja la distro; OpenBlock te forja el server que corre en ella.
Ambos comparten idea: **config declarativa + generadores puros → proyecto ejecutable**.

DistroForge **ya completó su port a Rust** (v0.4.0): su núcleo es `forge-core`, y
la app React/TS original quedó en `legacy/` como referencia y suite de paridad.
Su paridad no la sostiene un fichero de versión —no lo tiene— sino tres cosas:

1. **Serialización byte-estable**: el orden de campos del struct replica el de la
   interfaz TS, porque `JSON.stringify` y serde serializan en orden de declaración.
2. **Suite espejo**: 77 tests en Rust frente a los 74 del TS, con los mismos
   invariantes (reimportar `distroforge.json`, validar la config por defecto).
3. **Un `diff -r` documentado**: generar el mismo proyecto con las dos
   implementaciones y comparar; byte a byte idénticos.

OpenBlock va por ese camino: núcleo puro, generadores deterministas y
`doctor --json` como contrato de verificación. El port está en `rust/`, con su
propio [`rust/README.md`](rust/README.md) —estado por módulo, divergencias
conocidas y la receta para seguir—. Lo publicado sigue siendo el TypeScript.

