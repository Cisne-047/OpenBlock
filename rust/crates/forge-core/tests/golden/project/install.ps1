# OpenBlock install — paridad
# Descarga el engine, plugins/mods y Java si falta. Requiere PowerShell 5+.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$JavaMajor = 17
$Java = "java"

function Need-Java {
  try { java -version 2>&1 | Out-Null; $true } catch { $true }
  try {
    $v = (& java -version 2>&1 | Out-String) -replace '.*version "(1\.)?(\d+).*','$2'
    return ([int]$v -ne $JavaMajor)
  } catch { return $true }
}

if (Need-Java) {
  Write-Host "== Bajando Temurin JDK $JavaMajor (runtime encapsulado) =="
  $arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x64" }
  $zip = "temurin$JavaMajor.zip"
  Invoke-WebRequest -Uri "https://api.adoptium.net/v3/binary/latest/$JavaMajor/ga/windows/$arch/jdk/hotspot/normal/eclipse" -OutFile $zip -UserAgent "OpenBlock/0.1.0 (+https://github.com/cisne-047)"
  New-Item -ItemType Directory -Force -Path runtime | Out-Null
  Expand-Archive -Path $zip -DestinationPath runtime -Force
  $root = (Get-ChildItem runtime -Directory | Where-Object Name -like "jdk*" | Select-Object -First 1).FullName
  Set-Content -Path runtime\java.path -Value (Join-Path $root "bin\java.exe")
  $Java = (Get-Content runtime\java.path)
  Remove-Item $zip
}

New-Item -ItemType Directory -Force -Path plugins, mods | Out-Null

$downloads = @(
  @{ url = "https://example/paper-1.20.4-100.jar"; dest = "paper-1.20.4-100.jar"; sha = "" }
  @{ url = "https://example/spark.jar"; dest = "plugins/spark.jar"; sha = "" }
)
foreach ($d in $downloads) {
  if (-not $d.url) { continue }
  Write-Host "== Bajando $(Split-Path $d.dest -Leaf) =="
  $dir = Split-Path $d.dest -Parent
  if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  Invoke-WebRequest -Uri $d.url -OutFile $d.dest -UserAgent "OpenBlock/0.1.0 (+https://github.com/cisne-047)"
  if ($d.sha) {
    # hashes de Modrinth son sha512 (128 chars); los de PaperMC, sha256
    $algo = if ($d.sha.Length -gt 64) { "SHA512" } else { "SHA256" }
    $h = (Get-FileHash $d.dest -Algorithm $algo).Hash.ToLower()
    if ($h -ne $d.sha.ToLower()) { throw "Hash no coincide: $($d.dest)" }
  }
}


Write-Host "== Listo. Arranca con: .\start.bat =="
