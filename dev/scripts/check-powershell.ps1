#Requires -Version 7.4
# 構文だけを解析する。対象スクリプトの処理・公開・録音は実行しない。
$ErrorActionPreference='Stop'
$files=@(Get-ChildItem -LiteralPath $PSScriptRoot -File | Where-Object Extension -in '.ps1','.psm1')
$failures=@()
foreach($file in $files){
    $tokens=$null;$issues=$null
    [void][Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$tokens,[ref]$issues)
    foreach($issue in $issues){$failures+="$($file.Name):$($issue.Extent.StartLineNumber): $($issue.Message)"}
}
if($failures.Count){$failures | Write-Output;exit 1}
Write-Output "成功: PowerShell構文 $($files.Count)ファイル（処理の実行なし）"
