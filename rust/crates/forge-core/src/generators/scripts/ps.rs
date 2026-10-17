//! Scripts de Windows, portados de `src/generators/scripts.ts`.
//!
//! Igual que en Linux: plantillas largas donde la paridad byte a byte es el único
//! juez. Los `{{` y `}}` son llaves literales de PowerShell que `format!` tendría
//! si no se escaparan.

use crate::catalog::java_major_for;
use crate::plan::DownloadPlan;
use crate::types::{EngineId, ServerConfig};

/// `start.bat` (`startBat`).
pub fn start_bat(cfg: &ServerConfig, plan: &DownloadPlan) -> String {
    let (name, engine, mc) = (&cfg.name, cfg.engine, &cfg.mc_version);
    let jar = &plan.engine_filename;
    let nogui = if cfg.engine == EngineId::Vanilla {
        "nogui"
    } else {
        "--nogui"
    };
    format!(
        r#"@echo off
rem OpenBlock — {name} ({engine} {mc})
setlocal
cd /d "%~dp0"

set "JAVA=java"
if exist runtime\java.path set /p JAVA=<runtime\java.path
set "JVM_ARGS="
if exist jvm-args.txt set /p JVM_ARGS=<jvm-args.txt

rem forge/neoforge: el installer genera su propio launcher (usa user_jvm_args.txt)
if exist run.bat (
  call run.bat nogui
  exit /b %errorlevel%
)

set "JAR={jar}"
if not exist "%JAR%" (
  echo Falta %JAR%. Ejecuta primero install.bat / install.ps1 1>&2
  exit /b 1
)
"%JAVA%" %JVM_ARGS% -jar "%JAR%" {nogui}
"#
    )
}

/// `install.ps1` (`installPs1`).
pub fn install_ps1(cfg: &ServerConfig, plan: &DownloadPlan) -> String {
    let name = &cfg.name;
    let major = java_major_for(&cfg.mc_version);
    let agent = crate::USER_AGENT;
    let list = plan
        .entries
        .iter()
        .map(|e| {
            format!(
                r#"  @{{ url = "{}"; dest = "{}"; sha = "{}" }}"#,
                e.url,
                e.dest,
                e.script_sum()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let installer = match &plan.installer {
        Some(i) => {
            let eng = &i.engine;
            let jar = &i.jar;
            format!(
                r#"Write-Host "== Ejecutando installer de {eng} (descarga librerias, tarda) =="
& $Java -jar "{jar}" --installServer
"#
            )
        }
        None => String::new(),
    };
    format!(
        r#"# OpenBlock install — {name}
# Descarga el engine, plugins/mods y Java si falta. Requiere PowerShell 5+.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$JavaMajor = {major}
$Java = "java"

function Need-Java {{
  try {{ java -version 2>&1 | Out-Null; $true }} catch {{ $true }}
  try {{
    $v = (& java -version 2>&1 | Out-String) -replace '.*version "(1\.)?(\d+).*','$2'
    return ([int]$v -ne $JavaMajor)
  }} catch {{ return $true }}
}}

if (Need-Java) {{
  Write-Host "== Bajando Temurin JDK $JavaMajor (runtime encapsulado) =="
  $arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {{ "aarch64" }} else {{ "x64" }}
  $zip = "temurin$JavaMajor.zip"
  Invoke-WebRequest -Uri "https://api.adoptium.net/v3/binary/latest/$JavaMajor/ga/windows/$arch/jdk/hotspot/normal/eclipse" -OutFile $zip -UserAgent "{agent}"
  New-Item -ItemType Directory -Force -Path runtime | Out-Null
  Expand-Archive -Path $zip -DestinationPath runtime -Force
  $root = (Get-ChildItem runtime -Directory | Where-Object Name -like "jdk*" | Select-Object -First 1).FullName
  Set-Content -Path runtime\java.path -Value (Join-Path $root "bin\java.exe")
  $Java = (Get-Content runtime\java.path)
  Remove-Item $zip
}}

New-Item -ItemType Directory -Force -Path plugins, mods | Out-Null

$downloads = @(
{list}
)
foreach ($d in $downloads) {{
  if (-not $d.url) {{ continue }}
  Write-Host "== Bajando $(Split-Path $d.dest -Leaf) =="
  $dir = Split-Path $d.dest -Parent
  if ($dir) {{ New-Item -ItemType Directory -Force -Path $dir | Out-Null }}
  Invoke-WebRequest -Uri $d.url -OutFile $d.dest -UserAgent "{agent}"
  if ($d.sha) {{
    # hashes de Modrinth son sha512 (128 chars); los de PaperMC, sha256
    $algo = if ($d.sha.Length -gt 64) {{ "SHA512" }} else {{ "SHA256" }}
    $h = (Get-FileHash $d.dest -Algorithm $algo).Hash.ToLower()
    if ($h -ne $d.sha.ToLower()) {{ throw "Hash no coincide: $($d.dest)" }}
  }}
}}

{installer}
Write-Host "== Listo. Arranca con: .\start.bat =="
"#
    )
}
