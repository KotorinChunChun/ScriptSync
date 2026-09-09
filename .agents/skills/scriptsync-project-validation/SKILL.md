---
name: scriptsync-project-validation
description: ScriptSyncのUI・録音・保存・音声出力・文書変更を、変更範囲に応じて検証する。実装後の回帰確認や検証レビューに使用し、仕様検討だけの依頼で実装を開始しない。
---

# ScriptSyncの変更検証

## 準備と判断

リポジトリルートの [AGENTS.md](../../../AGENTS.md)、[開発者ガイド](../../../docs/DEVELOPERS_GUIDE.md)、対象版の要求・実装計画を確認する。コマンドはルートを作業ディレクトリにしてpwshで実行する。対象版はREADMEと実際のmanifestで判断し、次版の計画があるだけで実装済みとみなさない。

`git status --short` と差分から変更範囲を特定する。ユーザーの原稿・録音・dev/sampleは検証fixtureへ流用せず、必要なら専用の非機密fixtureを作る。旧メモリの検証手順と異なる場合は現行コード・開発者ガイドを照合し、このプロジェクト用手順を使う。

## 検証を選ぶ

| 変更 | 実施する検証 |
|---|---|
| 文書・スキルのみ | 相対リンク、現行仕様との一致、frontmatter、git diff --check。アプリの再ビルドは不要 |
| UI・CSS・キー操作 | 関連UI試験、npm test、npm run build。配置変更は実ブラウザーでも確認 |
| Rustの保存・音声加工・出力 | 関連試験と cargo test --manifest-path src-tauri/Cargo.toml。RPC変更はUI側も検証 |
| 録音crate | cargo test --manifest-path crates/scriptsync-recorder/Cargo.toml --locked --target-dir src-tauri/target と呼出側の関連試験 |
| ASR・CLI側の音声合成エンジン | 開発者ガイドの対象外部環境試験のみ追加。通常録音・結合の検証に外部環境を必須化しない |
| 配布成果物・起動 | [リリース手順](../scriptsync-project-release/SKILL.md)を読む |

新規lockfileの生成が必要な変更では通常のCargo試験で解決し、差分を確認してからlocked検証する。既存lockfileを試験の都合だけで書き換えない。試験件数は固定せず実行結果を記録する。

UI・録音・出力に変更がある場合は [回帰確認](references/regressions.md) の該当部分を読む。CLI側の音声合成エンジンの形式変更は [実アプリ保存形式](../../../docs/talk-project-format.md) を参照し、実アプリの保存ファイルと比較する。

## 環境エラーと終了条件

- esbuildの親ディレクトリアクセス拒否はソース不良と決めつけず、許可された環境で再試行する。依存取得・権限の障害はコンパイルエラーと区別する。
- Git所有者警告は確認した当該リポジトリに限って `git -c safe.directory=<ルート> ...` を使える。全体設定を広げない。
- 関連試験が通った後、変更・失敗・未解決の懸念がない限り同じ試験を繰り返さない。

結果は対象版のimpへ、必要な進捗はtaskへ日本語で記録する。今回実行した試験、過去結果の参照、未実施を分ける。ブラウザーのRPCモック・EXE起動だけでネイティブ録音、ダイアログ、非アクティブ時キー、音質を確認済みにしない。必要な実機検証は [読み上げテスト](../../../samples/reading-test/README.md) と [ノイズ手順](../../../dev/audio-recording-noise-fix-procedure.md) を案内する。
