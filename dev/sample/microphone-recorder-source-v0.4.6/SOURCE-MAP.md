# microphone-recorder ソースマップ

## 中心ソース

| ファイル | 役割 |
|---|---|
| `src/src-tauri/src/bin/microphone_recorder.rs` | CLI引数、単発録音、コンソール対話、確認再生 |
| `src/src-tauri/src/bin/microphone_recorder_ui/mod.rs` | localhost限定HTTP、録音状態、画面操作 |
| `src/src-tauri/src/bin/microphone_recorder_ui/index.html` | 依存ライブラリなしの収録画面 |
| `src/src-tauri/src/bin/microphone_recorder_ui/tests.rs` | Host・Token・台帳パス等の画面境界試験 |
| `src/src-tauri/src/recording/sample_session.rs` | 10課題、JSON台帳、品質判定、非破壊無音短縮 |
| `src/src-tauri/src/windows/microphone_recording.rs` | 録音要求、停止・進捗、WAV検査、結果JSON |

## 直接利用する支援モジュール

| 領域 | 主なファイル | 役割 |
|---|---|---|
| Windowsデバイス | `windows/devices.rs` | Capture/Render Endpoint列挙と既定Endpoint |
| 入力 | `windows/wasapi_capture.rs`、`windows/device_format.rs`、`windows/winmm_capture.rs` | WASAPI共有/排他とWinMM fallback、48 kHz stereo変換 |
| 確認再生 | `windows/wasapi_render.rs` | 既定Render EndpointへのWAV再生 |
| 状態 | `windows/worker_status.rs` | 起動待ち、稼働、再接続、停止状態 |
| 音声搬送 | `audio/buffer.rs`、`audio/format.rs`、`audio/resampler.rs`、`audio/clock.rs` | bounded ring、内部音声形式、時刻・変換 |
| WAV入出力 | `recording/writer.rs`、`recording/codecs.rs` | 録音workerと確認再生用decode |
| エラー | `error.rs` | 共通エラーと終了時の変換 |

## 実装上の境界

- ブラウザー画面は`127.0.0.1`の一時ポートのみで待ち受け、起動ごとのToken、Host、Originを検証します。
- UI起動では録音を開始しません。明示した`record`操作後だけWASAPIを開きます。
- マイク名の重複を避けるため、保存と再接続にはEndpoint IDを使用します。
- Captureは内部形式へ変換してからbounded ringへ書き、writerがFloat32 WAVへ保存します。過負荷時のdrop数は結果に残します。
- 台帳は一時ファイルを同期後にrenameし、録り直しはUUID付き別WAVにします。
- 無音短縮は文頭・文中・文末の対象区間を100 msへ縮め、発声側を削らず、別WAVと編集範囲を台帳へ追加します。

## 移植時に削れるもの

パッケージは既に録音専用の最小crateへ分離してあり、`commands`、`routing`、`plugins`、`shortcuts`、`runner`等を含みません。さらに機能を削る場合も、次の試験を残してください。

- CLI引数の排他・範囲検証
- 録音先の既存ファイル非上書き
- 台帳パスのディレクトリ脱出拒否
- localhostのHost・Token・Origin検証
- 原本を変えない無音短縮
- クリッピングと小音量警告
- マイク切断・WAV無音・dropの報告
