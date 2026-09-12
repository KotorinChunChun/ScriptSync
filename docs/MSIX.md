# MSIXのローカル作成

Windows SDKのMakeAppxとSignToolでx64版を作成する。アプリ版0.0.2はMSIX上で0.0.2.0になる。署名はPC共通の`%LOCALAPPDATA%\KotorinChunChun\MSIX\signing.json`が指す`CurrentUser\My`の既存証明書を使う。Publisherは`CN=KotorinChunChun`。秘密鍵の作成・輸出・信頼登録は行わない。

## 作成手順

対象版のソースで`npm run tauri build -- --no-bundle`を完了してから、対象ソースとEXEを明示する。スクリプトはEXE・package.json・tauri.conf.jsonの版を照合する。同じ版でもコードが異なる可能性があるため、ビルド元コミットを実装記録へ残す。

```powershell
cd C:\develop\ScriptSync
pwsh -File dev/scripts/package-ScriptSync-msix.ps1 `
  -SourceDirectory C:\develop\ScriptSync\release\msix-v0.0.2\source `
  -ExecutablePath C:\develop\ScriptSync\release\msix-v0.0.2\target\release\scriptsync.exe `
  -OutputDirectory C:\develop\ScriptSync\release\msix-v0.0.2
```

生成物は`ScriptSync_0.0.2.0_x64.msix`と秘密鍵を含まない`ScriptSync.cer`。既存MSIXを上書きせず、パッケージ検証・SHA-256署名・SignTool検証・CMS暗号署名・署名者とCERの一致確認を行う。インストールと外部公開はこのスクリプトには含まない。

自己署名証明書を信頼ストアへ登録していないPCでは、SignToolの信頼チェーン検証は未信頼ルートとして失敗する。この場合もCMS暗号署名を独立検証し、結果を`signature-verification.txt`へ保存する。インストール先では、この公開証明書の発行者と拇印を確認して信頼する必要がある。信頼登録は自動実行しない。

## 実行環境と検証範囲

Windows 10 2004以降のx64とWebView2 Runtimeを前提とする。MSIXの生成・署名検証と、インストール後の動作確認は別に扱う。

v0.0.2のASRはコンパイル時のソース配置にある`tools/asr`、合成比較は既存の外部TextToSpeechCLI環境に依存する。今回の専用ソース配置ではvenvとmodelsを元プロジェクトの環境へ接続する。ASRを使う間は`release/msix-v0.0.2/source`と接続元環境を保持する。このMSIXはPython・認識モデル・外部CLIを同梱した他PC向けの完結パッケージではない。

仕様根拠：[MicrosoftのMSIX手動作成手順](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-manual-conversion)。
