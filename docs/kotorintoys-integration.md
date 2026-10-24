# KotorinToysの管理連携

2026-10-24。ScriptSync（読みあわせ）の開発正本は `C:\develop\KotorinToys\tools\ScriptSync`。独立Gitと [ScriptSyncリポジトリ](https://github.com/KotorinChunChun/ScriptSync) を維持し、親は採用コミットをgitlinkで記録する。

## 対応する操作

本体の開発一覧から、管理用EXEの起動・画面表示・状態確認・正常停止に対応する。[tool.json](../tool.json) が4能力を宣言する。通常起動はメイン画面を表示し、本体の `--startup --launch-origin host_manual` 起動は非表示で待機する。起動済みの場合、本体は既存PIDを維持して画面を表示し、最小化状態も復元する。本体を終了しても子は動作を続ける。

停止時は画面側とバックエンドの両方を照合する。録音・保存・文字起こし・一括処理・連続録音モード・原稿や設定の未保存入力・確認／編集ダイアログが残っている場合は拒否する。作業を完了してから再試行する。強制終了や未保存内容の自動破棄へ切り替えない。停止受理後は新しい操作を遮断し、試聴とホットキーを終了して画面状態を保存する。

## ビルドと確認

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
pwsh -File dev/scripts/build-debug-ScriptSync-kotorintoys.ps1
```

スクリプトは画面を内包したDebug EXEを `target/kotorintoys/scriptsync.exe` に配置する。Viteサーバーは不要。Rust／Node.js等の前提は [開発者ガイド](DEVELOPERS_GUIDE.md) を参照する。

```powershell
cd C:\develop\KotorinToys
pwsh -File dev/scripts/start.ps1 -LocalCatalog
```

「すべて」「未導入」からScriptSyncを選択して起動する。起動中の本体がある場合はその画面が再利用されるため、一覧モードの切替時は先に本体を終了する。ローカル変更だけでは公開済み本体の一覧・実行物は更新されない。

ローカル開発一覧では実行ファイルがあるツールの状態を「停止中（開発）」「実行中（開発）」として表示する。導入Verは「未導入」を維持し、正式なインストールとは区別する。通常の配信一覧では従来の未導入表示を維持する。

単体開発は `npm run tauri dev`。一般の操作は [利用者ガイド](USERS_GUIDE.md)、キーは [キー定義](../dev/SHORTCUT_KEYS.md)、単体配布は [MSIX手順](MSIX.md) を参照する。

## 通信と保護

管理契約2のUTF-8・1行JSONを使用する。窓口は `\\.\pipe\KotorinToys.<現在のWindows利用者SID>.script-sync`。現在利用者だけのACLとローカル接続、同じ対話セッション、表示・停止の対象PIDを検証する。応答はPID・4部版・準備状態・起動経路・能力・結果と要求IDを返し、原稿や録音パスを含めない。`status` の文字列要求にも対応する。

要求は8,192バイト、要求IDは128バイト、通信と画面確認は接続後2秒まで。遅延した表示・停止は実行せず、停止応答を返せなかった場合は受理を撤回する。未対応操作は `unsupported`、誤PIDと作業中は `refused`。本体は停止受理とプロセス終了を別に確認する。

`--settings-dir <絶対パス>` では設定・ASR領域・画面状態・WebViewデータを専用領域へ分離できる。本体の実接続試験はこの指定を使い、通常の設定・個人原稿・録音を使用しない。通常起動の保存先は維持する。

## 配布と既存データ

今回は開発EXEの管理連携。`distributionStatus: unpublished` はKotorinToys対応インストーラーとして未公開という意味で、単体の署名付きMSIX公開状態とは別。既存MSIXの `jp.kotorinchunchun.scriptsync`、Publisher、Application Id、保存形式を維持する。汎用Inno製品には変換せず、本体からのインストール・更新・削除とMSIX専用導入管理は未対応。

自動起動予約は追加しない。`silent-start` は本体が非表示で起動できる能力であり、正式導入やOS予約の条件を省略するものではない。既存の `.scriptsync` と `.scriptsync.data/` は一組で保持し、今回の連携で移行しない。

実装と検証は [管理連携記録](../dev/history/v0.0.6/kotorintoys-management-imp.md) を参照する。
