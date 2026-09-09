# 読みあわせ (ScriptSync)

人間の確認済み音声を基準に、合成音声の誤読候補を見つけ、原文の必要箇所だけを補正するWindows向け制作支援ツールです。

## 現在の状態

v0.0.1 完成確定版（2026-09-09）。複数Markdownの差分マージによる台本編集はv0.0.2で対応予定です。Rust + Tauri v2 + TypeScriptで、台本の発話単位録音・テイク選択・WAV結合、読み確認・局所補正・合成比較・採点保存を実装しています。収録ショートカット・テイクメモ・要確認一覧、波形トリム・音量調整、台本改訂、一括ASR／合成、区間比較、補正事例検索、読点比較、検証付きルール出力に対応します。Talk編集ファイルは試験対応で、Talk 1.3.8.2・田中さん2.0.1の既定条件で実画面の編集・保存・再起動・再合成を確認済みです。ローカルASRは確認用の候補を作り、正解の自動判定はしません。操作は[利用者ガイド](docs/USERS_GUIDE.md)、検証方法は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。

開発ブランチは `feature/v0.0.2`（文書準備済み・実装未着手）。完成版は `feature/v0.0.1` とタグ `v0.0.1` に固定しています。

## 開発文書

- [v0.0.2 要求定義](dev/v0.0.2-req.md)
- [v0.0.2 実装計画](dev/v0.0.2-imp.md)
- [v0.0.2 タスク表](dev/v0.0.2-task.md)
- [v0.0.2 実装開始案内](dev/v0.0.2-README.md)
- [v0.0.1 完成版の実装・検証記録](dev/history/v0.0.1/v0.0.1-imp.md)
- [データ・連携仕様](docs/architecture.md)
- [実証・受け入れ計画](docs/validation.md)
- [調査根拠](docs/source-inventory.md)

## 開発環境

- Windows
- Rust 2024 edition
- Tauri v2
- Node.js / TypeScript / Vite

```powershell
npm install
npm run tauri dev
```

音声生成では外部の `amekoro-speech.exe` とローカル設定を参照します。認証情報、録音、原稿、生成音声、実行ログはGitへ含めません。

読み上げテストは[台本・手順・結果記入用](samples/reading-test/README.md)から開始できます。アプリ内にもテスト台本の選択と発話別ガイドがあります。
