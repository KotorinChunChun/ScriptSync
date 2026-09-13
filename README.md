# 読みあわせ (ScriptSync)

Markdownやテキストを発話ごとに録音し、良いテイクを選んでFLAC/WAVへ結合出力するWindows向けアプリです。必要に応じて、読みの確認・局所補正・合成音声の比較採点も行えます。

## 使い始める

**1 台本 → 2 録音 → 3 読み確認・補正 → 4 比較・採点 → 5 結合・出力**

自分の録音をまとめるだけなら、工程3・4は省略できます。「新規プロジェクト」で `.scriptsync` ファイルを作り、台本を追加して録音・採用を進めてください。

- [利用者ガイド](docs/USERS_GUIDE.md)：プロジェクト、録音、トリミング、出力
- [読み上げテスト](samples/reading-test/README.md)：初回確認用の文章と手順
- [よくある質問](docs/FAQ.md)
- [キー操作](dev/SHORTCUT_KEYS.md)・[効果音の定義](dev/SOUND_EFFECTS.md)

## バージョン

| 対象 | 状態 |
| --- | --- |
| v0.0.1 | 差し込み修正を含め最終完成。`feature/v0.0.1`、タグ `v0.0.1-final`。[完成記録](dev/history/v0.0.1/RELEASE.md) |
| v0.0.3 | 実装・検証完了。原稿名変更、同名取込への照合、削除予定、ペイン縮小、行対応付け、アーカイブ管理。[実装記録](dev/history/v0.0.3/v0.0.3-imp.md) |
| v0.0.2 | 2026-09-12終了。`feature/v0.0.2`。現タスクの課題はユーザー確認済み・問題なし。[終了記録](dev/history/v0.0.2/v0.0.2-README.md) |

現在の配布版は0.0.5.5（内部アプリ版0.0.5）です。[ダウンロード](https://excel-chunchun.sakura.ne.jp/download/ScriptSync/)。録音の読み上げ対象フィルタ、確定台本を保持する修正案編集、新アイコン、設定とタイトルバーの版情報を反映しました。これまでのバグ修正の総まとめとして、既定マイク・再生先・画面配置を修正し、Whisper文字起こしと任意のCodex文章改善を分離しました。[v0.0.5実装記録](dev/v0.0.5-imp.md)を参照してください。v0.0.4では台本の確定保存、Web交換ZIP、端末設定と文字起こし環境導入を追加しました。[実装・検証記録](dev/history/v0.0.4/v0.0.4-imp.md)を参照してください。従来の主な機能は、発話別テイク、通常／連続録音キー、停止延長、手動トリム、階層別・手動分割出力、音量均一化です。

## 開発起動

WindowsでPowerShell 7（pwsh）、Node.js/npm、Rust、TauriのWindowsビルド環境を用意し、次を実行します。

```powershell
cd C:\develop\ScriptSync
npm ci
npm run tauri dev
```

UI変更は開発起動中に反映されます。録音中のコード更新は避け、停止・保存してから更新してください。`npm run dev` だけのブラウザー表示では録音や保存はできません。

通常の録音・結合にCLI側の音声合成エンジンやASRは不要です。読み認識・合成比較の環境は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。文字起こしは設定画面からアプリ専用環境を準備します。比較合成用の外部CLIは別途必要です。

文書中の「TextToSpeech CLI」は、テキストを音声に変換する外部CLIソフトウェアの抽象名です。

## 資料と開発の入口

- [プロジェクトビジョン](docs/PROJECT_VISION.md)
- [ドキュメント目録](docs/README.md)
- [開発者ガイド](docs/DEVELOPERS_GUIDE.md)
- [ファイル構成・変更箇所の目録](docs/REPOSITORY_MAP.md)
- [AI作業の入口](AGENTS.md)
- v0.0.2：[要求](dev/history/v0.0.2/v0.0.2-req.md)／[計画](dev/history/v0.0.2/v0.0.2-imp.md)／[タスク](dev/history/v0.0.2/v0.0.2-task.md)
- [v0.0.1の確定時検証記録](dev/history/v0.0.1/v0.0.1-imp.md)

自動テストの成功と実マイクの可聴品質は別に扱います。v0.0.2の実マイク・連続録音・ASR負荷／音質はユーザーから確認済み・問題なしとの報告を受領しました。過去の試験条件と記録は保持しています。[検証記録と手順](docs/validation.md)を参照してください。

v0.0.4：[要求](dev/history/v0.0.4/v0.0.4-req.md)／[実装記録](dev/history/v0.0.4/v0.0.4-imp.md)。

- [Web交換ZIPの使い方](docs/web-exchange.md)
- [クラウド連携の設定](docs/cloud-upload.md)
