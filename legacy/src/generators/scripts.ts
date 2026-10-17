import type { DownloadPlan } from "../plan.js";
import type { ServerConfig } from "../types.js";
import { USER_AGENT } from "../version.js";

/**
 * Scripts de arranque e instalación autocontenidos: el server generado se
 * ejecuta igual en Windows (start.bat) y Linux (start.sh), y el zip del
 * proyecto se puede instalar en otra máquina sin tener OpenBlock encima.
 */

const SHELL_QUOTE = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;

export function startSh(cfg: ServerConfig, plan: DownloadPlan): string {
  return `#!/usr/bin/env bash
# OpenBlock — ${cfg.name} (${cfg.engine} ${cfg.mcVersion})
set -euo pipefail
cd "$(dirname "$0")"

JAVA="java"
[ -f runtime/java.path ] && JAVA="$(cat runtime/java.path)"
JVM_ARGS="$(cat jvm-args.txt 2>/dev/null || true)"

# forge/neoforge: el installer genera su propio launcher (usa user_jvm_args.txt)
if [ -f run.sh ]; then
  exec bash run.sh nogui
fi

JAR="${plan.engineFilename}"
if [ ! -f "$JAR" ]; then
  echo "Falta $JAR. Ejecuta primero ./install.sh" >&2
  exit 1
fi
exec "$JAVA" $JVM_ARGS -jar "$JAR" ${cfg.engine === "vanilla" ? "nogui" : "--nogui"}
`;
}

export function startBat(cfg: ServerConfig, plan: DownloadPlan): string {
  return `@echo off
rem OpenBlock — ${cfg.name} (${cfg.engine} ${cfg.mcVersion})
setlocal
cd /d "%~dp0"

set "JAVA=java"
if exist runtime\\java.path set /p JAVA=<runtime\\java.path
set "JVM_ARGS="
if exist jvm-args.txt set /p JVM_ARGS=<jvm-args.txt

rem forge/neoforge: el installer genera su propio launcher (usa user_jvm_args.txt)
if exist run.bat (
  call run.bat nogui
  exit /b %errorlevel%
)

set "JAR=${plan.engineFilename}"
if not exist "%JAR%" (
  echo Falta %JAR%. Ejecuta primero install.bat / install.ps1 1>&2
  exit /b 1
)
"%JAVA%" %JVM_ARGS% -jar "%JAR%" ${cfg.engine === "vanilla" ? "nogui" : "--nogui"}
`;
}

export function jvmArgsTxt(line: string): string {
  return line + "\n";
}

export function userJvmArgsTxt(line: string): string {
  return line + "\n";
}

function shDownloads(plan: DownloadPlan): string {
  const urls: string[] = [];
  const dests: string[] = [];
  const sums: string[] = [];
  for (const e of plan.entries) {
    urls.push(SHELL_QUOTE(e.url));
    dests.push(SHELL_QUOTE(e.dest));
    sums.push(SHELL_QUOTE(e.sha256 ?? e.sha512 ?? ""));
  }
  return `URLS=(${urls.join(" ")})
DESTS=(${dests.join(" ")})
SUMS=(${sums.join(" ")})`;
}

