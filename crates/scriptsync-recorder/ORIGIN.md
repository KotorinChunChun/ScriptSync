# 録音基盤の由来

`dev/sample/microphone-recorder-source-v0.4.6/src/src-tauri/` から2026-09-08にコピー。
元パッケージのSHA256SUMS.txtとTHIRD-PARTY-NOTICES.txtを参照する。
公開ライセンスの新規付与は行わない。

ScriptSyncでの変更：WAV新規作成を排他的にし、writer失敗時の音声削除を取り除いた。
停止後のcapture欠落数と残りデータ転送時の欠落数を合算する。
台帳と製品UIはScriptSync側で管理し、見本CLIは診断用として保持する。
