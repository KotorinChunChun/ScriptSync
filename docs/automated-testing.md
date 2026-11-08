# 画面操作ツールを使わない自動検証

ブラウザー版は[Web版の検証手順](web-browser.md)、AI・音声合成の模擬接続は[サービス接続](service-connections.md)も参照してください。いずれもComputer Use不要です。

WindowsのPowerShell 7.4以上、Node.js 22.12以上、Rust/MSVC、WebView2を用意します。通常の原稿・録音・設定を試験データに使いません。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci --ignore-scripts
npm run verify
```

基本検証では版と構成、現行ファイルと履歴の秘密情報、npm監査、UI、フロントエンドのビルド、Rust本体、録音crateの既定構成とMP3なし構成、第三者ライセンスをまとめて確認します。Rustの依存関係を初めて取得する場合は、開発者ガイドに従ってCargoを準備してください。

npmとRust/OSVの依存監査にはインターネット接続が必要です。Rust照合は公開registryの名前・版だけを送信します。Tauri間接依存のUNIC 0.9.0に関する既知の保守終了5件は、公式分類がunmaintainedであることを確認して表示し、脆弱性・不健全性とは分けます。新しい指摘をまとめて無視する構成にはしません。

## 台本取込・確定とテイク登録をCLIで検証する

Rust依存の準備後に、PowerShell 7から次を実行します。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib workflow_tests::markdown_project_and_imported_take_round_trip -- --exact
```

試験は一時フォルダーに2行のMarkdown、無音WAV、設定、プロジェクトを用意します。アプリが使う処理を直接呼び、台本取込と承認済み原稿の確定、保存、再読込を検証します。再読込後に1行目へWAVをテイクとして取り込んで採用し、もう一度保存・再読込して、発話との紐づけと音声のバイト列・SHA-256が一致することを検証します。正常終了時に一時フォルダーを削除します。

アプリのウィンドウ、既存設定・履歴、実マイク、外部APIを使いません。録音画面の操作や実際の収録・試聴は別途確認します。

## 実WebView2まで自動で検証する

```powershell
npm run verify:native
```

画面を内包したDebug EXEをビルドし、Playwrightから試験プロセスのWebView2にCDPで接続します。ブラウザーの追加ダウンロード、Computer Use、UI Automation、マウス座標による操作は不要です。

- CSPを適用した状態での画面起動、実際のIPC、版、管理ready、inline scriptの拒否。
- OSのファイル選択画面を使わないプロジェクトの作成と台本・WAVの取り込み、既存のプロジェクト入口ファイルの上書き拒否。
- 合成テイク2つの結合、元テイクの保持、専用音声protocolのRange、デコード、再生時刻。
- 台本の編集直後にPageDownを押しても、移動先の台本が書き換わらないこと。
- 設定画面を開いている間の停止拒否、対象PIDの照合、試験プロセスの正常停止。

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

試験に成功した場合は、管理契約に従って試験プロセスを正常停止します。異常時は結果を残し、試験が起動した隔離プロセスだけを終了します。強制終了が必要だった試験は成功に数えません。スクリーンショットはCDPでアプリの表示だけを取得します。

## 自動化で証明しないもの

可聴品質、実マイクの音質・環境ノイズ、すべての録音/再生機器、非アクティブ時のOSグローバルキー、OSファイル選択画面、インストールと更新は別の受け入れ確認です。特に標準のネイティブ試験は実マイクを録音しません。`-AudioDevice` なしの成功を、WASAPIや可聴出力の成功に読み替えないでください。

ネイティブ試験の方式は[MicrosoftのWebView2/Playwright案内](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/playwright)に沿っています。試験のCDP接続を製品の外部管理APIとして使用しないでください。
