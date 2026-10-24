#Requires -Version 7.4
[CmdletBinding()]
param([switch]$Native,[switch]$AudioDevice)
$ErrorActionPreference='Stop'
$source=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$version=Get-Content (Join-Path $source 'dev/msix-version.json') -Raw | ConvertFrom-Json
$distribution="$($version.applicationVersion).$($version.revision)"
$logs=Join-Path $source "dev/history/v$distribution/logs/verify-$(Get-Date -Format yyyyMMdd-HHmmss)-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Path $logs -Force | Out-Null
if($AudioDevice -and !$Native){throw '-AudioDeviceは-Nativeと一緒に指定してください。'}
$cargo=Get-Command cargo -ErrorAction SilentlyContinue
if(!$cargo){
 $candidates=@((Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'),(Join-Path $env:USERPROFILE 'scoop/apps/rustup-msvc/current/.cargo/bin/cargo.exe'))
 if($env:CARGO_HOME){$candidates=@((Join-Path $env:CARGO_HOME 'bin/cargo.exe'))+$candidates}
 $found=$candidates | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
 if(!$found){throw 'Rust/Cargoを導入してPATHへ追加してください。'}
 $env:CARGO_HOME=Split-Path (Split-Path $found)
 $rustup=Join-Path (Split-Path $env:CARGO_HOME) '.rustup'
 if(Test-Path -LiteralPath $rustup){$env:RUSTUP_HOME=$rustup}
 $env:PATH="$(Split-Path $found);$env:PATH"
}
function Invoke-Test([string]$Name,[string]$Command,[string[]]$Arguments){
 Write-Host "検証: $Name"
 & $Command @Arguments *> (Join-Path $logs "$Name.log")
 if($LASTEXITCODE -ne 0){Get-Content (Join-Path $logs "$Name.log") -Tail 35;throw "失敗: $Name / $logs"}
}
Push-Location $source
try{
 Invoke-Test 'static' 'npm.cmd' @('run','check:static','--','--release-page')
 Invoke-Test 'security-tests' 'npm.cmd' @('run','test:security')
 Invoke-Test 'source-history' 'npm.cmd' @('run','check:security','--','--history')
 Invoke-Test 'dependency-audit' 'npm.cmd' @('audit','--audit-level=low')
 Invoke-Test 'ui' 'npm.cmd' @('test','--','--maxWorkers=2')
 Invoke-Test 'frontend' 'npm.cmd' @('run','build')
 Invoke-Test 'rust' 'cargo' @('test','--manifest-path','src-tauri/Cargo.toml','--locked')
 Invoke-Test 'recorder' 'cargo' @('test','--manifest-path','crates/scriptsync-recorder/Cargo.toml','--locked','--target-dir','src-tauri/target')
 Invoke-Test 'recorder-no-mp3' 'cargo' @('test','--manifest-path','crates/scriptsync-recorder/Cargo.toml','--locked','--no-default-features','--target-dir','src-tauri/target')
 Invoke-Test 'licenses' 'npm.cmd' @('run','licenses:check')
 if($Native){
  Invoke-Test 'native-build' 'pwsh' @('-NoProfile','-File','dev/scripts/build-debug-ScriptSync-kotorintoys.ps1')
  $options=@('dev/scripts/test-native.mjs');if($AudioDevice){$options+='--with-audio-device'}
  Invoke-Test 'native' 'node' $options
 }
 "成功: $logs"
}finally{Pop-Location}
