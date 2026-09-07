#Requires -Version 7.4
# 秘密鍵付きPFXを暗号化して移植用フォルダーへ出力する。repoやWeb領域へは出力しない。
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$OutputDirectory,
    [string]$CertificateThumbprint,
    [string]$Publisher
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) { throw 'MSIX署名の移植はWindows専用です。' }

$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$modulePath = Join-Path $PSScriptRoot 'MsixSigning.psm1'
$initializerPath = Join-Path $PSScriptRoot 'initialize-msix-signing.ps1'
$importerPath = Join-Path $PSScriptRoot 'import-msix-signing-bundle.ps1'
$exporterPath = Join-Path $PSScriptRoot 'export-msix-signing-bundle.ps1'
$documentPath = Join-Path $PSScriptRoot 'MSIX_SIGNING.md'

foreach ($requiredPath in @($modulePath, $initializerPath, $importerPath, $exporterPath, $documentPath)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "移植用ファイルが見つかりません: $requiredPath"
    }
}

Import-Module $modulePath -Force
$profile = Read-MsixSigningProfile
if ([string]::IsNullOrWhiteSpace($Publisher)) { $Publisher = $profile.publisher }
if ([string]::IsNullOrWhiteSpace($CertificateThumbprint)) { $CertificateThumbprint = $profile.thumbprint }

$certificate = Get-MsixSigningCertificate `
    -Publisher $Publisher `
    -CertificateThumbprint $CertificateThumbprint
try {
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Read-Host 'PFXと移植スクリプトを保存する安全なフォルダーの絶対パス'
    }
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        throw '出力先フォルダーが指定されていません。'
    }

    $outputRoot = [IO.Path]::GetFullPath($OutputDirectory)
    $projectRootWithSeparator = $projectRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if ($outputRoot.Equals($projectRoot, [StringComparison]::OrdinalIgnoreCase) -or
        $outputRoot.StartsWith($projectRootWithSeparator, [StringComparison]::OrdinalIgnoreCase)) {
        throw '秘密鍵を含むPFXをリポジトリ配下へ出力することは禁止されています。'
    }
    if (Test-Path -LiteralPath $outputRoot -PathType Leaf) {
        throw "出力先がファイルです: $outputRoot"
    }
    if ((Test-Path -LiteralPath $outputRoot -PathType Container) -and
        (Get-ChildItem -LiteralPath $outputRoot -Force | Select-Object -First 1)) {
        throw "出力先フォルダーは空である必要があります: $outputRoot"
    }

    $pfxFileName = 'KotorinChunChun-common-signing.pfx'
    $pfxPath = Join-Path $outputRoot $pfxFileName
    if (Test-Path -LiteralPath $pfxPath) {
        throw "既存のPFXを上書きしません: $pfxPath"
    }

    if (-not $PSCmdlet.ShouldProcess($outputRoot, 'MSIX署名移植用バンドルを作成')) {
        Write-Output "WhatIf: 移植用バンドルを作成します: $outputRoot"
        return
    }

    $pfxPassword = Read-Host 'PFX用の強いパスワード（移植先で再入力します）' -AsSecureString
    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    Export-PfxCertificate `
        -Cert "Cert:\CurrentUser\My\$($certificate.Thumbprint)" `
        -FilePath $pfxPath `
        -Password $pfxPassword `
        -NoClobber | Out-Null

    Copy-Item -LiteralPath $importerPath -Destination (Join-Path $outputRoot (Split-Path $importerPath -Leaf))
    Copy-Item -LiteralPath $modulePath -Destination (Join-Path $outputRoot (Split-Path $modulePath -Leaf))
    Copy-Item -LiteralPath $initializerPath -Destination (Join-Path $outputRoot (Split-Path $initializerPath -Leaf))
    Copy-Item -LiteralPath $exporterPath -Destination (Join-Path $outputRoot (Split-Path $exporterPath -Leaf))
    Copy-Item -LiteralPath $documentPath -Destination (Join-Path $outputRoot (Split-Path $documentPath -Leaf))

    $manifest = [ordered]@{
        schemaVersion = 1
        publisher = $certificate.Subject
        thumbprint = $certificate.Thumbprint
        pfxFileName = $pfxFileName
    }
    $manifestPath = Join-Path $outputRoot 'msix-signing-transfer.json'
    [IO.File]::WriteAllText(
        $manifestPath,
        (($manifest | ConvertTo-Json) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )

    $readmePath = Join-Path $outputRoot 'README.txt'
    $readme = @"
KotorinChunChun 共通MSIX署名移植バンドル

1. このフォルダーを、秘密鍵を扱ってよい移植先PCへ安全に移送してください。
2. 移植先PCで pwsh を起動し、次を実行してください。

   pwsh -NoProfile -ExecutionPolicy Bypass -File .\import-msix-signing-bundle.ps1

3. PFX作成時に設定したパスワードを対話入力してください。

4. 移植先PCで開発する各アプリのStore外MSIXは、Manifest Publisherを
   CN=KotorinChunChunに合わせ、アプリ側のリリース処理からPC共通設定を参照して署名してください。
   署名時はCurrentUser\Myの拇印をSignToolへ渡し、PFXを各アプリへコピーしないでください。

5. import-msix-signing-bundle.ps1は、秘密情報を含まない共通テンプレートを
   C:\develop\default\dev\templatesへ自動同期します。既存ファイルが異なる場合は上書きせず停止します。

AIへMSIX作成を依頼するときは、次の条件を伝えてください。

このPCでは、Store外MSIXの署名にKotorinChunChun共通署名証明書を使用してください。
PublisherはCN=KotorinChunChun、証明書はCurrentUser\Myの秘密鍵付き証明書を使い、
%LOCALAPPDATA%\KotorinChunChun\MSIX\signing.jsonの拇印を参照してください。
証明書の自動発行、Friendly Nameによる自動選択、個別PFXの使用やコピーは行わず、
アプリ既存の署名用ビルドスクリプトを使用してください。TrustedPeopleへの登録、インストール、
公開は別途明示した場合だけ行ってください。

PFXとパスワードは別経路で管理し、PFXをリポジトリ・Web公開領域・ログへ置かないでください。
この処理はTrustedPeople等の信頼ストアへ登録しません。ビルド用のCurrentUser/Myだけを設定します。
"@
    [IO.File]::WriteAllText($readmePath, $readme, [Text.UTF8Encoding]::new($false))

    Write-Output "MSIX署名移植バンドルを作成しました: $outputRoot"
    Write-Output "PFX: $pfxPath"
    Write-Output 'PFXのパスワードは、この出力やファイルへ記録しないでください。'
}
finally {
    if ($certificate) { $certificate.Dispose() }
}
