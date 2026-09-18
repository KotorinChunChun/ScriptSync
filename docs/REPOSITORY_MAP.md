# ファイル構成目録

現在の開発版0.0.6.0と最終公開版0.0.5.7の実装を探すための目録です。全生成物や過去版の変更一覧ではありません。文書の入口は[文書目録](README.md)、構造上の判断は[現行アーキテクチャ](architecture.md)、作業規約は[AGENTS](../AGENTS.md)を参照してください。

版ごとの要求・実装・検証結果は `dev/vX.Y.Z-*.md` と `dev/history/vX.Y.Z/` に置き、本書へ版別の追記を重ねません。ファイルの責務が変わったときに本書を更新します。

## UI

UIは `src/` にあります。

| パス | 役割 |
|---|---|
| [main.ts](../src/main.ts) | 画面構築、RPC、工程状態、録音操作、台本・テイクの統合 |
| [models.ts](../src/models.ts) | UIが参照するプロジェクト、原稿、発話、テイク、設定の型 |
| [html.ts](../src/html.ts) | HTML生成時の共通エスケープ |
| [navigation.ts](../src/navigation.ts)、[navigation.css](../src/navigation.css) | 5工程、台本フィルタ、部・章・節ツリー |
| [navigationFollow.ts](../src/navigationFollow.ts) | 前後移動時の選択行追従、祖先展開、スクロール位置 |
| [scriptSources.ts](../src/scriptSources.ts) | 原稿ごとの確定本文・下書き・行選択・対応付け |
| [scriptReview.ts](../src/scriptReview.ts)、[scriptDiff.ts](../src/scriptDiff.ts)、[fullScriptDiff.ts](../src/fullScriptDiff.ts) | 差分の整列、承認、全文比較 |
| [recordingFeedback.ts](../src/recordingFeedback.ts) | 録音画面の修正案を元原稿の下書きへ戻す位置照合 |
| [recordingArchives.ts](../src/recordingArchives.ts) | 原稿・発話のアーカイブ表示、試聴、完全消去確認 |
| [legacySources.ts](../src/legacySources.ts) | 旧形式の既存台本を原稿へ対応付ける補助 |
| [pointerReorder.ts](../src/pointerReorder.ts) | ポインター操作による原稿・行の並べ替え |
| [aiReview.ts](../src/aiReview.ts)、[aiReview.css](../src/aiReview.css) | AI文章改善の前後比較、承認、辞書候補 |
| [takeTrim.ts](../src/takeTrim.ts)、[takeTrim.css](../src/takeTrim.css) | テイク波形、開始・終了位置の編集と保存 |
| [waveScale.ts](../src/waveScale.ts) | 波形表示の時間・振幅スケール |
| [nativeAudio.ts](../src/nativeAudio.ts) | Rustのネイティブ再生とUIの時計・操作を同期 |
| [exportView.ts](../src/exportView.ts)、[exportView.css](../src/exportView.css) | 出力ブロック、境界、命名、間隔、試聴、結合出力 |
| [exchangeView.ts](../src/exchangeView.ts) | ScriptSyncWeb用ZIPの出力・取込 |
| [cloudUploadView.ts](../src/cloudUploadView.ts) | 確定台本の専用Web送信画面 |
| [settingsView.ts](../src/settingsView.ts)、[settingsView.css](../src/settingsView.css) | 録音・再生、Whisper、Codex、静的変換辞書の端末設定 |
| [appInfo.ts](../src/appInfo.ts) | 表示用のアプリ名、内部版、MSIX再配布版 |
| [closeGuard.ts](../src/closeGuard.ts) | 保存待ち・保存失敗・未保存操作の終了確認 |
| [confirmAction.ts](../src/confirmAction.ts) | 削除など破壊操作の確認UI |
| [transientError.ts](../src/transientError.ts) | Whisper・Codex等の一時エラー表示 |
| [reloadProtection.ts](../src/reloadProtection.ts) | 誤リロード防止と録音中の保護 |
| [soundEffects.ts](../src/soundEffects.ts) | 効果音のネイティブ再生 |
| [readingTest.ts](../src/readingTest.ts) | 読み上げ試験用プロジェクトの案内 |
| [style.css](../src/style.css)、[buttonSizes.css](../src/buttonSizes.css)、[scriptWorkspace.css](../src/scriptWorkspace.css) | 共通外観、ボタン規格、台本・録音レイアウト |

## Rustバックエンド

バックエンドは `src-tauri/src/` にあります。

