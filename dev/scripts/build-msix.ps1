#Requires -Version 7.4
[CmdletBinding()]
param([string]$OutputDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$package = Get-Content -LiteralPath (Join-Path $source 'package.json') -Raw | ConvertFrom-Json
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $source ("release/msix-v{0}/{1}" -f $package.version,(Get-Date -Format 'yyyyMMdd-HHmmss'))
}
Push-Location $source
try {
    & npm.cmd run tauri build -- --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'リリースビルドに失敗しました。MSIXは作成しません。' }
    & (Join-Path $PSScriptRoot 'package-ScriptSync-msix.ps1') -SourceDirectory $source -ExecutablePath (Join-Path $source 'src-tauri/target/release/scriptsync.exe') -OutputDirectory $OutputDirectory
    if (-not $?) { throw 'MSIXの作成または署名検証に失敗しました。' }
    $revision = & git rev-parse HEAD
    $dirty = @(& git status --porcelain)
    [pscustomobject]@{ version=$package.version; commit=$revision; dirty=($dirty.Count -gt 0); executable_sha256=(Get-FileHash (Join-Path $source 'src-tauri/target/release/scriptsync.exe')).Hash } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'build-info.json') -Encoding utf8
    Write-Host "署名済みMSIX: $OutputDirectory"
} finally { Pop-Location }
