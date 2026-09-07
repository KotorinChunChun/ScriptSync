#Requires -Version 7.4
# 秘密鍵付きPFXをCurrentUser/Myへ登録し、PC共通MSIX設定を初期化する。
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$PfxPath,
    [string]$TemplateDirectory = 'C:\develop\default\dev\templates'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) { throw 'MSIX署名の移植はWindows専用です。' }

$bundleRoot = [IO.Path]::GetFullPath($PSScriptRoot)
$manifestPath = Join-Path $bundleRoot 'msix-signing-transfer.json'
$modulePath = Join-Path $bundleRoot 'MsixSigning.psm1'
$initializerPath = Join-Path $bundleRoot 'initialize-msix-signing.ps1'
$importerPath = Join-Path $bundleRoot 'import-msix-signing-bundle.ps1'
$exporterPath = Join-Path $bundleRoot 'export-msix-signing-bundle.ps1'
$documentPath = Join-Path $bundleRoot 'MSIX_SIGNING.md'
$requiredTemplateFiles = @(
    $modulePath,
    $initializerPath,
    $importerPath
)
$optionalTemplateFiles = @($exporterPath, $documentPath) |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }
$templateFiles = @($requiredTemplateFiles + $optionalTemplateFiles)

foreach ($requiredPath in @($manifestPath) + $requiredTemplateFiles) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "移植バンドルのファイルが不足しています: $requiredPath"
    }
}

try {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json -ErrorAction Stop
}
catch {
    throw '移植バンドルの公開メタデータをJSONとして読み込めません。'
}

$manifestFields = @($manifest.PSObject.Properties.Name)
$requiredFields = @('schemaVersion', 'publisher', 'thumbprint', 'pfxFileName')
if ($manifestFields.Count -ne $requiredFields.Count -or
    @($requiredFields | Where-Object { $_ -notin $manifestFields }).Count -gt 0) {
    throw '移植バンドルの公開メタデータ項目が不正です。'
}
if ($manifest.schemaVersion -ne 1 -or
    $manifest.publisher -isnot [string] -or $manifest.publisher -notmatch '^CN=.+') {
    throw '移植バンドルのPublisherまたは形式が不正です。'
}
$expectedThumbprint = ([string]$manifest.thumbprint).Replace(' ', '').ToUpperInvariant()
if ($expectedThumbprint -notmatch '^[A-F0-9]{40}$') {
    throw '移植バンドルの拇印が不正です。'
}
$expectedPfxName = [IO.Path]::GetFileName([string]$manifest.pfxFileName)
if ($expectedPfxName -cne [string]$manifest.pfxFileName -or $expectedPfxName -notmatch '\.pfx$') {
    throw '移植バンドルのPFXファイル名が不正です。'
}

if ([string]::IsNullOrWhiteSpace($PfxPath)) {
    $pfxCandidates = @(Get-ChildItem -LiteralPath $bundleRoot -Filter '*.pfx' -File)
    if ($pfxCandidates.Count -ne 1) {
        throw '移植バンドル直下にPFXを1つだけ置くか、-PfxPathでPFXを指定してください。'
    }
    $PfxPath = $pfxCandidates[0].FullName
}
else {
    $PfxPath = [IO.Path]::GetFullPath($PfxPath)
}
if (-not (Test-Path -LiteralPath $PfxPath -PathType Leaf)) {
    throw "PFXが見つかりません: $PfxPath"
}
if ((Split-Path -Leaf $PfxPath) -cne $expectedPfxName) {
    throw '指定されたPFXのファイル名が移植メタデータと一致しません。'
}

$templateDirectory = [IO.Path]::GetFullPath($TemplateDirectory)
if (Test-Path -LiteralPath $templateDirectory -PathType Leaf) {
    throw "共通テンプレートの同期先がファイルです: $templateDirectory"
}
$templateConflicts = @()
foreach ($templateSource in $templateFiles) {
    $templateDestination = Join-Path $templateDirectory (Split-Path -Leaf $templateSource)
    if (Test-Path -LiteralPath $templateDestination -PathType Container) {
        $templateConflicts += $templateDestination
        continue
    }
    if (Test-Path -LiteralPath $templateDestination -PathType Leaf) {
        $sourceHash = (Get-FileHash -LiteralPath $templateSource -Algorithm SHA256).Hash
        $destinationHash = (Get-FileHash -LiteralPath $templateDestination -Algorithm SHA256).Hash
        if ($sourceHash -cne $destinationHash) {
            $templateConflicts += $templateDestination
        }
    }
}
if ($templateConflicts.Count -gt 0) {
    throw "共通テンプレートの同期先に異なる既存ファイルがあります。上書きしません: $($templateConflicts -join ', ')"
}

Import-Module $modulePath -Force
$profilePath = Get-MsixSigningProfilePath
if (Test-Path -LiteralPath $profilePath -PathType Leaf) {
    $existingProfile = Read-MsixSigningProfile -Path $profilePath
    if ($existingProfile.publisher -cne [string]$manifest.publisher -or
        $existingProfile.thumbprint -cne $expectedThumbprint) {
        throw '移植先に別の共通署名設定が存在します。既存設定を上書きしません。'
    }
}

