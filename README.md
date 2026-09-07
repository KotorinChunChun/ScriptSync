# 読みあわせ (ScriptSync)

人間の確認済み音声を基準に、合成音声の誤読候補を見つけ、原文の必要箇所だけを補正するWindows向け制作支援ツールです。

## 現在の状態

v0.0.1。Rust + Tauri v2 + TypeScriptで、台本の発話単位録音・テイク選択・WAV結合、読み確認・局所補正・合成比較・採点保存を実装しています。ローカルASRは確認用の候補を作り、正解の自動判定はしません。操作は[利用者ガイド](docs/USERS_GUIDE.md)、検証方法は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。

## 開発文書

- [要求定義](dev/v0.0.1-req.md)
- [実装計画](dev/v0.0.1-imp.md)
- [タスク表](dev/v0.0.1-task.md)
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
