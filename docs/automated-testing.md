# 画面操作ツールを使わない自動検証

## 本体機能をCLIで実行・検証する

`scriptsync-cli.exe` はウィンドウ・WebView・管理パイプを作らず、GUI版が使用する通常のアプリ処理を呼びます。MSIXにも同梱しています。ソースからの検証は次を使用します。

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --bin scriptsync-cli --locked
npm run test:cli
```

試験は配布版に対応するlogs配下へ専用設定、台本、合成WAVを作り、実行ファイルを起動します。台本取込・確定、テイク結合と元音声保持、トリムプレビュー、取出しの上書き拒否、WAV・FLAC出力、模擬サーバーとの認証・文字起こし、未承認原稿の保持、別プロセスでの設定・プロジェクト再読込、正常終了を確認します。マイク、既定設定、利用者の原稿・録音、ブラウザー、画面操作ツールは使用しません。

実接続を確認する場合は、受付中のサーバーと合成音声を明示します。指定の音声だけを指定サーバーへ送信します。

```powershell
npm run test:cli -- --asr-url http://127.0.0.1:8769 --asr-model whisper/base --asr-wav C:/work/synthetic-ja.wav
```

MSIX導入後は、Windowsが登録する `scriptsync-cli.exe` の実行エイリアスを使います。`scriptsync-cli --help` で確認できます。WindowsApps配下の実体を直接起動する方式にはしません。配布CLI自体の検証は `npm run test:cli -- --installed`。版・実体のハッシュを確認して正規のエイリアスから実行し、実接続の指定も併用できます。ソースからビルドしたEXEを選ぶ場合は `--exe <絶対パス>` です。

コマンド名をPATHから見つけられない環境では、登録先を直接指定します。利用者のPATHを変更する必要はありません。

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\scriptsync-cli.exe" --help
```

CLIの入力と出力は1行1JSONのJSON Linesです。起動には `--settings-dir <専用設定の絶対パス>` が必須で、既定の利用者設定へ自動で接続しません。出力の `ok` がfalseならそのコマンドは失敗です。通信が正常に完了した場合、個々の操作エラーはJSONで返し、その後のコマンドを受け付けます。

```json
{"id":1,"action":"markdown_preview","payload":{"text":"合成の台本です。"}}
{"id":2,"action":"state","payload":{}}
{"id":3,"action":"shutdown","payload":{}}
```

| 操作 | actionと主なpayload |
|---|---|
| 設定の取得・保存 | machine_settings、settings_save（取得したsettingsを編集して渡す） |
| 接続確認 | asr_server_check（url） |
| 新規・読込 | new / open（pathの絶対パス） |
| Markdown読込 | markdown_import / markdown_drop（pathsの絶対パス一覧） |
| 台本確定 | script_commit（project_id、base_revision、base_generation、sources） |
| 音声取込・採用 | import_audio（segment、path）、select（segment、take） |
| 結合・加工 | join_takes、preview_take_edit、trim_take（画面と同じID・hash・範囲） |
| 文字起こし | recording_asr（segment、take）、結果確認後にrecording_draft（segment、text） |
| 出力 | export_blocks（unit、naming、format、global_gaps等の通常設定）、download_take（segment、take、path） |
| 保存・完了待ち・終了 | save_project、wait_idle、close_project、shutdown |

非同期処理は `wait_idle` 後に `state` で確認します。待機上限は既定640秒、payloadのtimeout_msで短縮できます。shutdownと標準入力の終了時には通常のstopを通し、処理完了を待って再生デバイスを閉じます。CLIのファイル操作は明示パスが必須で、出力には新しいファイル名を指定します。ブラウザーやExplorerを開く操作、UIセッション操作は拒否します。録音や再生など実機を使う操作は、明示的に要求した場合だけ通常の製品処理へ渡します。

CLI自身は要求や結果を診断ログへ保存しません。標準出力には操作結果が含まれるため、実データを扱う際のリダイレクト先は利用者側で管理してください。自動試験の報告は判定・件数・ハッシュを中心にし、音声やサーバートークンを含めません。

ブラウザー版は[Web版の検証手順](web-browser.md)、AI・音声合成の模擬接続は[サービス接続](service-connections.md)も参照してください。いずれもComputer Use不要です。

WindowsのPowerShell 7.4以上、Node.js 22.12以上、Rust/MSVC、WebView2を用意します。通常の原稿・録音・設定を試験データに使いません。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci --ignore-scripts
npm run verify
```

基本検証では版と構成、現行ファイルと履歴の秘密情報、npm監査、UI、フロントエンドのビルド、Rust本体、録音crateの既定構成とMP3なし構成、第三者ライセンスをまとめて確認します。Rustの依存関係を初めて取得する場合は、開発者ガイドに従ってCargoを準備してください。

npmとRust/OSVの依存監査にはインターネット接続が必要です。Rust照合は公開registryの名前・版だけを送信します。UNIC 0.9.0の5件とchrono 0.4.45の既知の保守終了通知は、ID・名前・版を限定し、公式分類がunmaintainedであることを確認して表示します。脆弱性・不健全性とは分け、新しい指摘をまとめて無視する構成にはしません。

## 台本取込・確定とテイク登録をCLIで検証する

Rust依存の準備後に、PowerShell 7から次を実行します。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib workflow_tests::markdown_project_and_imported_take_round_trip -- --exact
```

試験は一時フォルダーに2行のMarkdown、無音WAV、設定、プロジェクトを用意します。アプリが使う処理を直接呼び、台本取込と承認済み原稿の確定、保存、再読込を検証します。再読込後に1行目へWAVをテイクとして取り込んで採用し、もう一度保存・再読込して、発話との紐づけと音声のバイト列・SHA-256が一致することを検証します。正常終了時に一時フォルダーを削除します。

アプリのウィンドウ、既存設定・履歴、実マイク、外部APIを使いません。録音画面の操作や実際の収録・試聴は別途確認します。

## テイクのTranscriptionServer連携を検証する

PowerShell 7で作業用チェックアウトのルートを開き、模擬サーバーを使った契約試験を実行します。通常のアプリ処理で、設定の保存、明示操作までの送信抑止、音声の送信、修正案の保存と再読込を調べます。

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib transcription_server::tests
```

実機試験は、受付中のTranscriptionServerと日本語の合成WAVを用意して実行します。この試験では指定のサーバーへ合成WAVを送信します。初回取得を避ける場合は、サーバーで準備済みの日本語モデルIDを指定してください。URLの既定は `http://127.0.0.1:8769`、モデルの既定は `whisper/tiny` です。

```powershell
$env:SCRIPTSYNC_TEST_ASR_WAV = '合成WAVの絶対パス'
$env:SCRIPTSYNC_TEST_ASR_MODEL = 'whisper/base'
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib transcription_server::tests::real_server_transcription_reaches_only_the_recording_draft -- --ignored --exact --nocapture
```

どちらもScriptSyncの設定とプロジェクトを一時フォルダーに分け、利用者の設定・原稿・録音・辞書を使用しません。録音画面の操作や実マイクでの録音は別途確認します。

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