| パス | 役割 |
|---|---|
| [lib.rs](../src-tauri/src/lib.rs)、[main.rs](../src-tauri/src/main.rs) | アプリ起動、ウィンドウ、Tauriコマンド登録 |
| [app.rs](../src-tauri/src/app.rs) | UI操作、保存、録音、再生、外部処理を統合する入口 |
| [domain.rs](../src-tauri/src/domain.rs) | 保存形式5のデータモデルと互換フィールド |
| [storage.rs](../src-tauri/src/storage.rs) | 保存世代、索引、ロック、旧形式移行、整合性検査 |
| [markdown.rs](../src-tauri/src/markdown.rs) | Markdownを部・章・節・発話へ解析 |
| [script_sources.rs](../src-tauri/src/script_sources.rs) | 原稿下書き、録音修正、行対応、台本確定 |
| [editing.rs](../src-tauri/src/editing.rs)、[script_merge.rs](../src-tauri/src/script_merge.rs) | 発話編集と原稿差分の補助処理 |
| [archive.rs](../src-tauri/src/archive.rs) | 原稿・発話のアーカイブと全世代からの完全消去 |
| [project_location.rs](../src-tauri/src/project_location.rs) | プロジェクト選択の既定フォルダー |
| [project_open.rs](../src-tauri/src/project_open.rs) | 起動引数・関連付けからのプロジェクト読込 |
| [project_rename.rs](../src-tauri/src/project_rename.rs) | 入口ファイルと所有データの安全な改名 |
| [audio.rs](../src-tauri/src/audio.rs)、[media.rs](../src-tauri/src/media.rs) | 音声加工、音声配信、hash検査 |
| [take_trim.rs](../src-tauri/src/take_trim.rs) | トリム確定と音声実体・参照の更新 |
| [native_playback.rs](../src-tauri/src/native_playback.rs) | 指定Windows endpointへの再生 |
| [hotkeys.rs](../src-tauri/src/hotkeys.rs)、[ui_session.rs](../src-tauri/src/ui_session.rs) | グローバルキーとUI生存監視 |
| [window_state.rs](../src-tauri/src/window_state.rs) | ウィンドウ位置・サイズの保存と画面外補正 |
| [machine_settings.rs](../src-tauri/src/machine_settings.rs) | プロジェクト外の端末設定、辞書、原子的保存 |
| [asr_runtime.rs](../src-tauri/src/asr_runtime.rs)、[asr_authenticode.rs](../src-tauri/src/asr_authenticode.rs) | Whisper専用環境の導入・確認・削除と署名検証 |
| [asr.rs](../src-tauri/src/asr.rs)、[automatic_asr.rs](../src-tauri/src/automatic_asr.rs) | テイクの文字起こしと整合性を固定した認識キュー |
| [text_improvement.rs](../src-tauri/src/text_improvement.rs) | 静的辞書前処理、Codex CLI呼出し、古い結果の拒否 |
| [alignment.rs](../src-tauri/src/alignment.rs)、[rules.rs](../src-tauri/src/rules.rs)、[queue.rs](../src-tauri/src/queue.rs) | 読み対応、補正ルール、比較処理キュー |
| [speech.rs](../src-tauri/src/speech.rs)、[talk_project.rs](../src-tauri/src/talk_project.rs) | TextToSpeech CLI呼出しと編集形式の生成・検証 |
| [export_blocks.rs](../src-tauri/src/export_blocks.rs)、[export_settings.rs](../src-tauri/src/export_settings.rs) | 分割、命名、間隔、結合、字幕、ブロックマップ、出力設定 |
| [exchange.rs](../src-tauri/src/exchange.rs) | Web交換ZIPの生成、検証、取込 |
| [cloud_upload.rs](../src-tauri/src/cloud_upload.rs) | 暗号化ZIPの専用API送信と失敗復旧 |

## 録音基盤

`crates/scriptsync-recorder/` はローカル録音基盤です。[ORIGIN.md](../crates/scriptsync-recorder/ORIGIN.md)に由来と取り込み方針を記録しています。

| パス | 役割 |
|---|---|
| `src/windows/wasapi_capture.rs` | WASAPI shared event入力、不連続・時刻・欠落の観測 |
| `src/windows/device_format.rs` | チャンネル・サンプル形式変換と連続リサンプリング |
| `src/windows/default_device.rs`、`devices.rs` | 既定・明示デバイスの解決と一覧取得 |
| `src/windows/wasapi_render.rs` | 指定endpointへの音声出力 |
| `src/windows/microphone_recording.rs` | 録音セッション、診断、保存結果の統合 |
| `src/recording/writer.rs`、`codecs.rs` | 音声バッファの書込とWAV／FLAC処理 |
| `src/audio/` | バッファ、時計、形式、メーター、リサンプラー |

## 試験、ツール、文書

| パス | 用途 |
|---|---|
| `tests/ui/` | VitestによるUI・RPC・非同期状態の回帰試験 |
| `src-tauri/src/*tests.rs` と各モジュール内tests | Rustの保存、境界、移行、音声、外部形式試験 |
| `tests/fixtures/` | 非機密の契約・形式fixture |
| [samples/reading-test/README.md](../samples/reading-test/README.md) | 人間が行う録音・読み上げ確認 |
| `tools/asr/` | アプリ専用Whisper環境で使うPythonワーカー |
| `dev/scripts/` | 静的検査、ASR準備、fixture生成、MSIX、公開補助 |
| [DEVELOPERS_GUIDE.md](DEVELOPERS_GUIDE.md) | 起動、試験、外部環境の開発手順 |
| [validation.md](validation.md) | 今後の版にも適用する検証・受け入れ方針 |
| [MSIX.md](MSIX.md) | 署名付きMSIXの生成・検証手順 |
| `dev/v0.0.5-*.md` | 現行版の要求と実装・検証記録 |
| `dev/history/vX.Y.Z/` | 終了した版の要求、実装、進捗、補足資料 |
| `.agents/skills/scriptsync-project-validation/` | 変更領域に応じた検証選択 |
| `.agents/skills/scriptsync-project-release/` | 完成版・成果物・Git状態の対応付け |

## 生成物と利用者データ

`node_modules/`、`dist/`、`src-tauri/target/`、`release/` は依存・ビルド・配布生成物です。`output/` は非機密fixtureや検証証拠の一時出力先です。いずれも現行ソースの正本ではありません。

利用者のプロジェクトは `.scriptsync` と `.scriptsync.data/` を一組で扱い、音声や索引だけを個別に書き換えません。`dev/sample/`、原稿、録音、認証情報を自動整理・一括追加・試験流用しません。端末設定、Whisper環境、Codex認証、外部CLI設定はプロジェクトデータと分離します。
