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

### Cambiado

- **Andamiaje de repositorio heredado de DistroForge** (17/10/2026), adaptado a un CLI TypeScript y no al stack Rust:
  - `.github/workflows/ci.yml`: type-check + suite en Node 20. DistroForge corre cuatro jobs porque tiene cuatro artefactos (core Rust, servidor, UI y legacy); aquí hay uno solo.
  - `LICENSE` MIT con la **excepción de identidad** que DistroForge estrenó: la marca (aquí el banner ASCII y el nombre) queda fuera de la licencia. Se añade el aviso de no afiliación con Mojang.
  - `CHANGELOG.md` con el mismo formato en español (Añadido / Cambiado / Arreglado / Pendiente) y la aclaración, que DistroForge ya hace, de que lo listado aún **no está etiquetado**.
  - `.gitattributes` y `.editorconfig` nuevos. El primero **no es cosmético**: los `start.sh` generados llevan shebang y se rompen con CRLF, así que `*.sh` queda forzado a LF y `*.ps1`/`*.bat` a CRLF.
  - `.gitignore` ampliado: además de `node_modules/` y `dist/`, ignora `server/`, `openblock-work/` y `.env*` para que no se versionen mundos, EULA ni contraseñas de RCON.

### Pendiente

- **Port a Rust**: el hito compartido con DistroForge. DistroForge ya lo completó (`rust/crates/forge-core`) manteniendo `legacy/` como suite de paridad; OpenBlock sigue en TypeScript y el `doctor --json` es el contrato previsto para verificar el port.
- **Etiquetar la v0.1.0** y decidir si se publica en npm (`bin`/`files` en `package.json` aún sin definir).
- **Endurecer la CLI**: el parseo de flags de `info`/`backup`/`stop`/`doctor` es manual (`rest.indexOf("--dir")`), no comparte el `parseArgs` de `modo avanzado` y falla en silencio con un flag mal escrito.
- **Versionar el formato del proyecto**: `openblock.json` no lleva campo de versión; un fichero viejo se lee como si fuera actual.

## Versiones anteriores

- Ninguna: v0.1.0 es la primera.
