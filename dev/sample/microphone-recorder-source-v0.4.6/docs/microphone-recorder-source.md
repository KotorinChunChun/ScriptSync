# microphone-recorder ソース見本一式

VoiceMixerの対話録音ツールを、別のRustプロジェクトで参照・移植するためのソースパッケージです。実行ファイル、DLL、ビルド済み成果物、録音済み音声、個人の`session.json`は含みません。

## パッケージの内容

```text
microphone-recorder-source-v0.4.6/
  README.md
  SOURCE-MAP.md
  SHA256SUMS.txt
  THIRD-PARTY-NOTICES.txt
  docs/
    BUILD_COMMANDS.md
    backend-overview.md
    interactive-recording.md
    microphone-recorder-source.md
    session.schema.json
    session.template.json
  src/src-tauri/
    Cargo.toml
    Cargo.lock
    src/
```

`src/src-tauri/src/`には録音ツールが直接利用するモジュールだけを収集しています。VoiceMixerのGraph、ルーティング、プラグイン、Tauri画面、音声fixtureは含みません。見本専用の小さい`lib.rs`と`Cargo.toml`で依存境界を固定し、単独でコンパイルできます。最初に`SOURCE-MAP.md`を確認してください。

## コピー後の確認

Rust 1.85以上のWindows x64環境を使用します。`Cargo.lock`は同梱しますが依存crate本体はベンダーしていないため、コピー先で未取得の依存を初回ビルドするときはcrates.ioへ接続できる必要があります。パッケージのルートから次を実行します。

```powershell
cargo check --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder --locked
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder -- --interactive --directory .\private\audio-samples
```

起動しただけでは録音しません。ブラウザーでマイクと課題を選び、録音ボタンを押すと開始します。個人音声と台帳を保存する`private/audio-samples/`などは、コピー先プロジェクトのGit除外対象にしてください。

## 別プロジェクトへ組み込むときの推奨境界

まずは`microphone-recorder`を別プロセスのCLIとしてソースビルドし、コピー先アプリから起動する方法を推奨します。戻り値は終了コードとJSONを使います。ブラウザー画面内部のlocalhost APIは外部プログラム向けの公開APIではありません。

ライブラリへ統合する場合は、`SOURCE-MAP.md`に示した順で次の境界を分離してください。

1. `sample_session.rs`の台帳・品質判定・無音短縮
2. `microphone_recording.rs`の録音要求・進捗・WAV検査
3. `microphone_recorder_ui/`のlocalhost画面
4. Windowsデバイス列挙・WASAPI Capture/Render

現在のWAV仕様は48 kHz、ステレオ、32-bit Floatです。録り直し、無音短縮とも既存ファイルを上書きしません。

## 関連ドキュメント

- `README.md`: コピー、実行、移植の入口
- `SOURCE-MAP.md`: 中心ソースと内部依存
- `docs/interactive-recording.md`: 操作と保存仕様
- `docs/backend-overview.md`: VoiceMixerバックエンド内での位置づけ
- `docs/BUILD_COMMANDS.md`: 元プロジェクトのビルド・実行方法
- `docs/session.schema.json`: 台帳JSON Schema
- `docs/session.template.json`: 編集可能な課題見本
- `THIRD-PARTY-NOTICES.txt`: Cargo依存の版・ライセンス宣言・入手元

## ソースパッケージの再生成

VoiceMixerのルートで実行します。コミット済みソースを取得するため、録音中の個人ファイルや未コミット変更は混入しません。

```powershell
pwsh -NoProfile -File .\dev\scripts\export-microphone-recorder-source.ps1
```

`release/microphone-recorder-source-v<version>/`と同名ZIPを生成します。既存成果物は上書きしません。生成時にパッケージ内ソースだけを使う`cargo check --bin microphone-recorder --locked`、禁止成果物の不在、SHA-256を確認します。

VoiceMixer自体のリポジトリには現時点でライセンスファイルがありません。このソースパッケージは再配布ライセンスを新たに付与するものではないため、組織外への配布前に所有者がライセンス方針を確定してください。