if (-not $PSCmdlet.ShouldProcess('Cert:\CurrentUser\My', 'MSIX署名用PFXをインポートし共通設定を登録')) {
    Write-Output "WhatIf: PFXをCurrentUser/Myへインポートし、共通設定を登録します: $PfxPath"
    return
}

$pfxPassword = Read-Host 'PFX用パスワード' -AsSecureString
try {
    $pfxData = Get-PfxData -FilePath $PfxPath -Password $pfxPassword -ErrorAction Stop
}
catch {
    throw 'PFXを読み込めません。パスワード、ファイル、または秘密鍵を確認してください。'
}
$leafCertificates = @($pfxData.EndEntityCertificates)
if ($leafCertificates.Count -ne 1) { throw 'PFX内のエンドエンティティ証明書を一意に特定できません。' }
$leaf = $leafCertificates[0]
if ($leaf.Subject -cne [string]$manifest.publisher -or $leaf.Thumbprint -cne $expectedThumbprint) {
    throw 'PFXのPublisherまたは拇印が移植メタデータと一致しません。インポートを中止しました。'
}
$now = Get-Date
if ($leaf.NotBefore -gt $now -or $leaf.NotAfter -le $now.AddDays(7)) {
    throw 'PFX内の証明書が有効期間外、または期限まで7日以下です。インポートを中止しました。'
}
$usages = @($leaf.Extensions |
    Where-Object { $_ -is [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension] } |
    ForEach-Object { $_.EnhancedKeyUsages } |
    ForEach-Object { $_.Value })
if ('1.3.6.1.5.5.7.3.3' -notin $usages) {
    throw 'PFX内の証明書にCode Signing用途がありません。インポートを中止しました。'
}
foreach ($extension in $leaf.Extensions) {
    if ($extension -is [System.Security.Cryptography.X509Certificates.X509KeyUsageExtension] -and
        -not ($extension.KeyUsages -band [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)) {
        throw 'PFX内の証明書がデジタル署名を許可していません。インポートを中止しました。'
    }
}

Write-Host "Publisher: $($leaf.Subject)"
Write-Host "Thumbprint: $($leaf.Thumbprint)"
Write-Host "有効期限: $($leaf.NotAfter)"
$confirmation = Read-Host 'この証明書をCurrentUser/Myへ登録する場合は YES と入力してください'
if ($confirmation -cne 'YES') { throw 'ユーザー確認がないため、インポートを中止しました。' }

$certificate = $null
$importedCertificates = @()
try {
    $existingCertificates = @(Get-ChildItem -LiteralPath 'Cert:\CurrentUser\My' |
        Where-Object { $_.Thumbprint -ceq $expectedThumbprint })
    $existing = $existingCertificates | Where-Object HasPrivateKey | Select-Object -First 1
    if ($existing) {
        Assert-MsixSigningCertificate `
            -Certificate $existing `
            -Publisher ([string]$manifest.publisher) `
            -Thumbprint $expectedThumbprint
        $certificate = $existing
        Write-Output '同じ秘密鍵付き証明書が既に登録されています。再インポートしません。'
    }
    else {
        $importedCertificates = @(Import-PfxCertificate `
            -FilePath $PfxPath `
            -CertStoreLocation 'Cert:\CurrentUser\My' `
            -Password $pfxPassword `
            -ErrorAction Stop)
        $certificate = @(Get-ChildItem -LiteralPath 'Cert:\CurrentUser\My' |
            Where-Object { $_.Thumbprint -ceq $expectedThumbprint -and $_.HasPrivateKey } |
            Select-Object -First 1)
        if (-not $certificate) {
            throw 'PFXを登録しましたが、CurrentUser/Myから秘密鍵付き証明書を取得できません。'
        }
        $certificate = $certificate[0]
        Assert-MsixSigningCertificate `
            -Certificate $certificate `
            -Publisher ([string]$manifest.publisher) `
            -Thumbprint $expectedThumbprint
        Write-Output '秘密鍵付き証明書をCurrentUser/Myへ登録しました。'
    }

    & $initializerPath `
        -CertificateThumbprint $expectedThumbprint `
        -Publisher ([string]$manifest.publisher)

    Write-Output "共通MSIX署名設定を登録しました: $(Get-MsixSigningProfilePath)"
    [IO.Directory]::CreateDirectory($templateDirectory) | Out-Null
    foreach ($templateSource in $templateFiles) {
        $templateDestination = Join-Path $templateDirectory (Split-Path -Leaf $templateSource)
        if (-not (Test-Path -LiteralPath $templateDestination -PathType Leaf)) {
            [IO.File]::Copy($templateSource, $templateDestination, $false)
        }
    }
    Write-Output "共通テンプレートを同期しました: $templateDirectory"
    Write-Output 'TrustedPeople等の信頼ストアは変更していません。'
}
finally {
    if ($certificate) { $certificate.Dispose() }
    foreach ($imported in $importedCertificates) {
        if ($imported) { $imported.Dispose() }
    }
}
