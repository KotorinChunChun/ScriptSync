# KotorinToysへの収録

2026-10-24。ScriptSync（読みあわせ）の開発正本は `C:\develop\KotorinToys\tools\ScriptSync`。独立Gitと [ScriptSyncリポジトリ](https://github.com/KotorinChunChun/ScriptSync) を維持し、親は採用コミットをgitlinkで記録する。

## 現在の対応範囲

KotorinToysのローカル一覧と配信用一覧のソースに、[tool.json](../tool.json) の定義を登録する。使い方・GitHubへの導線を用意する。ローカル変更だけでは公開済み本体の一覧は変わらない。

本体管理用の状態取得・画面表示・正常停止・無表示起動の通信口は未実装。`capabilities` は空配列であり、本体からの起動・停止と自動起動を利用できない。配布状態 `unpublished` はKotorinToys対応インストーラーとして未公開という意味であり、単体の署名付きMSIX公開状態とは別である。MSIXを汎用Inno導入へ渡さない。

`appId: KotorinToys.ScriptSync` は本体一覧の登録識別子。既存MSIXの `jp.kotorinchunchun.scriptsync`、Publisher、Application Id、保存先を変更するものではない。既存の `.scriptsync` と対応する `.scriptsync.data/` は一組で保持する。ソース移動で個人の原稿・録音・アプリ設定を移行しない。

## 単体で利用する

配布版は [配布ページ](https://excel-chunchun.sakura.ne.jp/download/ScriptSync/) の署名付きMSIXを使用する。操作は [利用者ガイド](USERS_GUIDE.md)、キーの有効条件は [キー定義](../dev/SHORTCUT_KEYS.md) を参照する。

開発起動は移動先で行う。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm run tauri dev
```

ビルド・前提環境は [開発者ガイド](DEVELOPERS_GUIDE.md)、パッケージ作成は [MSIX手順](MSIX.md) に従う。`tool.json` の実行先は既存ビルドの `src-tauri/target/release/scriptsync.exe`。配置済みの古いEXEを、今回ビルド・検証したものとは扱わない。

## 本体の開発一覧で確認する

```powershell
cd C:\develop\KotorinToys
pwsh -File dev/scripts/start.ps1 -LocalCatalog
```

一覧の「すべて」または「未導入」でScriptSyncを選ぶ。起動中の本体がある場合はその画面が再利用されるため、一覧モードを切り替えるには先に本体を終了する。管理操作の無効状態と使い方への導線を確認する。
