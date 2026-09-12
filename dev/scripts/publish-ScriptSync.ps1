#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('Plan','Prepare','Stage','Publish','Verify')][string]$Mode='Plan',
    [string]$ArtifactDirectory,
    [string]$WebsiteDirectory='C:\develop\excel-chunchun',
    [string]$PythonPath='python',
    [switch]$PublishConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$version=(Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$packageVersion="$version.0"
$base='https://excel-chunchun.sakura.ne.jp/download/beta/'
$names=@("ScriptSync_${packageVersion}_x64.msix",'ScriptSync.cer','ScriptSync.appinstaller')
$site=Join-Path $WebsiteDirectory 'www/download/beta'
if($Mode -eq 'Plan'){
    $names | ForEach-Object { Write-Output "$base$_ -> $(Join-Path $site $_)" }
    return
}
if($Mode -eq 'Publish' -and !$PublishConfirmed){throw '公開の明示指示を受けてから -PublishConfirmed を指定してください。'}
if(!$ArtifactDirectory){throw '署名済み成果物の -ArtifactDirectory を明示してください。'}
$artifact=[IO.Path]::GetFullPath($ArtifactDirectory)
$msix=Join-Path $artifact $names[0]
$cer=Join-Path $artifact $names[1]
# 公開対象の実体、署名者、版を検査。秘密鍵やlayoutは送信しない。
$signature=Get-AuthenticodeSignature -LiteralPath $msix
$certificate=[Security.Cryptography.X509Certificates.X509Certificate2]::new($cer)
try {
    if($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or
       $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint -or
       $certificate.Subject -ne 'CN=KotorinChunChun'){throw 'MSIX署名または公開証明書が一致しません。'}
} finally {$certificate.Dispose()}
$zip=[IO.Compression.ZipFile]::OpenRead($msix)
try {
    $reader=[IO.StreamReader]::new($zip.GetEntry('AppxManifest.xml').Open())
    try {[xml]$manifest=$reader.ReadToEnd()}finally{$reader.Dispose()}
} finally {$zip.Dispose()}
$identity=$manifest.Package.Identity
if($identity.Name -ne 'jp.kotorinchunchun.scriptsync' -or $identity.Version -ne $packageVersion -or
   $identity.Publisher -ne 'CN=KotorinChunChun' -or $identity.ProcessorArchitecture -ne 'x64'){throw 'MSIX Identity・版が一致しません。'}
$installer=@"
<?xml version="1.0" encoding="utf-8"?>
<AppInstaller Uri="${base}ScriptSync.appinstaller" Version="$packageVersion" xmlns="http://schemas.microsoft.com/appx/appinstaller/2018">
  <MainPackage Name="jp.kotorinchunchun.scriptsync" Publisher="CN=KotorinChunChun" Version="$packageVersion" ProcessorArchitecture="x64" Uri="$base$($names[0])" />
  <UpdateSettings><OnLaunch HoursBetweenUpdateChecks="0" /></UpdateSettings>
</AppInstaller>
"@
$appPath=Join-Path $artifact $names[2]
if($Mode -eq 'Prepare'){
    [IO.File]::WriteAllText($appPath,$installer,[Text.UTF8Encoding]::new($false))
    Write-Output "App Installer生成・署名・Identity確認完了: $appPath"
    return
}
if(!(Test-Path $appPath) -or (Get-Content $appPath -Raw) -cne $installer){throw 'Prepareを実行して現在版のApp Installerを生成してください。'}
if($Mode -eq 'Stage'){
    # コピー前に全対象を検査し、版付きMSIXの異なる内容による上書きを防ぐ。
    $destination=Join-Path $site $names[0]
    if((Test-Path $destination) -and (Get-FileHash $destination).Hash -ne (Get-FileHash $msix).Hash){throw '同じ版の異なるMSIXが既にあります。版を更新してください。'}
    New-Item -ItemType Directory -Force $site | Out-Null
    foreach($name in $names){Copy-Item -LiteralPath (Join-Path $artifact $name) -Destination (Join-Path $site $name) -Force}
    Write-Output 'ScriptSyncの3ファイルだけをステージしました。Website側で対象差分を確認してコミットしてください。'
    return
}
if($Mode -eq 'Publish'){
    foreach($name in $names){
        $target=Join-Path $site $name
        if(!(Test-Path $target) -or (Get-FileHash $target).Hash -ne (Get-FileHash (Join-Path $artifact $name)).Hash){throw "ステージ済みファイル不一致: $name"}
    }
    $uploader=Join-Path $WebsiteDirectory 'scripts/upload_ftp.py'
    # パッケージ、公開CER、固定App Installerの順。共有download全体を送信しない。
    foreach($name in $names){
        & $PythonPath $uploader --dry-run (Join-Path $site $name)
        if($LASTEXITCODE -ne 0){throw 'FTP dry-run失敗'}
    }
    foreach($name in $names){
        & $PythonPath $uploader (Join-Path $site $name)
        if($LASTEXITCODE -ne 0){throw "FTP送信失敗: $name"}
    }
}
# Verify/Publish: 公開先を取得し、HTTP、MIME、長さ、内容hashをローカルと比較。
$mime=@('application/msix','application/pkix-cert','application/appinstaller')
for($i=0;$i -lt $names.Count;$i++){
    $name=$names[$i]
    $response=Invoke-WebRequest -Uri "$base$name" -SkipHttpErrorCheck
    if($response.StatusCode -ne 200){throw "HTTP異常: $name"}
    $contentType=([string]$response.Headers['Content-Type']).Split(';')[0].Trim()
    if($contentType -ne $mime[$i]){throw "MIME不一致: $name ($contentType)"}
    $stream=$response.RawContentStream
    $stream.Position=0
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream))
    $local=Get-Item (Join-Path $artifact $name)
    if($hash -ne (Get-FileHash $local.FullName).Hash -or
       [long]([string]$response.Headers['Content-Length']) -ne $local.Length){throw "公開内容不一致: $name"}
    Write-Output "公開検証成功: $base$name SHA256=$hash"
}
