# OpenBlock

**Forja servidores de Minecraft según tu hardware.** Hermano de [DistroForge](https://github.com/cisne-047/DistroForge) (que forja distros Linux): OpenBlock forja servidores de MC.

- **Requiere**: Node.js 20 o superior, en Windows o Linux
- **Historial de cambios**: [CHANGELOG.md](CHANGELOG.md)
- **Licencia**: MIT para el código; el banner y el nombre quedan fuera ([LICENSE](LICENSE))
- **Estado**: v0.1.0 sin etiquetar, en desarrollo — el repositorio es privado por ahora Detecta tu máquina (Windows/Linux), elige engine, build exacta, RAM, GC, distancias y plugins/mods de rendimiento, instala todo y **deja el server arrancable con un script**. Y cuando está listo, el **Doctor** hace un dry run: cuenta errores y warnings, arregla lo seguro y te pregunta antes de tocar nada considerable.

Misma arquitectura que DistroForge: TypeScript ESM + generadores puros (`FileMap`), proyecto con `openblock.json` de round-trip, zip portable, vitest. Presentación estilo Minecraft (splash texts, logros, barras de cofre).

## Uso

```bash
npm install
npm run openblock            # modo guiado (wizard estilo MC)
```

```bash
# Modo avanzado: todo por flags, scriptable
npm run openblock -- advanced \
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
  hw/               detección (CIM/PowerShell en Windows, /proc+lsblk en Linux), tiers, bench
  api/              Mojang piston-meta, PaperMC fill v3, Purpur, Fabric/Quilt meta,
                    Forge maven+promos, NeoForge maven, Modrinth v2 (+mrpack, CurseForge)
  generators/       jvm.ts (Aikar/ZGC), properties.ts, paper.ts, scripts.ts,
                    service.ts, ops.ts (firewall+backup), index.ts (FileMap+zip)
  plan.ts           manifiesto de descargas (builds exactas + custom items) resuelto por API
  apply.ts          ejecuta: escribe, baja, instala, pregenera por RCON
  doctor.ts         mini-IA: reglas estáticas + de log, boot test, fixes, score
  guided.ts         wizard · advanced.ts flags · index.ts CLI · rcon.ts · ui.ts (estilo MC)
tests/              vitest: tiers, flags Aikar exactos, properties, rcon codec, mrpack, doctor
```

## Por qué existe

DistroForge te forja la distro; OpenBlock te forja el server que corre en ella.
Ambos comparten idea: **config declarativa + generadores puros → proyecto ejecutable**.
Siguiente hito: port conjunto a Rust cuando estén feature-complete (la suite vitest
y el `doctor --json` son el contrato de verificación pre/post port).
