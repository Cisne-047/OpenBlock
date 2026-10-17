<#
.SYNOPSIS
    Instala el binario de OpenBlock (Rust) en Windows, desde un Release de GitHub.

.DESCRIPTION
    No necesita Rust ni Node. Descarga el binario de la última versión y lo deja en
    %LOCALAPPDATA%\OpenBlock, que se añade al PATH del usuario (el de tu cuenta, no
    el del sistema: no hace falta ser administrador).

    Se puede lanzar sin clonar el repo:

        irm https://raw.githubusercontent.com/Cisne-047/OpenBlock/main/install.ps1 | iex

.PARAMETER DryRun
    No descarga nada: dice qué URL usaría y dónde lo dejaría.

.PARAMETER Version
    Versión a instalar (por defecto, la última).

.NOTES
    Para quien edite este fichero: va en UTF-8 **con BOM**, y el BOM es necesario.
    Windows PowerShell 5.1 lee los .ps1 sin BOM como ANSI, así que los bytes de
    `✓`, `↓` y las vocales acentuadas se convierten en comillas tipográficas —que
    PowerShell acepta como delimitador de cadena— y el script deja de compilar con
    un error que ni siquiera señala la línea buena. Con BOM lo lee bien, y la ruta
    documentada (`irm … | iex`, que decodifica por el charset del HTTP) no se entera
    de que está.

.EXAMPLE
    .\install.ps1
    .\install.ps1 -DryRun
#>
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$repo = "Cisne-047/OpenBlock"
$asset = "forge-x86_64-pc-windows-msvc.exe"

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "solo hay binarios para x86_64 y este equipo es $($env:PROCESSOR_ARCHITECTURE)"
}

$url = if ($Version -eq "latest") {
    "https://github.com/$repo/releases/latest/download/$asset"
} else {
    "https://github.com/$repo/releases/download/v$Version/$asset"
}

$destino = Join-Path $env:LOCALAPPDATA "OpenBlock"
$forge = Join-Path $destino "forge.exe"

Write-Host "↓ $url"
if ($DryRun) {
    Write-Host "(dry-run) se instalaría en $forge"
    return
}

New-Item -ItemType Directory -Force -Path $destino | Out-Null
# se descarga a un temporal y se mueve: si se corta, no queda un forge.exe a medias
$tmp = "$forge.descargando"
Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
Move-Item -Force -Path $tmp -Destination $forge

# el PATH del usuario: no toca el del sistema y no pide permisos de administrador
$pathUsuario = [Environment]::GetEnvironmentVariable("Path", "User")
if ($pathUsuario -notlike "*$destino*") {
    [Environment]::SetEnvironmentVariable("Path", "$pathUsuario;$destino", "User")
    Write-Host "✓ $destino añadido al PATH de tu usuario (abre una consola nueva para verlo)"
}

Write-Host "✓ forge instalado en $forge"
& $forge --version
Write-Host ""
Write-Host "  forge help    para ver los comandos"
Write-Host "  forge doctor  para diagnosticar un server que ya tengas"
