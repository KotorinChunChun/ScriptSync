#Requires -Version 7.4
param([Parameter(Mandatory)][string]$ArtifactDirectory)
$ErrorActionPreference='Stop'
$entry=Join-Path $PSScriptRoot 'publish-ScriptSync.ps1'
$fixture=Join-Path ([IO.Path]::GetTempPath()) ("scriptsync-publish-"+[guid]::NewGuid())
try {
    & $entry -Mode Plan
    $blocked=$false
    try {& $entry -Mode Publish}catch {if($_ -match 'PublishConfirmed'){$blocked=$true}else{throw}}
    if(!$blocked){throw '公開ガードが作動しません'}
    & $entry -Mode Prepare -ArtifactDirectory $ArtifactDirectory
    & $entry -Mode Stage -ArtifactDirectory $ArtifactDirectory -WebsiteDirectory $fixture
    $files=@(Get-ChildItem (Join-Path $fixture 'www/download/ScriptSync') -File)
    if($files.Count -ne 3){throw '3ファイル限定違反'}
    $package=$files | Where-Object Extension -eq '.msix'
    Set-Content $package.FullName '競合fixture'
    $blocked=$false
    try {& $entry -Mode Stage -ArtifactDirectory $ArtifactDirectory -WebsiteDirectory $fixture}catch {if($_ -match '異なるMSIX'){$blocked=$true}else{throw}}
    if(!$blocked){throw '同版差異ガードが作動しません'}
    '成功: Plan、公開ガード、署名検証、App Installer生成、3ファイルStage、同版差異拒否'
}finally{
    $resolved=[IO.Path]::GetFullPath($fixture)
    if($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()),[StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf).StartsWith('scriptsync-publish-') -and (Test-Path $resolved)){
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
