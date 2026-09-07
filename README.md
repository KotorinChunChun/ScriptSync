# 読みあわせ (ScriptSync)

人間の確認済み音声を基準に、合成音声の誤読候補を見つけ、原文の必要箇所だけを補正するWindows向け制作支援ツールです。

## 現在の状態

v0.0.1の開発基盤を準備した段階です。Rust + Tauri v2 + TypeScriptを採用し、音声取込、読み照合、局所補正、比較評価は要求・実装計画に沿って段階的に実装します。

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

