#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('Plan','Prepare','Stage','DryRun','Publish','Verify')][string]$Mode='Plan',
    [string]$ArtifactDirectory,
    [string]$WebsiteDirectory='C:\develop\excel-chunchun',
    [string]$PythonPath='python',
    [switch]$PublishConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$version=(Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
. (Join-Path $PSScriptRoot "msix-version.ps1")
$packageVersion=Get-ScriptSyncMsixVersion $root
$base='https://excel-chunchun.sakura.ne.jp/download/ScriptSync/'
$names=@("ScriptSync_${packageVersion}_x64.msix",'ScriptSync.cer','ScriptSync.appinstaller','index.html','icon.png')
$site=Join-Path $WebsiteDirectory 'www/download/ScriptSync'
$siteKey=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath($site).ToLowerInvariant())))
if($Mode -eq 'Plan'){
    $names | ForEach-Object { Write-Output "$base$_ -> $(Join-Path $site $_)" }
    return
}
if($Mode -eq 'Publish' -and !$PublishConfirmed){throw '公開の明示指示を受けてから -PublishConfirmed を指定してください。'}
if(!$ArtifactDirectory){throw '署名済み成果物の -ArtifactDirectory を明示してください。'}
$artifact=[IO.Path]::GetFullPath($ArtifactDirectory)
$baseline=Join-Path $artifact "publish-baselines/$siteKey.json"
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
    # OPC/MSIXは@や+をパート名でpercent-encodeする。インストール先の名前へ戻して照合する。
    $parts=@{}
    foreach($part in $zip.Entries){
        $name=[Uri]::UnescapeDataString($part.FullName)
        if($parts.ContainsKey($name)){throw 'MSIXに重複したパート名があります。'}
        $parts[$name]=$part
    }
    if(!$zip.GetEntry('LICENSE.txt') -or !$zip.GetEntry('THIRD-PARTY-NOTICES.md')){throw 'ライセンス文書を同梱したMSIXを作成してください。'}
    $manifestPath=Join-Path $root 'licenses/third-party/manifest.json'
    $expectedLicenseManifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    foreach($expected in @([pscustomobject]@{path='manifest.json';sha256=(Get-FileHash $manifestPath).Hash})+@($expectedLicenseManifest.files)){
        $entry=$parts['licenses/'+$expected.path]
        if(!$entry){throw "対応ソースまたはライセンスがMSIXにありません: $($expected.path)"}
        $stream=$entry.Open()
        try {$actual=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream))}finally{$stream.Dispose()}
        if($actual -ne $expected.sha256){throw "MSIXのライセンスと依存構成が一致しません: $($expected.path)"}
    }
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
    Copy-Item -LiteralPath (Join-Path $root 'dev/release-page/index.html') -Destination (Join-Path $artifact 'index.html') -Force
    Copy-Item -LiteralPath (Join-Path $root 'dev/release-page/icon.png') -Destination (Join-Path $artifact 'icon.png') -Force
    Write-Output "App Installer生成・署名・Identity確認完了: $appPath"
    return
}
if(!(Test-Path $appPath) -or (Get-Content $appPath -Raw) -cne $installer){throw 'Prepareを実行して現在版のApp Installerを生成してください。'}
if($Mode -eq 'Stage'){
    # コピー前に全対象を検査し、版付きMSIXの異なる内容による上書きを防ぐ。
    $destination=Join-Path $site $names[0]
    if((Test-Path $destination) -and (Get-FileHash $destination).Hash -ne (Get-FileHash $msix).Hash){throw '同じ版の異なるMSIXが既にあります。版を更新してください。'}
    if(!(Test-Path $baseline)){
        $previous=[ordered]@{}
        foreach($name in $names){$file=Join-Path $site $name;$previous[$name]=if(Test-Path $file){(Get-FileHash $file).Hash.ToLowerInvariant()}else{$null}}
        New-Item -ItemType Directory -Force (Split-Path $baseline) | Out-Null
        $previous | ConvertTo-Json | Set-Content -LiteralPath $baseline -Encoding utf8
    }
    New-Item -ItemType Directory -Force $site | Out-Null
    foreach($name in $names){Copy-Item -LiteralPath (Join-Path $artifact $name) -Destination (Join-Path $site $name) -Force}
    Write-Output 'ScriptSyncの5ファイルだけをステージしました。Website側で対象差分を確認してコミットしてください。'
    return
}
if($Mode -in 'DryRun','Publish'){
    foreach($name in $names){
        $target=Join-Path $site $name
        if(!(Test-Path $target) -or (Get-FileHash $target).Hash -ne (Get-FileHash (Join-Path $artifact $name)).Hash){throw "ステージ済みファイル不一致: $name"}
    }
    if(!(Test-Path $baseline)){throw 'Stageで公開前の基準を記録してください。'}
    $uploader=Join-Path $PSScriptRoot 'publish-ScriptSync-ftps.py'
    $backup=Join-Path $root "dev/history/v$packageVersion/logs/ftps-publication"
    $arguments=@($uploader,'--website',$WebsiteDirectory,'--artifact',$artifact,'--baseline',$baseline,'--backup-directory',$backup)
    & $PythonPath @arguments
    if($LASTEXITCODE -ne 0){throw 'FTPS DryRun失敗'}
    if($Mode -eq 'DryRun'){return}
    & $PythonPath @arguments --execute
    if($LASTEXITCODE -ne 0){throw 'FTPS公開失敗。退避と読戻し結果を確認してください。'}
}
# Verify/Publish: 公開先を取得し、HTTP、MIME、長さ、内容hashをローカルと比較。
$mime=@('application/msix','application/pkix-cert','application/appinstaller','text/html','image/png')
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
