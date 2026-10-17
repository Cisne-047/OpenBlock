@echo off
rem OpenBlock — paridad (paper 1.20.4)
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

set "JAR=paper-1.20.4-100.jar"
if not exist "%JAR%" (
  echo Falta %JAR%. Ejecuta primero install.bat / install.ps1 1>&2
  exit /b 1
)
"%JAVA%" %JVM_ARGS% -jar "%JAR%" --nogui