export function installSh(cfg: ServerConfig, plan: DownloadPlan, javaMajor: number): string {
  return `#!/usr/bin/env bash
# OpenBlock install — ${cfg.name}
# Descarga el engine, plugins/mods y Java si falta. Requiere curl y tar.
set -euo pipefail
cd "$(dirname "$0")"

JAVA_MAJOR=${javaMajor}
JAVA="java"

need_java() {
  command -v java >/dev/null 2>&1 || return 0
  V="$(java -version 2>&1 | sed -n 's/.*version "\\(1\\.\\)\\?\\([0-9]*\\).*/\\2/p')"
  [ "$V" != "$JAVA_MAJOR" ] && return 0
  return 1
}

if need_java; then
  echo "== Bajando Temurin JDK $JAVA_MAJOR (runtime encapsulado) =="
  ARCH="$(uname -m)"; [ "$ARCH" = "aarch64" ] && A=aarch64 || A=x64
  TARBALL="OpenJDK${javaMajor}-jdk_\$A.tar.gz"
  curl -fL --retry 2 -o "$TARBALL" \\
    "https://api.adoptium.net/v3/binary/latest/$JAVA_MAJOR/ga/linux/\$A/jdk/hotspot/normal/eclipse"
  mkdir -p runtime
  tar -xzf "$TARBALL" -C runtime
  ROOT="$(find runtime -maxdepth 1 -type d -name 'jdk*' | head -1)"
  chmod +x "$ROOT/bin/java" "$ROOT/bin/keytool" 2>/dev/null || true
  echo "$ROOT/bin/java" > runtime/java.path
  JAVA="$(cat runtime/java.path)"
  rm -f "$TARBALL"
fi

mkdir -p plugins mods
${shDownloads(plan)}
for i in "\${!URLS[@]}"; do
  URL="\${URLS[$i]}"; DEST="\${DESTS[$i]}"; SUM="\${SUMS[$i]}"
  [ -z "$URL" ] && continue
  echo "== Bajando \${DEST##*/} =="
  mkdir -p "$(dirname "$DEST")"
  curl -fL --retry 2 -o "$DEST" "$URL"
  if [ -n "$SUM" ]; then
    # los hashes de Modrinth son sha512 (64+ chars); los de PaperMC, sha256
    if [ "\${#SUM}" -gt 64 ]; then CHK=sha512sum; else CHK=sha256sum; fi
    echo "$SUM  $DEST" | "$CHK" -c - >/dev/null \\
      || { echo "Hash no coincide: $DEST" >&2; exit 1; }
  fi
done

${plan.installer
      ? `echo "== Ejecutando installer de ${plan.installer.engine} (descarga librerías, tarda) =="
"$JAVA" -jar "${plan.installer.jar}" --installServer
`
      : ""}
chmod +x start.sh
echo "== Listo. Arranca con: ./start.sh =="
`;
}

export function installPs1(cfg: ServerConfig, plan: DownloadPlan, javaMajor: number): string {
  const list = plan.entries
    .map((e) => `  @{ url = "${e.url}"; dest = "${e.dest}"; sha = "${e.sha256 ?? e.sha512 ?? ""}" }`)
    .join("\n");
  return `# OpenBlock install — ${cfg.name}
# Descarga el engine, plugins/mods y Java si falta. Requiere PowerShell 5+.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$JavaMajor = ${javaMajor}
$Java = "java"

function Need-Java {
  try { java -version 2>&1 | Out-Null; $true } catch { $true }
  try {
    $v = (& java -version 2>&1 | Out-String) -replace '.*version "(1\\.)?(\\d+).*','$2'
    return ([int]$v -ne $JavaMajor)
  } catch { return $true }
}

if (Need-Java) {
  Write-Host "== Bajando Temurin JDK $JavaMajor (runtime encapsulado) =="
  $arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x64" }
  $zip = "temurin$JavaMajor.zip"
  Invoke-WebRequest -Uri "https://api.adoptium.net/v3/binary/latest/$JavaMajor/ga/windows/$arch/jdk/hotspot/normal/eclipse" -OutFile $zip -UserAgent "${USER_AGENT}"
  New-Item -ItemType Directory -Force -Path runtime | Out-Null
  Expand-Archive -Path $zip -DestinationPath runtime -Force
  $root = (Get-ChildItem runtime -Directory | Where-Object Name -like "jdk*" | Select-Object -First 1).FullName
  Set-Content -Path runtime\\java.path -Value (Join-Path $root "bin\\java.exe")
  $Java = (Get-Content runtime\\java.path)
  Remove-Item $zip
}

New-Item -ItemType Directory -Force -Path plugins, mods | Out-Null

$downloads = @(
${list}
)
foreach ($d in $downloads) {
  if (-not $d.url) { continue }
  Write-Host "== Bajando $(Split-Path $d.dest -Leaf) =="
  $dir = Split-Path $d.dest -Parent
  if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  Invoke-WebRequest -Uri $d.url -OutFile $d.dest -UserAgent "${USER_AGENT}"
  if ($d.sha) {
    # hashes de Modrinth son sha512 (128 chars); los de PaperMC, sha256
    $algo = if ($d.sha.Length -gt 64) { "SHA512" } else { "SHA256" }
    $h = (Get-FileHash $d.dest -Algorithm $algo).Hash.ToLower()
    if ($h -ne $d.sha.ToLower()) { throw "Hash no coincide: $($d.dest)" }
  }
}

${plan.installer
      ? `Write-Host "== Ejecutando installer de ${plan.installer.engine} (descarga librerias, tarda) =="
& $Java -jar "${plan.installer.jar}" --installServer
`
      : ""}
Write-Host "== Listo. Arranca con: .\\start.bat =="
`;
}
