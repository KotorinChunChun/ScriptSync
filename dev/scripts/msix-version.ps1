# MSIXの末尾はアプリ版と独立した再配布番号。
function Get-ScriptSyncMsixVersion([string]$SourceDirectory) {
    $app=(Get-Content (Join-Path $SourceDirectory 'package.json') -Raw | ConvertFrom-Json).version
    $release=Get-Content (Join-Path $SourceDirectory 'dev/msix-version.json') -Raw | ConvertFrom-Json
    if($release.applicationVersion -ne $app -or $app -notmatch '^\d+\.\d+\.\d+$' -or
       [string]$release.revision -notmatch '^\d+$' -or [long]$release.revision -gt 65535){
        throw 'MSIX版設定が不正です。アプリ版と再配布番号を確認してください。'
    }
    return "$app.$($release.revision)"
}
