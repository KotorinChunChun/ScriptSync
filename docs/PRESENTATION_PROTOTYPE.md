# プレゼン連携試作

既存ScriptSyncの音声出力から、スライド送り・OBSシーン切り替え・PPTX生成を試す独立ツールです。Node.js 22以降（組込みWebSocket対応）、Windows連携にはPowerShell 7を使用します。製品本体の保存形式や配布版は変更しません。

## 起動してデモを試す

```powershell
cd C:\develop\ScriptSync
npm run presentation:dev
```

npmがPATHにない場合は `node tools/presentation/server.mjs` でも起動できます。[試作画面](http://127.0.0.1:4317)をブラウザーで開きます。ポートは環境変数 `SCRIPTSYNC_PRESENTATION_PORT` で変更できます。終了は起動した端末のCtrl+Cです。

1. 最初は3ブロックのデモです。各4秒で冒頭に短い確認音が鳴ります。人声ではありません。
2. 「再生」「一時停止」「次へ」「前へ」で進行します。音量は画面で変更できます。
3. 自動進行を試す場合は、一時停止し「進行」を「自動」にして「設定を反映」、再生します。音声終了時に次のブロックへ進み、最後で停止します。
4. スライダーで選択音声内の途中へ移動できます。先頭で前へ・末尾で次へは無効です。

音声再生はこの画面1枚が担当します。画面を閉じたり通信が3秒以上途切れると進行は停止します。APIだけでは音声を再生できません。再生機器はブラウザーの設定に従い、ScriptSync本体のデバイス設定とは別です。

## 実際の音声を使う

ScriptSyncで結合・出力を行い、出力フォルダーの `block-map.json` の絶対パスを指定します。WAV/FLACとJSONを同じフォルダーに置きます。取込時に音声のSHA-256を照合し、失敗した出力を採用しません。未採用発話は現行製品の出力ルールに従い、マップに含まれる音声だけを利用します。

見出し、箇条書き（6行以内、各140文字以内）、PowerPointスライドID、OBSシーンを編集して「設定を反映」します。元の台本・録音には反映しません。「対応表を保存」でJSONをダウンロードし、次回は同じ音声出力を読み込んでから「対応表を取り込む」で復元します。取込時は外部連携をオフにします。

再出力ではexport_idが変わるため旧対応表の自動流用はできません。誤った音声とスライドの組合せを避ける試作上の制約です。設定はサーバー終了で失われるため、必要な対応表を保存してください。

## PowerPoint

Windowsデスクトップ版PowerPointが必要です。PPTXの絶対パスを入力し「開いてスライド一覧を取得」を押します。各ブロックで取得したスライドを選択し、「スライド送りを有効にする」をオンにして設定を反映します。再生・ブロック移動で対象スライドへ移ります。複数ブロックに同じスライドを割り当てられます。

読み取り専用で開き、元PPTXを保存・変更しません。既に開いている同じ資料は再利用します。指定にはスライド番号でなくSlideIDを使います。スライド削除・別資料への差替え時は対応を見直してください。試作はWindows COM経由で操作するため、PowerPointのダイアログが開いている場合は先に閉じてください。

## OBS

同じPCのOBS StudioでWebSocket v5サーバーを有効にし、URL（既定 `ws://127.0.0.1:4455`）とパスワードを入力します。「接続してシーン一覧を取得」で接続し、各ブロックのシーンを選択、「シーン切り替えを有効にする」をオンにして設定を反映します。OBSのプログラム側シーンを変更します。

パスワードは設定ファイル・対応表に保存せず、接続後に入力欄を消去します。録画・配信の開始停止は行いません。外部操作の失敗で進行を止めますが、その直前に成功したPowerPoint/OBS操作は元に戻しません。両アプリを同一瞬間に切り替える保証はありません。

## スライド生成

資料タイトルを設定・反映し「PPTXを生成」を押すと、表紙＋各ブロックの編集可能なスライドをダウンロードします。読み上げ全文は発表者ノートに入り、音声は埋め込みません。長い文章は自動要約しません。表示する箇条書きを短く編集し、生成後のレイアウトをPowerPointで確認してください。

表紙が1枚加わるため、対応するスライドは原則2枚目以降です。生成したPPTXを保存後、PowerPoint連携の一覧から対応付けてください。プレビューは選択ブロックの内容を表示する簡易表示で、PowerPointの実画面を取り込んだものではありません。

## 外部API

ローカルPCから利用できます。画面下部「操作ログ・外部API」にあるセッショントークンを `X-ScriptSync-Token` ヘッダーへ設定します。サーバー起動ごとに変わります。認証情報をソースや資料に書き込まないでください。

| メソッド・パス | 入出力 |
|---|---|
| GET `/api/state` | map、plan、index、time、status、revision、epoch、ログを取得 |
| POST `/api/load` | `{"path":"C:\\…\\block-map.json"}`：音声出力を取込 |
| POST `/api/plan` | `{"plan":対応表}`：全対応表を検証して反映。一時停止中のみ |
| POST `/api/transport` | `{"action":"play"}`：play / pause / stop / next / previous |
| POST `/api/transport` | `{"action":"seek","index":1,"time":0.5}`：0始まりのブロックと秒 |
| GET `/api/deck` | PPTXバイナリ。planのtitle・cuesのtitle/bulletsとmapの全文から生成 |
| POST `/api/powerpoint/open` | `{"path":"C:\\…\\資料.pptx"}`：スライドID・番号・タイトルを取得 |
| POST `/api/obs/connect` | `{"url":"ws://127.0.0.1:4455","password":"…"}`：OBSへ接続・シーン取得 |
| POST `/api/obs/disconnect` | `{}`：OBSを切断 |

生成ツールはstateから台本・発話IDを取得し、plan.cuesのtitle/bullets/slide_id/sceneを編集して返せます。project_id・export_id・segment_idsは保持してください。対応表スキーマは `scriptsync.presentation-plan` / version 1。進行方式は `manual` / `auto`、連携の有効状態は `powerpoint_enabled` / `obs_enabled`。

`/api/progress` と transportの `ended` / `failure` は再生画面専用です。外部ツールは上表の制御APIを使います。ブラウザーは約0.5秒間隔で操作を受け取り、音声終了イベントで自動進行します。精密な映像同期・スライド内アニメーション制御は対象外です。

## 検証

```powershell
node --test tools/presentation/presentation.test.mjs
```

記録は[v0.0.7実装報告](../dev/history/v0.0.7/v0.0.7-imp.md)。公式仕様：[PowerPoint GotoSlide](https://learn.microsoft.com/en-us/office/vba/api/powerpoint.slideshowview.gotoslide)、[PowerPoint Open](https://learn.microsoft.com/en-us/office/vba/api/powerpoint.presentations.open)、[OBS WebSocket v5](https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md)。
