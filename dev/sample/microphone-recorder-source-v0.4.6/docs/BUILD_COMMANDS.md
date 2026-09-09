# VoiceMixer ビルド・実行コマンド一覧

PowerShell 7（`pwsh`）で、リポジトリのルートから実行します。

```powershell
cd C:\develop\VoiceMixer
```

## 普段の実行

通常の開発でVoiceMixerを起動するときは、次のコマンドを使います。

```powershell
pwsh -NoProfile -File .\dev\scripts\run-dev-VoiceMixer.ps1
```

検証ダッシュボードを起動するときは、次のスクリプトを使います。Pluginのビルドも自動で行われます。

```powershell
pwsh -File .\dev\scripts\start-validation-dashboard.ps1
```

マイク入力デバイスの一覧だけを確認するときは、次のコマンドを使います。

```powershell
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder -- --list
```

## コマンドが長くなる理由

このプロジェクトでは、Cargoのマニフェストがルートではなく `src/src-tauri/Cargo.toml` にあります。また、Tauriはオプションfeatureとして定義され、複数の実行バイナリが登録されています。そのため、目的に応じて次の引数を付けます。

| 引数 | 意味 |
| :--- | :--- |
| `--manifest-path .\src\src-tauri\Cargo.toml` | ルートから対象のCargoプロジェクトを指定する |
| `--features tauri` | Tauriアプリに必要なTauri機能を有効にする |
| `--all-features` | `tauri`、`edge-websocket`など全featureを有効にする |
| `--bin <名前>` | ビルド・実行するバイナリを限定する |
| `--all-targets` | ライブラリ、バイナリ、テストなど全ターゲットを検査する |
| `--release` | 最適化されたリリースプロファイルでビルドする |
| `--` | Cargoの引数と、アプリ自身の引数を分ける |

`--manifest-path` を省略したい場合は、先に `cd .\src\src-tauri` を実行してください。その場合、以下のコマンド例から `--manifest-path .\src\src-tauri\Cargo.toml` を取り除けます。

## 普段のビルド

### Rustバックエンドの通常ビルド

```powershell
cargo build --manifest-path .\src\src-tauri\Cargo.toml
```

通常のデバッグビルドです。バックエンドの変更を確認したいときに使います。

生成先は `src/src-tauri\target\debug\` です。

### VoiceMixerアプリのデバッグビルド

```powershell
cargo build --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer
```

Tauriアプリ本体だけを起動確認したいときに使います。通常のアプリ開発では、このコマンドが最も明確です。

生成物は `src/src-tauri\target\debug\voicemixer.exe` です。

### VoiceMixerアプリのリリースビルド

```powershell
cargo build --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer --release
```

配布前の最適化ビルドや、デバッグ情報を減らした実行ファイルを確認したいときに使います。

生成物は `src/src-tauri\target\release\voicemixer.exe` です。

## 個別バイナリのビルド・実行

現在登録されている主なバイナリは次のとおりです。

| バイナリ | ビルド | 実行 | 用途 |
| :--- | :--- | :--- | :--- |
| `voicemixer` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer` | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer` | Tauriデスクトップアプリ |
| `microphone-recorder` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder` | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder -- --list` | WASAPIマイク列挙・録音 |
| `render-probe` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --bin render-probe` | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin render-probe -- --list` | Render排他の短いテスト信号 |
| `reconnect-probe` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --bin reconnect-probe` | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin reconnect-probe -- --list` | 切断・再接続の監視とJSON記録 |
| `rvc-passthrough-plugin` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --bin rvc-passthrough-plugin` | 個別起動ではなく検証ダッシュボードから使用 | Pass-through RVC Process Plugin |
| `validation-dashboard` | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --bin validation-dashboard` | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin validation-dashboard -- --port 8787` | 検証用HTTPダッシュボード |

### マイク録音の実行例

文章選択、再録音、確認再生、JSON台帳、無音短縮を使う場合は[対話録音](interactive-recording.md)を参照してください。

```powershell
# デバイス一覧を表示
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder -- --list

# 指定デバイスを10秒録音
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin microphone-recorder -- --device "MIC (BRIDGE CAST V2)" --output ".\src\src-tauri\target\microphone.wav" --duration-ms 10000
```

`cargo run` の `--` より後ろが `microphone-recorder` 自身の引数です。

### 切断・再接続プローブの実行例

```powershell
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin reconnect-probe -- --list
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin reconnect-probe -- --device "USB" --timeout-ms 120000
```

実行中に対象デバイスのケーブルを抜き差しします。標準出力のJSONに切断・再接続・復旧時間・`auto_reconnect_devices`の試行結果が入ります。

## 全体のコンパイル確認

### 全feature・全targetのビルド

```powershell
cargo build --manifest-path .\src\src-tauri\Cargo.toml --all-features --all-targets
```

全featureと全ターゲットをコンパイルできるか確認したいときに使います。通常のアプリ起動だけなら、`--features tauri --bin voicemixer` で十分です。

### 実行ファイルを作らない高速チェック

```powershell
cargo check --manifest-path .\src\src-tauri\Cargo.toml --all-features --all-targets
```

コードがコンパイル可能かを素早く確認したいときに使います。リンク済みの実行ファイルは生成しません。

### テストの実行

```powershell
cargo test --manifest-path .\src\src-tauri\Cargo.toml --all-features --all-targets
```

単体テスト・統合テストを実行したいときに使います。

### プロジェクト標準の一括確認

```powershell
pwsh -File .\dev\scripts\check.ps1
```

Format確認、通常featureのCheck、Tauri featureのCheck、テスト、Windows向けtargetのCheckをまとめて実行します。コミット前や構成変更後はこのコマンドを使用します。

## 検証ダッシュボード

### 手動で起動

```powershell
cargo run --manifest-path .\src\src-tauri\Cargo.toml --bin validation-dashboard -- --port 8787
```

`http://127.0.0.1:8787` で検証画面を開きます。ポート番号を変更する場合は `--port` の値を変更します。

### 起動スクリプトを使う

```powershell
pwsh -File .\dev\scripts\start-validation-dashboard.ps1
```

RVC Pass-through Pluginを先にビルドしてから、検証ダッシュボードを起動します。ポートを指定する場合は次のようにします。

```powershell
pwsh -File .\dev\scripts\start-validation-dashboard.ps1 -Port 8788
```

## 推奨する使い分け

| 状況 | 使用コマンド |
| :--- | :--- |
| Rustコードを軽く確認 | `cargo check --manifest-path .\src\src-tauri\Cargo.toml` |
| VoiceMixerをデバッグ起動 | `cargo run --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer` |
| VoiceMixerの実行ファイルを作成 | `cargo build --manifest-path .\src\src-tauri\Cargo.toml --features tauri --bin voicemixer` |
| 配布向けに最適化 | 上記に `--release` を追加 |
| 全体を検証 | `pwsh -File .\dev\scripts\check.ps1` |
| 検証画面を起動 | `pwsh -File .\dev\scripts\start-validation-dashboard.ps1` |
