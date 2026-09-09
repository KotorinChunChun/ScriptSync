# 録音基盤の由来

`dev/sample/microphone-recorder-source-v0.4.6/src/src-tauri/` から2026-09-08にコピー。
元パッケージのSHA256SUMS.txtとTHIRD-PARTY-NOTICES.txtを参照する。
公開ライセンスの新規付与は行わない。

ScriptSyncでの変更：WAV新規作成を排他的にし、writer失敗時の音声削除を取り除いた。
停止後のcapture欠落数と残りデータ転送時の欠落数を合算する。
台帳と製品UIはScriptSync側で管理し、見本CLIは診断用として保持する。

## 2026-09-09時点の追加変更

アプリ統合後、連続リサンプリング、入力不連続の扱い、端点フェードと品質情報・書き込み失敗の伝達を改修しています。元の受領版そのものとは区別してください。変更経緯・検証範囲は[確定版実装記録](../../dev/history/v0.0.1/v0.0.1-imp.md)、再確認は[ノイズ手順](../../dev/audio-recording-noise-fix-procedure.md)を参照します。
