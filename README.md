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
| v0.0.2 | `feature/v0.0.2` で要求・計画を準備済み。コード実装は未着手 |

現在の実行時バージョンは0.0.1です。複数Markdownの差分マージ・手動対応付けは0.0.2の予定であり、現在は使用できません。0.0.1の主な機能は、発話別テイク、通常／連続録音キー、停止延長、手動トリム、階層別・手動分割出力、音量均一化です。

## 開発起動

WindowsでPowerShell 7（pwsh）、Node.js/npm、Rust、TauriのWindowsビルド環境を用意し、次を実行します。

```powershell
cd C:\develop\ScriptSync
npm ci
npm run tauri dev
```

UI変更は開発起動中に反映されます。録音中のコード更新は避け、停止・保存してから更新してください。`npm run dev` だけのブラウザー表示では録音や保存はできません。

通常の録音・結合にCLI側の音声合成エンジンやASRは不要です。読み認識・合成比較の環境は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。現状は開発PCの外部CLI・ASR配置を利用しており、EXE単独の他PC配布を保証するパッケージではありません。

文書中の「TextToSpeech CLI」は、テキストを音声に変換する外部CLIソフトウェアの抽象名です。

## 資料と開発の入口

- [ドキュメント目録](docs/README.md)
- [開発者ガイド](docs/DEVELOPERS_GUIDE.md)
- [ファイル構成・変更箇所の目録](docs/REPOSITORY_MAP.md)
- [AI作業の入口](AGENTS.md)
- v0.0.2：[要求](dev/v0.0.2-req.md)／[計画](dev/v0.0.2-imp.md)／[タスク](dev/v0.0.2-task.md)
- [v0.0.1の確定時検証記録](dev/history/v0.0.1/v0.0.1-imp.md)

自動テストの成功と実マイクの可聴品質は別に扱います。WASAPIノイズ対策は実装済みですが、30秒×5回の実機比較は未完了です。[検証記録と手順](docs/validation.md)を参照してください。
