# AI・音声合成サーバーへの接続

Web版とデスクトップ版は利用者側の接続ゲートウェイを使います。既定は `http://localhost:4318`。画面の設定でサーバー名/IP・ポートを指定できます。localhostは画面を開いている端末自身です。スマートフォンからPCのゲートウェイを使う場合はPCのLAN IPを指定します。

## ゲートウェイの起動

Node.js 22.12以上。公開サイトにAPIキーを置きません。秘密は `C:\AI\env\ScriptSync\` または環境変数で管理します。ソース・プロジェクト・ログへ値を含めません。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
# 以下の変数は、自分の秘密管理からプロセスへ設定する（画面へ表示しない）。
# SCRIPTSYNC_GATEWAY_TOKEN : 24文字以上の推測困難な接続トークン
# CODEX_APP_SERVER_URL    : 既定 ws://localhost:4500
# CODEX_APP_SERVER_TOKEN  : app-serverで認証を有効にした場合のトークン
# OPENAI_BASE_URL         : 既定 https://api.openai.com/v1
# OPENAI_API_KEY          : OpenAIまたは互換APIのキー
# SCRIPTSYNC_TTS_URL      : 既定 http://localhost:4319
# SCRIPTSYNC_TTS_TOKEN    : 音声合成サーバーの認証を使う場合
npm run services:start
```

Codex app-serverは利用者が別に起動します。`codex app-server --listen ws://127.0.0.1:4500` が接続例です。WebSocketは公式にも実験的な方式です。LANへ直接公開する場合は必ず認証を設定してください。本アプリは既存のCodexプロセスを停止・再設定しません。

ゲートウェイは既定127.0.0.1だけで待受します。LAN利用はSCRIPTSYNC_GATEWAY_HOST、必要なTLS終端・証明書・限定ファイアウォールを別途設定します。公開HTTPSページからの接続にはブラウザーのローカル接続許可/CORS/TLS制約があるため、ブラウザーの保護を無効化せず接続テストで確認します。SCRIPTSYNC_ALLOWED_ORIGINSはカンマ区切りの完全なOriginで、既定は本サイトとローカル開発1420だけです。

Web設定の接続トークンはメモリだけに保持します。再読込後は再入力します。デスクトップはSCRIPTSYNC_GATEWAY_TOKEN、またはC:/AI/env/ScriptSync/gateway-token.txtから読みます。どちらもプロジェクトZIPへ書きません。

## 音声合成HTTP契約

今後作る音声合成サーバーは、既存CLIとは独立して次を提供します。

- `GET /v1/voices`：JSONの音声一覧。
- `POST /v1/audio/speech`：JSON `{model,voice,input,response_format:"wav",speed}` を受け、RIFF/WAVE音声を返す。
- 認証は必要な場合Bearer。エラーは非2xx。タイムアウト300秒。実話者とモデル版の一致は別途確認し、名前を指定しただけで同一条件を保証しません。

## 自動検証

`npm run test:services` は模擬HTTP/WebSocketで認証、Origin拒否、Codexの初期化と正常/失敗完了、API本文を確認します。実サービスの課金・ログイン・音質は合成サーバー試験とは別です。
