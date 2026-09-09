param(
    [string]$SampleDirectory = (Join-Path $PSScriptRoot '../sample/登壇原稿_40分版')
)
$ErrorActionPreference = 'Stop'
$samplePath = (Resolve-Path -LiteralPath $SampleDirectory).Path
$previousSample = $env:SCRIPT_SYNC_MERGE_SAMPLE
Push-Location (Join-Path $PSScriptRoot '../..')
try {
    $env:SCRIPT_SYNC_MERGE_SAMPLE = $samplePath
    & cargo test --manifest-path src-tauri/Cargo.toml presentation_corpus_experiment -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw '原稿マージ試験が失敗しました。テスト出力を確認してください。' }
} finally {
    $env:SCRIPT_SYNC_MERGE_SAMPLE = $previousSample
    Pop-Location
}
