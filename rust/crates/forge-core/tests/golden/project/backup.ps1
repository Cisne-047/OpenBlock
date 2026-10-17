# Backup de mundos + configs a backups\, rota los últimos 7.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
New-Item -ItemType Directory -Force -Path backups | Out-Null
$targets = @("world","world_nether","world_the_end","server.properties","paper-global.yml","paper-world-defaults.yml","spigot.yml","bukkit.yml") | Where-Object { Test-Path $_ }
$dest = "backups\backup-$stamp.zip"
Compress-Archive -Path $targets -DestinationPath $dest
Get-ChildItem backups\backup-*.zip | Sort-Object LastWriteTime -Descending | Select-Object -Skip 7 | Remove-Item -Force
Write-Host "Backup creado: $dest"
