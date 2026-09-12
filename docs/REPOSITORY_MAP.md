# ファイル構成目録

v0.0.2完成版の実装を探すための目録です。全生成物の一覧ではありません。文書の入口は[文書目録](README.md)、変更時の規約は[AGENTS](../AGENTS.md)です。

## UI

| パス | 役割 |
|---|---|
| [src/main.ts](../src/main.ts) | 画面構築、RPC、録音状態、台本編集、キー操作、読み確認・採点 |
| [src/navigation.ts](../src/navigation.ts) / navigation.css | 5工程・ツリー・フィルターの表示 |
| [src/exportView.ts](../src/exportView.ts) / exportView.css | 出力ブロック、境界、命名、間隔、試聴と出力 |
| [src/takeTrim.ts](../src/takeTrim.ts) / takeTrim.css | テイク波形、開始終了編集 |
| [src/soundEffects.ts](../src/soundEffects.ts) | 効果音再生。素材・ライセンスはpublic/sounds |
| [src/reloadProtection.ts](../src/reloadProtection.ts) | 誤リロード防止 |
| [src/readingTest.ts](../src/readingTest.ts) | 読み上げ試験の台本・案内 |
| [src/style.css](../src/style.css) | 共通レイアウト・外観 |

## Rustバックエンド

各ファイルは `src-tauri/src/` にあります。

| ファイル | 役割 |
|---|---|
| [lib.rs](../src-tauri/src/lib.rs)、main.rs | アプリ起動、ウィンドウ、コマンド登録 |
| [app.rs](../src-tauri/src/app.rs) | UIからの操作、録音・保存・プロジェクト処理の統合 |
| [domain.rs](../src-tauri/src/domain.rs)、[storage.rs](../src-tauri/src/storage.rs) | データモデル、保存世代、プロジェクトファイル、整合性 |
| [markdown.rs](../src-tauri/src/markdown.rs)、editing.rs | Markdown解析・確定、編集。ファイル別の差分・確定はscript_sources.rsと連携 |
| [audio.rs](../src-tauri/src/audio.rs)、take_trim.rs、media.rs | 音声加工、トリム保存、音声配信 |
| [export_blocks.rs](../src-tauri/src/export_blocks.rs)、export_settings.rs | 分割・命名・間隔・結合・出力設定 |
| [hotkeys.rs](../src-tauri/src/hotkeys.rs)、ui_session.rs、window_state.rs | キー登録、UI生存確認、ウィンドウ状態 |
| [asr.rs](../src-tauri/src/asr.rs)、speech.rs | ローカル認識、外部音声CLI |
| [alignment.rs](../src-tauri/src/alignment.rs)、rules.rs、queue.rs | 読み対応、補正ルール、処理キュー |
| [talk_project.rs](../src-tauri/src/talk_project.rs) | CLI側の音声合成エンジン形式の生成・検証 |

プロジェクト選択の既定フォルダーは [project_location.rs](../src-tauri/src/project_location.rs) が保存・再読込を担当します。VTT字幕はexport_blocks.rsで音声と共通の時間軸から生成します。

## 試験・補助・設定

| パス | 用途 |
|---|---|
| [crates/scriptsync-recorder/ORIGIN.md](../crates/scriptsync-recorder/ORIGIN.md) | ローカル録音基盤の由来。実装は同crateのsrc |
| tests/ui/ | VitestのUI回帰試験 |
| src-tauri/src/*tests.rs、各モジュール内tests | Rustの境界・機能・ワークフロー試験 |
| tests/fixtures/ | 契約・形式の非機密fixture |
| [samples/reading-test/README.md](../samples/reading-test/README.md) | 人間が読むサンプル・試験手順・結果記入用 |
| tools/asr/ | ローカル認識用Pythonワーカー |
| dev/scripts/ | ASR準備・合成fixture・コーパス検証スクリプト |
| package.json、package-lock.json | UIコマンド・依存・アプリ版 |
| src-tauri/Cargo.toml、Cargo.lock、tauri.conf.json | Rust依存・アプリ版・ウィンドウ・ビルド設定 |
| dev/history/v0.0.2/v0.0.2-*.md | v0.0.2の最終要求・実装記録・進捗。途中記録は同フォルダーのsnapshots |
| dev/history/v0.0.1/ | 確定版の要求・実装履歴 |
| dev/mockups/ | 設計確認用HTML。実アプリとは別 |
| dev/templates/ | 文書テンプレート |
| .agents/skills/scriptsync-project-validation/ | プロジェクト専用の変更検証手順と回帰確認 |
| .agents/skills/scriptsync-project-release/ | プロジェクト専用のリリース確定手順 |

## 生成物・利用者データ

`node_modules/`、`dist/`、`src-tauri/target/`、`release/`は依存・ビルド生成物です。`output/`は検証用プロジェクトや音声等の出力先です。利用者が別フォルダーへ保存する場合もあります。

プロジェクトは `.scriptsync` と対応する `.scriptsync.data/` を一組で扱います。データ内の `revisions/` が正本です。音声や索引を個別に書き換えて整合性を直したことにしないでください。

`dev/sample/` はユーザー提供資料です。現行録音コードは `crates/scriptsync-recorder/` 側にあります。原資料を自動整理・削除・一括コミットしません。外部CLIのソースや個人認証設定は本リポジトリの一部ではありません。

## v0.0.2の追加モジュール

- src/scriptSources.ts：ファイル別下書き・差分・保存payload。
- src/recordingArchives.ts：廃棄行の試聴・完全消去確認。
- src-tauri/src/script_sources.rs：台本下書き・全体確定・空行・旧台本対応付け。
- src-tauri/src/automatic_asr.rs：録音保存後の認識キューと空欄への入力。
- src-tauri/src/archive.rs：履歴を含む完全消去と共有音声保護。

### 共通UI基盤

- [src/html.ts](../src/html.ts)：画面共通のHTML本文・引用符付き属性のエスケープ。
- [src/models.ts](../src/models.ts)：UIで参照するテイク・発話・認識結果・スナップショットの型。保存形式の正本はRust側にある。

## v0.0.3の変更箇所

- src/scriptSources.ts：同名取込の下書き生成、行対応の候補と新規選択。
- src/main.ts、src/scriptWorkspace.css：原稿名編集、削除予定、ペイン縮小、差分操作。
- src/recordingArchives.ts：原稿・行のアーカイブ管理画面、試聴、確認付き完全消去。
- src-tauri/src/script_sources.rs、domain.rs、storage.rs：保存形式5、原稿削除の確定と保持。
- src-tauri/src/archive.rs：原稿・行の全世代消去と再試行。
