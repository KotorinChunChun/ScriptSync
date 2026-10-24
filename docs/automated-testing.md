# 画面操作ツールを使わない自動検証

ブラウザー版は[Web版の検証手順](web-browser.md)、AI・音声合成の模擬接続は[サービス接続](service-connections.md)も参照してください。いずれもComputer Use不要です。

WindowsのPowerShell 7.4以上、Node.js 22.12以上、Rust/MSVC、WebView2を用意します。通常の原稿・録音・設定を試験データに使いません。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci --ignore-scripts
npm run verify
```

基本検証は、版・構成、秘密情報の現行/履歴、npm監査、UI、フロントbuild、Rust本体、録音crateの既定/MP3なし構成、第三者ライセンスをまとめて検査します。最初にRust依存の取得が必要なら、開発者ガイドに従ってCargoの準備を済ませてください。

npmとRust/OSVの依存監査にはインターネット接続が必要です。Rust照合は公開registryの名前・版だけを送信します。Tauri間接依存のUNIC 0.9.0に関する既知の保守終了5件は、公式分類がunmaintainedであることを確認して表示し、脆弱性・不健全性とは分けます。新しい指摘をまとめて無視する構成にはしません。

## 実WebView2まで自動で検証する

```powershell
npm run verify:native
```

画面内包Debug EXEをビルドし、Playwrightから試験プロセスのWebView2へCDP接続します。ブラウザーの追加ダウンロードやComputer Use・UI Automation・マウス座標操作は不要です。次を自動確認します。

- CSP下の画面起動・実IPC・版・管理ready、inline scriptの拒否。
- OSのファイル選択画面を使わないプロジェクト作成・台本/WAV取込、既存入口の上書き拒否。
- 合成テイク2つの結合、元テイクの保持、専用音声protocolのRange・デコード・再生時計。
- 編集直後のPageDownで別の台本を汚さないこと。
- 設定画面中の停止拒否、対象PIDの照合、試験プロセスの正常停止。

実際の既定出力デバイスが使えるPCでは、無音データでWASAPIとテイク編集画面の再生バー・時刻・一時停止・先頭停止も検証できます。

```powershell
npm run verify:native -- -AudioDevice
```

UIだけの再検証や、署名用ビルドのEXEを検証する場合は、ビルド済みの対象を明示できます。アプリ版とスクリプト側の版を合わせてください。

```powershell
npm run test:native -- --exe src-tauri/target/release/scriptsync.exe --with-audio-device
```

## 隔離と出力

既存ScriptSyncの管理パイプに応答があれば試験を中止し、既存プロセスを停止しません。試験自身が起動するプロセスだけへ、一時的なloopbackデバッグポートと `--settings-dir` を指定します。通常起動にデバッグ設定を残しません。合成無音WAV・2行の台本・プロジェクト・設定・スクリーンショット・JSON/HTML報告は、対象配布版の `dev/history/vX.Y.Z/logs/` に保存し、Gitと公開物から除外します。

成功時は管理契約から正常停止します。異常時は結果を保持し、試験所有の隔離プロセスだけを後片付けします。強制終了が必要だった試験を成功とは扱いません。スクリーンショットはCDPからアプリ内容だけを取得し、PC画面全体を収録しません。

## 自動化で証明しないもの

可聴品質、実マイクの音質・環境ノイズ、すべての録音/再生機器、非アクティブ時のOSグローバルキー、OSファイル選択画面、インストールと更新は別の受け入れ確認です。特に標準のネイティブ試験は実マイクを録音しません。`-AudioDevice` なしの成功を、WASAPIや可聴出力の成功に読み替えないでください。

ネイティブ試験の方式は[MicrosoftのWebView2/Playwright案内](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/playwright)に沿っています。試験のCDP接続を製品の外部管理APIとして使用しないでください。
