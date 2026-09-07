# 既存証明書をPC共通設定に固定する。鍵の新規発行・PFX出力・信頼登録はしない。
#Requires -Version 7.4
[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$CertificateThumbprint,
    [Parameter(Mandatory)][string]$Publisher,
    [string]$ProfilePath
)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'MsixSigning.psm1') -Force
if (-not $ProfilePath) { $ProfilePath = Get-MsixSigningProfilePath }
$ProfilePath = [IO.Path]::GetFullPath($ProfilePath)
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$($CertificateThumbprint.ToUpperInvariant())" -ErrorAction Stop
try {
    Assert-MsixSigningCertificate -Certificate $certificate -Publisher $Publisher -Thumbprint $CertificateThumbprint.ToUpperInvariant()
    if (Test-Path -LiteralPath $ProfilePath) {
        $existing = Read-MsixSigningProfile -Path $ProfilePath
        if ($existing.thumbprint -cne $certificate.Thumbprint -or $existing.publisher -cne $Publisher) {
            throw '別の共通署名設定が既に存在します。上書きせず、全アプリの証明書移行を確認してください。'
        }
        Write-Output '既存の共通MSIX署名設定は同じ証明書です。変更しません。'
        return
    }
    $profile = [ordered]@{
        schemaVersion = 1; storeLocation = 'CurrentUser'; storeName = 'My'
        thumbprint = $certificate.Thumbprint; publisher = $certificate.Subject
    }
    if ($PSCmdlet.ShouldProcess($ProfilePath, '既存証明書の公開識別情報だけをPC共通設定へ保存')) {
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($ProfilePath)) | Out-Null
        # CreateNewにより、確認後に別設定が作成されても上書きしない。
        $stream = [IO.File]::Open($ProfilePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try {
            $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($profile | ConvertTo-Json) + "`n")
            $stream.Write($bytes, 0, $bytes.Length)
        } finally { $stream.Dispose() }
        Write-Output "共通MSIX署名設定を保存しました: $ProfilePath"
    }
} finally { $certificate.Dispose() }
