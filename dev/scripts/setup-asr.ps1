param([string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$asrRoot = Join-Path $projectRoot 'tools/asr'
& $Python -m venv (Join-Path $asrRoot 'venv')
if ($LASTEXITCODE -ne 0) { throw 'Python 3.12以降のvenv作成に失敗しました' }
$runtime = Join-Path $asrRoot 'venv/Scripts/python.exe'
& $runtime -m pip install 'faster-whisper==1.2.1' 'pykakasi==2.3.0'
if ($LASTEXITCODE -ne 0) { throw 'ASR依存のインストールに失敗しました' }
& $runtime (Join-Path $asrRoot 'transcribe.py') --prepare
if ($LASTEXITCODE -ne 0) { throw 'ASRモデルの準備に失敗しました' }
