/**
 * Texto de ayuda de la CLI, en su propio módulo.
 *
 * Vivía dentro de `index.ts`, que además ejecuta `main()` al importarse: sacarlo
 * permite que `advanced.ts` lo muestre en `openblock advanced --help` sin crear
 * un ciclo de imports que arrastraría el arranque entero de la CLI.
 */
export const HELP = `
OpenBlock — forja servidores de Minecraft según tu hardware (Windows/Linux)

  openblock                      Modo guiado (wizard)
  openblock guided               Igual
  openblock advanced [...]       Modo avanzado por flags
      --config openblock.json    base de config reimportable (round-trip)
      --mc 1.21.4 --engine paper|purpur|folia|fabric|quilt|forge|neoforge|vanilla
      --ram-mb 8192 --players 20 --port 25565 --vd 8 --sd 6
      --profile conservative|balanced|aggressive --pregen 4000
      --modpack <url-mrpack> --dir ./server --target linux|windows|both
      --no-service --no-backup --no-rcon --no-pregen
      --dry-run --yes --force
  openblock detect               Hardware + tier + recomendación
  openblock bench [--detect]     Benchmark CPU/disco (~5s)
  openblock generate [...]       Solo genera el proyecto (sin instalar); --zip out.zip
  openblock install [...]        advanced con apply (alias)
  openblock doctor [--dir X]     Mini-IA: dry run del server, errores/warnings,
                                 fixes automáticos, pregunta lo considerable
      --no-boot                  análisis estático sin arrancar
      --yes                      no preguntar (aplica solo lo seguro)
      --json                     reporte JSON (para scripts/port Rust).
                                 Implica solo lectura: no escribe ni instala nada
  openblock info --dir ./server  Config del server instalado
  openblock backup --dir ./server   Ejecuta backup.sh/.ps1 ahora
  openblock stop --dir ./server  Apaga por RCON
  openblock help                 Esta ayuda

Advanced extras: --build N (paper/purpur/folia/forge/neoforge), --loader-build N,
  --installer-build N (fabric/quilt), --custom url1,ruta2 (plugins/mods propios)

Project format: openblock.json round-trip, como DistroForge.
`;
