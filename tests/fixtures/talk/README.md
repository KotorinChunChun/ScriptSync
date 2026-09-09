# CLI側の音声合成エンジン実機検証用fixture

2026-09-08に本プロジェクトの合成検証文「いちばへ行きます。」から作成。

- actual.json：TextToSpeech CLIの実際の解析結果と既定の合成条件。認証情報は含まない。
- generated-format7.tstprj：修正版Rust出力。CLI側の音声合成エンジン 1.3.8.2でボイス・読み・アクセントの表示を確認。
- saved-by-talk-1.3.8.2.tstprj：CLI側の音声合成エンジンで読みをイチ、最初の語のhlをlh、開始時刻を0.100秒に編集して保存。CLI側の音声合成エンジン再起動後の再読込とWAV合成を確認。

詳細はdocs/talk-project-format.mdを参照。第三者の公開サンプルやユーザーの既存プロジェクトは含めない。
