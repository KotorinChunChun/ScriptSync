# 開発者ガイド

開発正本は `C:\develop\KotorinToys\tools\ScriptSync`。親は採用コミットを記録し、子のGit履歴・配布形式を維持します。[KotorinToys収録](kotorintoys-integration.md)を参照してください。

現行版はv0.0.6です。[実装記録](../dev/history/v0.0.6/v0.0.6-imp.md)で自動試験・ネイティブ操作確認・未確認範囲を確認してください。公開版v0.0.5以前の記録はdev直下またはhistoryに保持しています。

## 開発起動とビルド

署名付きMSIXのローカル作成は[MSIX作成手順](MSIX.md)を参照してください。

Windows、PowerShell 7、Node.js 22.12以上/npm（検証は22系）、Rust、TauriのWindowsビルド環境（MSVC・WebView2）が必要です。録音・プロジェクト保存の確認にはネイティブアプリを使用します。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci
npm run tauri dev
```

UIはViteのホットリロードで反映されます。録音は停止・保存してから更新してください。Rust変更時は再ビルドされます。`npm run dev`だけではブラウザー表示用で、ネイティブ録音やファイル操作の動作確認にはなりません。

KotorinToysから操作する開発版は `pwsh -File dev/scripts/build-debug-ScriptSync-kotorintoys.ps1` で作成します。画面を内包した `target/kotorintoys/scriptsync.exe` を本体が管理します。非表示起動と正常停止、専用設定の試験方法は [管理連携ガイド](kotorintoys-integration.md) を参照してください。

```powershell
npm run tauri build -- --no-bundle
```

実行ファイルは `src-tauri/target/release/scriptsync.exe` です。既存アプリを閉じてから最新ビルドを起動します。配布用インストーラーの作成・別PCでの動作保証とは別です。

## 通常の検証

一括検証は `npm run verify`、実WebView2・IPC・CSPまでの自動検証は `npm run verify:native`。実出力デバイスを使う無音の検証は `npm run verify:native -- -AudioDevice`。詳細は[自動検証ガイド](automated-testing.md)を参照してください。OS画面操作やComputer Useは必要ありません。

変更前に `git status --short` を確認し、既存変更を保持します。関係する試験から実施し、結果と未確認事項を実装計画へ記録します。

```powershell
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path crates/scriptsync-recorder/Cargo.toml --locked --target-dir src-tauri/target
```

UI試験はDOMとRPCモック、Rust試験は保存・録音処理・編集・出力などを検証します。マイク音質、OSのダイアログ、非アクティブ時のキー入力、実際の再生音の検証は別途必要です。[検証記録](validation.md)と[読み上げテスト](../samples/reading-test/README.md)を参照してください。

文書だけの変更では参照先、現在のコード・画面名との一致、`git diff --check`を確認します。過去の試験件数を今回の実行結果として記載しません。

## 外部音声・認識環境

録音と採用音声の結合にはTextToSpeech CLI・ASRは不要です。比較合成はTextToSpeech CLIを呼び出します。配置先は `C:/develop/TextToSpeechCLI`、実行ファイルは `C:/develop/TextToSpeechCLI/TextToSpeechCLI.exe` です。音声合成エンジン・話者・認証・接続設定はCLI側で管理します。Rustの起動先と検証スクリプトもこのパスへ更新しています。設定ファイルはCLI側の既定を利用し、明示指定が必要なら環境変数 `SCRIPTSYNC_TTS_SETTINGS`（スクリプトでは `-Settings` も可）で渡します。新EXEの動作確認は未実施です。呼出引数・入出力形式はCLIの契約に合わせます。認証情報は同梱しません。

ローカルASRは独立Pythonワーカーです。Python 3.12を推奨します。

```powershell
pwsh -File dev/scripts/setup-asr.ps1 -Python 'Python実行ファイルの絶対パス'
```

初回準備は依存とモデルを取得します。通常の認識はキャッシュを使用し、原稿・期待読みを認識のヒントに渡しません。候補は人間が確認し、正解読みへ自動確定しません。配布先ASRは設定画面からアプリ専用環境へ導入します。上のsetup-asr.ps1は開発試験用で、配布先には要求しません。Codex CLI方式も同じローカル認識器を専用MCP経由で実行します。

```powershell
pwsh -File dev/scripts/create-synthetic-fixtures.ps1
$env:SCRIPT_SYNC_FIXTURES = '生成されたフォルダーの絶対パス'
cargo test --manifest-path src-tauri/Cargo.toml --lib synthetic_end_to_end -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib synthetic_asr_measurement -- --ignored --nocapture
```

これら2件は通常ignoredの外部環境試験です。合成fixtureの成功は人間の音声品質の保証にはなりません。CLI側の音声合成エンジンファイルは[実機検証形式](talk-project-format.md)を参照します。

## 構造と保存契約

[ファイル構成目録](REPOSITORY_MAP.md)から担当モジュールを探し、[現行アーキテクチャ](architecture.md)で保存と処理の境界を確認してください。

- `.scriptsync`と対応する`.scriptsync.data/`がプロジェクト一式です。
- `revisions/`の保存世代が正本で、`project.json`は閲覧用索引です。破損時に黙って過去へ戻さず、既存世代・音声を保持します。
- テイクのトリム保存は音声実体・ハッシュ・長さ・波形を更新します。画面だけの開始終了変更で済ませません。
- 原文、読み上げ編集、録音時原稿を区別します。ファイルIDと発話IDを保持し、変更行の読み・テイク維持は同じファイル内の明示対応で確認します。
- 録音crateの由来・変更点は[ORIGIN](../crates/scriptsync-recorder/ORIGIN.md)を更新します。

## バージョンと文書更新

要求は `dev/history/vX.Y.Z/vX.Y.Z-req.md`、設計・実装結果は同フォルダーの `vX.Y.Z-imp.md`、進捗は `vX.Y.Z-task.md` に日本語で記録します。アプリ版は0.0.6、保存形式は5です。

キー変更時は[キー定義](../dev/SHORTCUT_KEYS.md)、効果音変更時は[効果音定義](../dev/SOUND_EFFECTS.md)、画面操作変更時は利用者ガイドとサンプル手順も更新します。生成物、個人原稿・音声、認証情報、ユーザー提供の `dev/sample/` を文書整理のために追加・削除しないでください。

## CLI移行時の互換性確認

実行パスの変更と通信契約の変更は別です。ユーザー指定によりplan形式識別子は `ttscli.speech-plan`、ログ出力先環境変数は `TTSCLI_LOG_DIR` に統一しています。コードと検証スクリプトへ反映済みです。新CLIとの実接続および設定省略時の動作は別途確認が必要です。Cargoのauthorsは作者の帰属情報でありCLIの名称ではありません。

v0.0.3：[要求](../dev/history/v0.0.3/v0.0.3-req.md)／[実装計画](../dev/history/v0.0.3/v0.0.3-imp.md)。


### Windows配布は署名済みMSIX

標準コマンドは `npm run release:msix`。リリースEXEをビルドし、既存共通証明書でMSIXを署名・検証する。TauriのNSIS/MSI自動生成は無効化した。`--no-bundle`によるEXEは中間成果物で、配布完了の代わりにしない。

詳細は[MSIX作成手順](MSIX.md)。インストール・証明書の信頼登録・公開は作成と区別する。

## 高速な静的チェック

```powershell
npm run check:static
npm run check:static -- --release-page
npm run check:powershell
npm run test:static
```

通常は1行目でアプリ版・MSIX版・ロックファイル・タイトル・プロジェクトスキル参照を検査します。配布ページを最新に更新した後だけ2行目を使用します。3行目はdev/scripts内のPowerShell構文のみを解析し、対象の処理は実行しません。4行目はcheckerの異常検出試験です。

これらはAI・ネットワーク・録音を使用せず、静的チェック本体はファイルを変更しません。機械処理用には `npm run check:static -- --json` を使用できます。成功時は要約だけ、失敗時は原因だけを確認してください。UI操作・保存・音声の動作確認は別途必要です。

## 共通AIモデル配置（2026-10-13）

Whisperモデルの取得・認識・導入確認は `C:\AI\Models\Speech\Whisper\ScriptSync` を共通の保存先として使用する。Python環境の保存先とモデルの保存先は独立している。
