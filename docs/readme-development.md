# 版別記録と開発の入口

## バージョン

| 対象 | 状態 |
| --- | --- |
| v0.0.6 | 公開版0.0.6.5。同一原稿内のテイク移動と移動先表示を改善。原稿更新の安全性、テイク編集・移動、待ち時間込みの合計時間表示、編集・未採用出力、発話完全消去と台本遷移の改善を追加。[要求](../dev/history/v0.0.6/v0.0.6-req.md)／[実装記録](../dev/history/v0.0.6/v0.0.6-imp.md) |
| v0.0.5 | 最終公開版0.0.5.7として終了。録音・再生・台本操作、Whisper／Codex分離、AI改善の承認比較、置換辞書、文字サイズ、行追従を実装。[終了記録](../dev/v0.0.5-close.md)／[要求](../dev/v0.0.5-req.md)／[実装・検証記録](../dev/v0.0.5-imp.md) |
| v0.0.4 | 台本の確定保存、Web交換ZIP、端末設定、文字起こし環境導入を実装。[要求](../dev/history/v0.0.4/v0.0.4-req.md)／[実装記録](../dev/history/v0.0.4/v0.0.4-imp.md) |
| v0.0.3 | 原稿名変更、同名取込の照合、削除予定、ペイン縮小、行対応付け、アーカイブ管理を実装。[要求](../dev/history/v0.0.3/v0.0.3-req.md)／[実装記録](../dev/history/v0.0.3/v0.0.3-imp.md) |
| v0.0.2 | 2026-09-12終了。実マイク・連続録音・ASR負荷／音質を含む課題はユーザー確認済み。[終了記録](../dev/history/v0.0.2/v0.0.2-README.md) |
| v0.0.1 | 差し込み修正を含め最終完成。タグ `v0.0.1-final`。[完成記録](../dev/history/v0.0.1/RELEASE.md) |

自動テストの成功と、実マイク・実再生機器・ネイティブ画面の確認は区別して記録しています。現行の検証方針は[実証・受け入れ方針](validation.md)、版ごとの実施結果と未確認事項は各版の実装記録を参照してください。

## 開発起動

WindowsでPowerShell 7（pwsh）、Node.js/npm、Rust、TauriのWindowsビルド環境を用意し、次を実行します。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci
npm run tauri dev
```

UI変更は開発起動中に反映されます。録音中のコード更新は避け、停止・保存してから更新してください。`npm run dev` だけのブラウザー表示では録音や保存はできません。

自動検証は `npm run verify`、実WebView2まで含める場合は `npm run verify:native`。Computer Useを使わない検証の範囲・無音の実機再生試験・結果の保存先は[自動検証ガイド](automated-testing.md)を参照してください。

通常の録音・結合に音声合成CLIやASRは不要です。文字起こし環境、Codex CLI、外部のTextToSpeech CLIを使う開発・検証は[開発者ガイド](DEVELOPERS_GUIDE.md)を参照してください。文書中の「TextToSpeech CLI」は、テキストを音声に変換する外部CLIソフトウェアの抽象名です。

## 資料と開発の入口

- [プレゼン連携試作](PRESENTATION_PROTOTYPE.md)：`npm run presentation:dev` で起動。音声出力とPowerPoint・OBSの対応付け、再生、テンプレートPPTX生成、ローカルAPIを試せます。配布版への組込み前の独立した試作です。

- [プロジェクトビジョン](PROJECT_VISION.md)
- [ドキュメント目録](README.md)
- [現行アーキテクチャ](architecture.md)
- [開発者ガイド](DEVELOPERS_GUIDE.md)
- [最新のファイル構成目録](REPOSITORY_MAP.md)
- [AI・開発者向け作業案内](../AGENTS.md)
