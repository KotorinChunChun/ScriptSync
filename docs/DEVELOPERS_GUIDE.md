# 開発者ガイド

## 構成

- `src/`: Tauri WebViewで動作するTypeScript UI
- `src-tauri/src/`: Rustバックエンド、ドメイン、保存、外部プロセス連携
- `tests/fixtures/`: 契約試験・回帰試験用の非機密fixture
- `dev/`: 要求、実装計画、進捗、運用資料

## 起動

```powershell
npm install
npm run tauri dev
```

## 検証

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm test
```

## 録音基盤と認識

`crates/scriptsync-recorder` は受領したv0.4.6のコピー。由来と修正点は同crateのORIGIN.mdを参照。アプリの状態と保存はRust、表示はTypeScript、認識のみ独立Pythonワーカーです。

ローカルASRの準備（Python 3.12を推奨）：

```powershell
pwsh -File dev/scripts/setup-asr.ps1 -Python 'Python実行ファイルのパス'
```

初回は依存とsmallモデルを取得します。通常の認識はキャッシュのみを使用し、原稿や期待読みを渡しません。候補は要確認として保存し、正解読みを自動で変更しません。実装方法の出典：[faster-whisper公式](https://github.com/SYSTRAN/faster-whisper)。漢字→かな変換にはpykakasiを使用するため、かな候補は発話の直接証拠ではありません。

## 合成fixtureによる実機試験

```powershell
pwsh -File dev/scripts/create-synthetic-fixtures.ps1
$env:SCRIPT_SYNC_FIXTURES = '上記が出力したフォルダの絶対パス'
cargo test --manifest-path src-tauri/Cargo.toml --lib synthetic_end_to_end -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib synthetic_asr_measurement -- --ignored --nocapture
cargo test --manifest-path crates/scriptsync-recorder/Cargo.toml --lib --locked --target-dir src-tauri/target
```

合成fixtureは人間の正解音声ではありません。既定でignoredの2試験を明示実行し、生成・認識・保存・結合を実サービスで確認します。通常試験はマイクやTalkを使用しません。

`npm test` はDOM上で入力のエスケープ、匿名表示、採点null、Unicode選択、マイクIDの呼出しを確認します。ネイティブ画面操作や実マイク音質の代替ではありません。開発時のみ `?preview=output/tests/<検証フォルダ>` で保存済み検証プロジェクトの閲覧プレビューが可能です。書込操作は動作しません。

## 配置上の前提

現在の音声CLI既定パスは `C:/develop/amekoro`。ASRワーカーはビルド時の本プロジェクトのtools/asrを参照します。別PCへの単体配布パッケージではなく、今回の開発PCで動かすv0.0.1です。別環境への移動時は接続パス・ASR配置を調整し、認証ファイルを配布しないでください。

`revisions` の不変JSONを保存の正本とし、`project.json` は閲覧用索引です。最新世代が破損した場合は自動で過去へ戻さずエラーを表示し、履歴を保全します。WAV確定と台帳保存の間で中断した場合もファイルを残します。
