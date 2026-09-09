param([string]$Exe = 'C:/develop/TextToSpeechCLI/TextToSpeechCLI.exe', [string]$Settings = $env:SCRIPTSYNC_TTS_SETTINGS)
$settingsArgs = @()
if ($Settings) { $settingsArgs = @('--settings', $Settings) }
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$fixtureRoot = Join-Path $projectRoot ('output/tests/synthetic/' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
$cases = @(
    @{ id='market-reference'; text='市場へ行きます。'; reading='いちばへいきます。'; role='synthetic_reference' },
    @{ id='market-wrong'; text='市場へ行きます。'; reading='しじょうへいきます。'; role='intentional_misreading' },
    @{ id='repeat-reference'; text='人気のない道で人気の商品を見ました。'; reading='ひとけのないみちでにんきのしょうひんをみました。'; role='synthetic_reference' },
    @{ id='repeat-wrong'; text='人気のない道で人気の商品を見ました。'; reading='にんきのないみちでにんきのしょうひんをみました。'; role='intentional_misreading' }
)
foreach ($case in $cases) {
    $caseDir = Join-Path $fixtureRoot $case.id
    New-Item -ItemType Directory -Path $caseDir | Out-Null
    $plan = @{ format='ttscli.speech-plan'; version=1; text=$case.text; voice=@{voice_name='tanaka-san_ja_JP'; voice_version='2.0.1'}; proposal=@{
        id='speech'; reading=$case.reading; model_expression='neutral'; voice_expression='neutral'; reason='ScriptSyncの合成検証音声。人間の正解音声ではありません。'; tone_parameters=@{alp=0;huskiness=0;intonation=1;pitch=0;speed=1;volume=0;style='';style_intensity=0}
    }}
    $planPath = Join-Path $caseDir 'plan.json'
    $plan | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $planPath -Encoding utf8
    $env:TTSCLI_LOG_DIR = Join-Path $caseDir 'ai_logs'
    & $Exe synthesize --plan $planPath @settingsArgs --output (Join-Path $caseDir 'audio.wav') --result-json (Join-Path $caseDir 'result.json') | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "合成失敗: $($case.id)" }
    $case.hash = (Get-FileHash -LiteralPath (Join-Path $caseDir 'audio.wav') -Algorithm SHA256).Hash
    Write-Output "生成完了: $($case.id)"
}
@{source='TextToSpeech CLI synthetic fixture';human_recording=$false;cases=$cases} | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $fixtureRoot 'manifest.json') -Encoding utf8
Write-Output $fixtureRoot
