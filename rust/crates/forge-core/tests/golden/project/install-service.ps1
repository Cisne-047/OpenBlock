# Instala el servicio de Windows con WinSW (requiere consola de Administrador)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$winsw = "openblock-paridad.exe"
if (-not (Test-Path $winsw)) {
  Write-Host "Bajando WinSW..."
  Invoke-WebRequest -Uri "https://github.com/winsw/winsw/releases/download/v2.12.0/WinSW-x64.exe" -OutFile $winsw
}
& ".\$winsw" install
& ".\$winsw" start
Write-Host "Servicio instalado: Get-Service openblock-paridad"
