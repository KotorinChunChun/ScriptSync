param([string]$Exe='C:/develop/TextToSpeechCLI/TextToSpeechCLI.exe',[string]$Settings=$env:SCRIPTSYNC_TTS_SETTINGS)
$settingsArgs = @()
if ($Settings) { $settingsArgs = @('--settings', $Settings) }
$ErrorActionPreference='Stop'
$projectRoot=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$runRoot=Join-Path $projectRoot ('output/tests/corpus/'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $runRoot | Out-Null
$cases=@(
 @{id='bridge';text='橋を渡ってから、箸を置いてください。';kana='はしをわたってから、はしをおいてください。';corrected='橋を渡ってから、箸を置いてください。'},
 @{id='market';text='市場で買い物をした後、株式市場の動向を調べます。';kana='いちばでかいものをしたあと、かぶしきしじょうのどうこうをしらべます。';corrected='いちばで買い物をした後、株式市場の動向を調べます。'},
 @{id='popularity';text='人気の店ですが、閉店後は人気がありません。';kana='にんきのみせですが、へいてんごはひとけがありません。';corrected='人気の店ですが、閉店後はひとけがありません。'},
 @{id='deadline';text='締切は九月三十日なのに、資料が見つかりません。';kana='しめきりはくがつさんじゅうにちなのに、しりょうがみつかりません。';corrected='締切はくがつさんじゅうにちなのに、資料が見つかりません。'}
)
$results=@()
foreach($case in $cases){
 foreach($variant in @(@{name='A';reading=$case.text},@{name='B';reading=$case.kana},@{name='C';reading=$case.corrected})){
  $folder=Join-Path $runRoot ($case.id+'/'+$variant.name)
  New-Item -ItemType Directory -Path $folder | Out-Null
  $plan=@{format='ttscli.speech-plan';version=1;text=$case.text;voice=@{voice_name='tanaka-san_ja_JP';voice_version='2.0.1'};proposal=@{id='speech';reading=$variant.reading;model_expression='neutral';voice_expression='neutral';reason='固定コーパスの合成可否検証。聴感は未評価。';tone_parameters=@{alp=0;huskiness=0;intonation=1;pitch=0;speed=1;volume=0;style='';style_intensity=0}}}
  $plan | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $folder 'plan.json') -Encoding utf8
  & $Exe synthesize --plan (Join-Path $folder 'plan.json') @settingsArgs --output (Join-Path $folder 'audio.wav') --result-json (Join-Path $folder 'result.json') | Out-Null
  if($LASTEXITCODE -ne 0){throw "合成失敗: $($case.id)/$($variant.name)"}
  $result=Get-Content (Join-Path $folder 'result.json') -Raw | ConvertFrom-Json
  if($result.actual.duration -le 0){throw '音声長が不正です'}
  $results+=@{case=$case.id;variant=$variant.name;duration=$result.actual.duration;hash=(Get-FileHash (Join-Path $folder 'audio.wav')).Hash;quality='未採点'}
  Write-Output "合成成功: $($case.id)/$($variant.name)"
 }
}
@{results=$results;success_count=$results.Count;expected_count=12;human_quality_evaluated=$false;dictionary_state='外部CLI設定・未取得'} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $runRoot 'report.json') -Encoding utf8
Write-Output $runRoot
