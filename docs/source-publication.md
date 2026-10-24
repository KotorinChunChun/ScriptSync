# ソース公開前の確認

GitHubの公開範囲を変更する操作と、ローカルで公開準備を整える操作は分けます。公開設定はユーザーの明示指示を受けて変更してください。

## 公開対象と個人データ

製品コード・合成fixture・文書だけをGitへ追加します。実原稿、録音、端末設定、APIキー、署名秘密鍵、実行ログ、動画、node_modulesを含めません。`.gitignore` は誤追加を減らすための補助で、すでに追跡したファイルや `git add -f` は防げません。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci --ignore-scripts
npm run test:security
npm run check:security -- --history
npm audit --audit-level=low
git status --short
git diff --cached --check
```

`check:security` は現行の追跡ファイルを検査します。`--history` はローカルの全参照から到達可能なGit履歴と、削除済みの機密ファイル名も検査します。公開する全ブランチ・タグを取得して実行し、ステージ後にも確認してください。未追跡・ignore済みのデータ、GitHubのIssue/添付/Actionsログ、未知の秘密形式をすべて検査するものではありません。指摘の値は出さず、位置と分類だけを表示します。

既知トークン・秘密鍵はfixtureでも検出します。一般のパスワード文字列だけは、合成試験と明確なplaceholderを区別します。検査を通すために秘密値をplaceholderとして扱わないでください。漏洩が見つかった場合は、値を失効・再発行し、公開前の履歴処理を別途判断します。

GitHub Actionsは読取権限のみで同じ検査・依存監査・UI試験・buildを実行します。Gitの認証情報はcheckout後に保持しません。実機音声やMSIXの動作確認をCIの成功と取り違えないでください。

## CSP

配布版はselfのscript、TauriのIPC、専用音声protocolを許可します。任意の外部script、eval、iframe、object、form送信を禁止します。動的な画面配置に必要なstyle属性は許可します。開発時だけ、固定ポート1420のloopback通信とHMR WebSocketを追加します。Rust側の外部通信はWebViewのCSPとは別に制御されます。

CSPを変えた場合は、画面内包のネイティブビルドで起動・IPC・音声試聴を確認してください。ブラウザーのDOMモック試験だけでは、WebView2上のCSPや音声protocolの許可を確認したことになりません。
