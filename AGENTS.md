# AI・開発者向け作業案内

## 最初に読む資料

1. [README](README.md)：現行版と入口。
2. [文書目録](docs/README.md)・[ファイル構成](docs/REPOSITORY_MAP.md)：対象の資料と実装を特定。
3. 対象版の `dev/vX.Y.Z-req.md`、`-imp.md`、`-task.md`：要求・設計・未完了事項。
4. [開発者ガイド](docs/DEVELOPERS_GUIDE.md)：起動と検証。

v0.0.2は2026-09-12にユーザーの全課題確認・終了指示により終了しました。最終資料は `dev/history/v0.0.2/`、途中記録は同フォルダーの `snapshots/` にあります。v0.0.3の実装・検証が完了しています。文書整備だけを依頼されたときに実装やデータ移行を開始しません。旧版の記録は `dev/history/v0.0.1/` にあります。モックアップ・初期案だけを現行仕様と判断しないでください。

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

v0.0.3：[要求](dev/history/v0.0.3/v0.0.3-req.md)／[実装計画](dev/history/v0.0.3/v0.0.3-imp.md)。

v0.0.4実装：[要求](dev/history/v0.0.4/v0.0.4-req.md)／[実装記録](dev/history/v0.0.4/v0.0.4-imp.md)。v0.0.3資料はdev/history/v0.0.3へ保存済みです。

## 配布形式（必須）

- Windowsの標準配布成果物は既存の共通証明書で署名したMSIX。`npm run release:msix`を使用し、MSIX・公開CER・署名検証結果を提示する。EXE単体やNSIS/MSIだけで配布完了としない。
- 開発確認用の`--no-bundle`は中間ビルド。配布の明示指定がない場合も署名済みMSIXを既定とする。詳細は[MSIX手順](docs/MSIX.md)。
- 旧版互換では入口形式とデータ形式を区別し、既存音声・過去保存世代を保持して検証する。未知の将来形式を版番号だけ変更して取り込まない。

v0.0.5は最終公開版0.0.5.7として終了しました：[終了記録](dev/v0.0.5-close.md)／[要求](dev/v0.0.5-req.md)／[実装記録](dev/v0.0.5-imp.md)。次版開始の指示がないため、資料はdev直下に保持しています。v0.0.4資料はdev/history/v0.0.4へ保存済みです。

## Web公開手順

[リリースルール](dev/RELEASE_RULE.md)を参照。Websiteの `/download/ScriptSync/` へScriptSync専用5ファイルを公開する。リリース指示の範囲と各工程の検証は同書に従う。
