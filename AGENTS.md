# AI・開発者向け作業案内

## 最初に読む資料

1. [README](README.md)：現行版と入口。
2. [文書目録](docs/README.md)・[ファイル構成](docs/REPOSITORY_MAP.md)：対象の資料と実装を特定。
3. 対象版の `dev/vX.Y.Z-req.md`、`-imp.md`、`-task.md`：要求・設計・未完了事項。
4. [開発者ガイド](docs/DEVELOPERS_GUIDE.md)：起動と検証。

v0.0.2は2026-09-12にユーザーの全課題確認・終了指示により終了しました。最終資料は `dev/history/v0.0.2/`、途中記録は同フォルダーの `snapshots/` にあります。次版は未開始です。文書整備だけを依頼されたときに実装やデータ移行を開始しません。旧版の記録は `dev/history/v0.0.1/` にあります。モックアップ・初期案だけを現行仕様と判断しないでください。

## 作業規約

- ユーザーの明示指示を優先します。質問だけなら回答し、変更を開始しません。
- PowerShellは `pwsh` を使用します。文書とコミットメッセージは日本語です。
- 作業前にGit差分を確認し、既存変更・個人データを保持します。`dev/sample/`、原稿・録音・認証情報を一括追加しません。
- 大きな実装はフェーズとチェックリストに分けます。要求はreq、AIの設計・実装結果はimp、進捗はtaskに記録します。
- 実装フェーズごとに関連ビルド・試験を行い、問題なければコミットします。文書のみの場合は参照・記述・差分を検査し、未実行の試験を成功扱いしません。
- 次の版へ進む際は旧版文書を `dev/history/` へ保存して新しい版の文書を作成します。
- キーや効果音変更時は [SHORTCUT_KEYS](dev/SHORTCUT_KEYS.md)・[SOUND_EFFECTS](dev/SOUND_EFFECTS.md) を更新します。利用者の操作変更はガイド・サンプルにも反映します。
- 自動試験と、実マイク音質・ネイティブ画面の実機確認を区別して報告します。
- 利用者に試験を依頼するときは、起動済みでなければ `cd C:\develop\ScriptSync` と `npm run tauri dev` を案内します。

保存の正本や音声の扱いは [architecture](docs/architecture.md)、保守上の要点は [KNOWLEDGE](dev/KNOWLEDGE.md) を参照してください。

## プロジェクト専用スキル

- 変更の検証には [scriptsync-project-validation](.agents/skills/scriptsync-project-validation/SKILL.md) を使用します。文書・UI・録音・出力から必要な検証を選びます。
- 完成版の確定や最新ビルドの起動には [scriptsync-project-release](.agents/skills/scriptsync-project-release/SKILL.md) を使用します。

仕様の正本は既存の要求・定義書です。スキルは確認手順を管理し、版番号・キー表・出力形式を複製しません。スキルの利用だけでは次版実装、公開、個人データ変更の許可になりません。
