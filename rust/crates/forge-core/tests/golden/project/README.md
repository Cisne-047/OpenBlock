# paridad

Server de Minecraft forjado con **OpenBlock** — Paper 1.20.4
(build 100)

## Arrancar

- **Windows:** `start.bat`
- **Linux:** `./start.sh`

Si copiaste este proyecto a otra máquina sin instalar nada, ejecuta antes:

- **Windows:** `install.ps1` (PowerShell)
- **Linux:** `./install.sh`

## Qué lleva dentro

- Engine: Paper (Fork de Spigot con miles de fixes y optimizaciones. El estándar.)
- RAM asignada: 8192 MB — flags: `jvm-args.txt`
- Puerto: 25565 — jugadores: 24 — view-distance: 8 / simulation: 6
- Perfil de rendimiento: aggressive

### Plugins/mods de rendimiento

- spark (plugin)

## Servicio

- **Linux:** `sudo ./install-service.sh` (systemd) — `systemctl status openblock-paridad`
- **Windows:** `install-service.ps1` como Administrador (WinSW)

## Firewall

- **Linux:** `sudo ./firewall.sh`
- **Windows:** `firewall.ps1` como Administrador

## Backups

- Manual: `./backup.sh` / `backup.ps1` — guarda mundos + configs en `backups/`, rota 7.
- Linux: instala el timer con `sudo cp openblock-backup.timer /etc/systemd/system/ && sudo systemctl enable --now openblock-backup.timer`.

## Config round-trip

Este proyecto lleva `openblock.json`: reimporta la config en OpenBlock o
regenera el server tras cambiarla.
