#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDirectory,
    [Parameter(Mandatory)][string]$ExecutablePath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'MsixSigning.psm1') -Force
$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$exe = (Resolve-Path -LiteralPath $ExecutablePath).Path
$config = Get-Content -LiteralPath (Join-Path $source 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$package = Get-Content -LiteralPath (Join-Path $source 'package.json') -Raw | ConvertFrom-Json
if ($config.version -ne $package.version -or $config.version -notmatch '^\d+\.\d+\.\d+$') { throw '版番号が一致しません。' }
$version = "$($config.version).0"
$exeVersion = [version](Get-Item -LiteralPath $exe).VersionInfo.FileVersion
if ($exeVersion -ne [version]$version -and $exeVersion -ne [version]$config.version) { throw 'EXEとパッケージの版番号が一致しません。' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
$msix = Join-Path $output "ScriptSync_${version}_x64.msix"
if (Test-Path -LiteralPath $msix) { throw '既存のMSIXは上書きしません。' }
$certificate = Get-MsixSigningCertificate -Publisher 'CN=KotorinChunChun'
function Invoke-Checked([string]$Tool, [string[]]$Arguments) {
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "処理失敗: $Tool ($LASTEXITCODE)" }
}
try {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Directory |
        Where-Object { $_.Name -match '^\d+(\.\d+){3}$' } |
        Sort-Object { [version]$_.Name } -Descending |
        Where-Object { (Test-Path (Join-Path $_.FullName 'x64/makeappx.exe')) -and (Test-Path (Join-Path $_.FullName 'x64/signtool.exe')) } |
        Select-Object -First 1
    if (-not $sdk) { throw 'Windows SDKがありません。' }
    $layout = Join-Path $output ('layout-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path (Join-Path $layout 'Assets') -Force | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $layout 'scriptsync.exe')
    foreach ($asset in @('StoreLogo.png','Square44x44Logo.png','Square150x150Logo.png')) {
        Copy-Item -LiteralPath (Join-Path $source "src-tauri/icons/$asset") -Destination (Join-Path $layout "Assets/$asset")
    }
    $manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" xmlns:uap3="http://schemas.microsoft.com/appx/manifest/uap/windows10/3" IgnorableNamespaces="uap rescap uap3">
  <Identity Name="jp.kotorinchunchun.scriptsync" Publisher="CN=KotorinChunChun" Version="$version" ProcessorArchitecture="x64" />
  <Properties><DisplayName>読みあわせ (ScriptSync)</DisplayName><PublisherDisplayName>KotorinChunChun</PublisherDisplayName><Description>発話別録音と台本管理</Description><Logo>Assets\StoreLogo.png</Logo></Properties>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
  <Resources><Resource Language="ja-jp" /></Resources>
  <Applications><Application Id="ScriptSync" Executable="scriptsync.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="読みあわせ (ScriptSync)" Description="発話別録音と台本管理" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" /><Extensions><uap:Extension Category="windows.fileTypeAssociation" EntryPoint="Windows.FullTrustApplication"><uap3:FileTypeAssociation Name="scriptsync" Parameters="&quot;%1&quot;"><uap:SupportedFileTypes><uap:FileType>.scriptsync</uap:FileType></uap:SupportedFileTypes></uap3:FileTypeAssociation></uap:Extension></Extensions></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /><DeviceCapability Name="microphone" /></Capabilities>
</Package>
"@
    [IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest, [Text.UTF8Encoding]::new($false))
    Invoke-Checked (Join-Path $sdk.FullName 'x64/makeappx.exe') @('pack','/d',$layout,'/p',$msix)
    $signTool = Join-Path $sdk.FullName 'x64/signtool.exe'
    Invoke-Checked $signTool @('sign','/fd','SHA256','/s','My','/sha1',$certificate.Thumbprint,$msix)
    $verification = & $signTool verify /pa /v $msix 2>&1
    $verificationExit = $LASTEXITCODE
    $verification | Set-Content -LiteralPath (Join-Path $output 'signature-verification.txt') -Encoding utf8
    $verification | Write-Host
    if ($verificationExit -ne 0 -and ($verification -join "`n") -notmatch '0x800B0109|root\s+certificate which is not trusted|信頼されていないルート') { throw 'SignTool検証に失敗しました。' }
    $signature = Get-AuthenticodeSignature -LiteralPath $msix
    if (-not $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint -or $signature.Status -in @('HashMismatch','NotSigned')) { throw '署名検証に失敗しました。' }
    # 自己署名の信頼登録は変更せず、CMSの暗号署名を独立に検証する。
    Add-Type -AssemblyName System.Security.Cryptography.Pkcs
    $zip = [IO.Compression.ZipFile]::OpenRead($msix)
    try {
        $stream = $zip.GetEntry('AppxSignature.p7x').Open()
        $buffer = [IO.MemoryStream]::new()
        try { $stream.CopyTo($buffer); $signatureBytes = $buffer.ToArray() }
        finally { $stream.Dispose(); $buffer.Dispose() }
        if ([Text.Encoding]::ASCII.GetString($signatureBytes,0,4) -ne 'PKCX') { throw '署名ヘッダーが不正です。' }
        $cms = [Security.Cryptography.Pkcs.SignedCms]::new()
        $cms.Decode([byte[]]$signatureBytes[4..($signatureBytes.Length-1)])
        $cms.CheckSignature($true)
        if ($cms.SignerInfos.Count -ne 1 -or $cms.SignerInfos[0].Certificate.Thumbprint -ne $certificate.Thumbprint) { throw 'CMS署名者が一致しません。' }
    } finally { $zip.Dispose() }
    $cer = Join-Path $output 'ScriptSync.cer'
    [IO.File]::WriteAllBytes($cer, $certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
    $publicCertificate = [Security.Cryptography.X509Certificates.X509Certificate2]::new($cer)
    try { if ($publicCertificate.Thumbprint -ne $certificate.Thumbprint -or $publicCertificate.HasPrivateKey) { throw '公開証明書が不正です。' } }
    finally { $publicCertificate.Dispose() }
    Get-FileHash -LiteralPath $msix -Algorithm SHA256
} finally { $certificate.Dispose() }
