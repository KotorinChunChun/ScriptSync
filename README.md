# 読みあわせ (ScriptSync)

Markdownやテキストを発話ごとに録音し、良いテイクを選んでFLAC/WAVへ結合出力するWindows向けアプリです。読みの確認、局所補正、合成音声との比較採点も必要に応じて利用できます。

## ダウンロードと対応環境

現在の配布版は0.0.6.2（内部アプリ版0.0.6）です。

- [ScriptSync配布ページ](https://excel-chunchun.sakura.ne.jp/download/ScriptSync/)
- 対応OS：Windows x64
- 標準配布形式：共通証明書で署名したMSIX

旧版から更新する場合も、既存の `.scriptsync` と対応する `.scriptsync.data/` を一組で保持してください。保存形式1～5を現行版で読み込めます。未知の将来形式は推測変換しません。

## 使い始める

**1 台本 → 2 録音 → 3 読み確認・補正 → 4 比較・採点 → 5 結合・出力**

自分の録音をまとめるだけなら、工程3・4は省略できます。「新規プロジェクト」で `.scriptsync` ファイルを作り、原稿を登録して台本を確定し、発話ごとに録音・採用を進めてください。

- [利用者ガイド](docs/USERS_GUIDE.md)：プロジェクト、台本、録音、AI文章改善、トリミング、出力
- [読み上げテスト](samples/reading-test/README.md)：初回確認用の文章と手順
- [よくある質問](docs/FAQ.md)
- [キー操作](dev/SHORTCUT_KEYS.md)・[効果音の定義](dev/SOUND_EFFECTS.md)
- [ScriptSyncWebとのZIP交換](docs/web-exchange.md)・[Webへの台本アップロード](docs/cloud-upload.md)

録音画面では確定台本を保持したまま修正案を編集できます。Whisperの文字起こしとCodex CLIによる文章改善は別操作で、AIの結果は左右比較して承認した場合だけ修正案へ反映します。単語の静的変換辞書、台本文字サイズ、読み上げ対象フィルタ、選択行への追従表示にも対応しています。

## バージョン

| 対象 | 状態 |
| --- | --- |
| v0.0.6 | 公開版0.0.6.2。原稿更新の安全性、テイク編集・移動、待ち時間込みの合計時間表示、編集・未採用出力、発話完全消去と台本遷移の改善を追加。[要求](dev/history/v0.0.6/v0.0.6-req.md)／[実装記録](dev/history/v0.0.6/v0.0.6-imp.md) |
| v0.0.5 | 最終公開版0.0.5.7として終了。録音・再生・台本操作、Whisper／Codex分離、AI改善の承認比較、置換辞書、文字サイズ、行追従を実装。[終了記録](dev/v0.0.5-close.md)／[要求](dev/v0.0.5-req.md)／[実装・検証記録](dev/v0.0.5-imp.md) |
| v0.0.4 | 台本の確定保存、Web交換ZIP、端末設定、文字起こし環境導入を実装。[要求](dev/history/v0.0.4/v0.0.4-req.md)／[実装記録](dev/history/v0.0.4/v0.0.4-imp.md) |
| v0.0.3 | 原稿名変更、同名取込の照合、削除予定、ペイン縮小、行対応付け、アーカイブ管理を実装。[要求](dev/history/v0.0.3/v0.0.3-req.md)／[実装記録](dev/history/v0.0.3/v0.0.3-imp.md) |
| v0.0.2 | 2026-09-12終了。実マイク・連続録音・ASR負荷／音質を含む課題はユーザー確認済み。[終了記録](dev/history/v0.0.2/v0.0.2-README.md) |
| v0.0.1 | 差し込み修正を含め最終完成。タグ `v0.0.1-final`。[完成記録](dev/history/v0.0.1/RELEASE.md) |

自動テストの成功と、実マイク・実再生機器・ネイティブ画面の確認は区別して記録しています。現行の検証方針は[実証・受け入れ方針](docs/validation.md)、版ごとの実施結果と未確認事項は各版の実装記録を参照してください。

## 開発起動

WindowsでPowerShell 7（pwsh）、Node.js/npm、Rust、TauriのWindowsビルド環境を用意し、次を実行します。

```powershell
cd C:\develop\ScriptSync
npm ci
npm run tauri dev
```

UI変更は開発起動中に反映されます。録音中のコード更新は避け、停止・保存してから更新してください。`npm run dev` だけのブラウザー表示では録音や保存はできません。

通常の録音・結合に音声合成CLIやASRは不要です。文字起こし環境、Codex CLI、外部のTextToSpeech CLIを使う開発・検証は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。文書中の「TextToSpeech CLI」は、テキストを音声に変換する外部CLIソフトウェアの抽象名です。

## 資料と開発の入口

- [プロジェクトビジョン](docs/PROJECT_VISION.md)
- [ドキュメント目録](docs/README.md)
- [現行アーキテクチャ](docs/architecture.md)
- [開発者ガイド](docs/DEVELOPERS_GUIDE.md)
- [最新のファイル構成目録](docs/REPOSITORY_MAP.md)
- [AI・開発者向け作業案内](AGENTS.md)
