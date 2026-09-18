# 文書目録

現在の最終公開版は0.0.6.1（内部アプリ版0.0.6）です。版別資料は `dev/history/v0.0.6/` に置いています。過去版の記録も `dev/history/vX.Y.Z/` に保存しています。

| 読む人・目的 | 入口 |
|---|---|
| プロジェクトの目的・役割 | [プロジェクトビジョン](PROJECT_VISION.md) |
| 初めて使う | [README](../README.md)、[利用者ガイド](USERS_GUIDE.md) |
| 操作に困った | [FAQ](FAQ.md)、[読み上げテストとサンプル](../samples/reading-test/README.md) |
| 録音キー・音の定義 | [ショートカット](../dev/SHORTCUT_KEYS.md)、[効果音](../dev/SOUND_EFFECTS.md) |
| 開発起動・ビルド・試験 | [開発者ガイド](DEVELOPERS_GUIDE.md) |
| コードを探す | [ファイル構成目録](REPOSITORY_MAP.md)、[現行アーキテクチャ](architecture.md) |
| AIによる保守 | [AGENTS](../AGENTS.md)、[技術知見](../dev/KNOWLEDGE.md) |
| AIの検証・リリース手順 | [変更検証スキル](../.agents/skills/scriptsync-project-validation/SKILL.md)、[リリース確定スキル](../.agents/skills/scriptsync-project-release/SKILL.md) |
| v0.0.3の実装・検証記録 | [v0.0.3実装記録](../dev/history/v0.0.3/v0.0.3-imp.md)、[進捗](../dev/history/v0.0.3/v0.0.3-task.md)、[検証計画・記録](validation.md) |
| v0.0.2の実装・検証を確認する | [v0.0.2入口](../dev/history/v0.0.2/v0.0.2-README.md)、[要求](../dev/history/v0.0.2/v0.0.2-req.md)、[計画](../dev/history/v0.0.2/v0.0.2-imp.md)、[進捗](../dev/history/v0.0.2/v0.0.2-task.md) |
| 録音品質を検証する | [実証・受け入れ方針](validation.md)、[読み上げテスト](../samples/reading-test/README.md) |
| 過去の録音ノイズ修正を確認する | [v0.0.1解決済み補足](../dev/history/v0.0.1/v0.0.1-audio-recording-noise-fix.md) |
| プレゼンテーションソフトへ音声の時間情報を渡す | [音声・字幕・ブロックマップ連携仕様](EXPORT_BLOCK_MAP.md) |
| CLI側の音声合成エンジンの形式を扱う | [CLI側の音声合成エンジンプロジェクト形式](talk-project-format.md) |
| 取り込み元の由来を確認 | [初期調査記録](source-inventory.md)、[録音crateの由来](../crates/scriptsync-recorder/ORIGIN.md) |
| 初期設計を調べる | [初期アーキテクチャ案（履歴）](history/architecture-initial-design.md)、[追加案](../dev/PLAN_IDEAS.md) |

初期案・モックアップは現行機能一覧ではありません。仕様の判断には対象版の要求、実装記録、実コードを照合します。過去の試験結果は実施日・条件と一緒に読み、今回の検証結果と混同しません。

v0.0.3：[要求](../dev/history/v0.0.3/v0.0.3-req.md)／[実装計画](../dev/history/v0.0.3/v0.0.3-imp.md)。

## v0.0.4

[要求](../dev/history/v0.0.4/v0.0.4-req.md)・[実装記録](../dev/history/v0.0.4/v0.0.4-imp.md)・[Web交換ZIP](web-exchange.md)・[クラウド設定](cloud-upload.md)。

## v0.0.5
[終了記録](../dev/v0.0.5-close.md)・[要求](../dev/v0.0.5-req.md)・[実装記録](../dev/v0.0.5-imp.md)。録音・再生・台本操作、Whisper／Codex分離、AI文章改善の承認比較、静的変換辞書、文字サイズ、選択行追従。
