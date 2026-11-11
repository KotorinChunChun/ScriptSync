# ブラウザー版と簡易版

- 本体ブラウザー版：https://excel-chunchun.sakura.ne.jp/test/scriptsync/
- 従来の簡易版ScriptSyncWeb：https://excel-chunchun.sakura.ne.jp/test/scriptsyncweb/

本体Webはデスクトップの5工程・差分承認・アーカイブ・テイク編集/結合・読み補正・音声比較/採点を共有し、保存と音声処理だけをブラウザーへ置き換えます。Whisperは専用WorkerのCPU/WASMで実行し、初回に取得したtiny/base/smallモデルをこのアプリ専用キャッシュへ保存します。原稿・音声をWhisperサーバーへ送信しません。認識やAIの結果は候補であり、自動で確定台本へ反映しません。

作業データの正本はIndexedDBにあり、本文と音声を一つのトランザクションで保存します。保存に失敗した場合は以前の正常な状態へ戻ります。全データをZIPに退避して復元でき、従来の交換ZIPとAES暗号化交換ZIPも取り込めます。バックアップには編集前の音声も含まれます。ブラウザー内では簡易版と別の領域に保存します。ドメインの移設時に端末内の保存データは引き継がれないため、旧OriginでZIPに退避してから新URLで取り込みます。

Web版の完成音声はWAV、字幕はVTT、時間情報はblock-map.jsonです。デスクトップのFLAC、Talk専用ファイル入出力、OSフォルダー表示、非アクティブ時グローバルキー、PowerPoint直接操作はブラウザー単体の対応外です。出力先を開く操作はZIPの再ダウンロードになります。録音はマイク許可が必要で、ページを開いたまま行います。長時間収録・実機音質・機器ごとの相性は実機受け入れで確認します。

AIと音声合成は[サービス接続手順](service-connections.md)で設定します。基本の録音・編集・WAV出力は外部サービスなしで完結します。公開サイト自体へ共用のAPIキーや利用者の原稿・録音を配置しません。

## ビルド・自動試験

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm ci --ignore-scripts
npm test
npm run test:services
npm run build:web
npm run test:web:e2e
# 実Whisperモデルの取得/推論も確認する場合
npm run test:web:e2e -- --asr
```

PlaywrightとヘッドレスのEdgeで合成マイクを使います。Computer Useやデスクトップ操作は不要です。試験データは対象版のlogsに保存し、IndexedDBは隔離したブラウザーコンテキストで使います。`--video` で記録するのはページだけで、実デスクトップとマイクは収録対象外です。必要な場合は `npm exec playwright install ffmpeg` で試験専用エンコーダーを取得します。

Webビルドはdist-webへ出力し、ブラウザーへ配布する24件のnpmライセンスを同梱します。デスクトップのビルドではブラウザー専用Worker/WASMを除外します。配布前は`npm run licenses:check`と`npm run check:security -- --history`を実行します。公開処理は`dev/scripts/publish-web.py`で接続しないDryRun、`--stage`でWebsite配置、`--execute`で限定FTPS送信とHTTPS読戻しを行います。製品コード・対象コミット・検証が確定してから実行し、既存対象は先に版別logsへ退避します。
