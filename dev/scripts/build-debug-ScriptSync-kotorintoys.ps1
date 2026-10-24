#Requires -Version 7.0
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location -LiteralPath $source
try {
    & npm.cmd run build
    if ($LASTEXITCODE) { throw 'ScriptSyncの画面ビルドに失敗しました。' }
    # custom-protocolで画面を内包し、Viteサーバー不要の開発EXEとする。
    & cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --locked
    if ($LASTEXITCODE) { throw 'ScriptSyncの管理用ビルドに失敗しました。' }
    $output = Join-Path $source 'target/kotorintoys'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $source 'src-tauri/target/debug/scriptsync.exe') -Destination (Join-Path $output 'scriptsync.exe') -Force
    Write-Output "ScriptSync管理用EXE: $output/scriptsync.exe"
} finally { Pop-Location }
