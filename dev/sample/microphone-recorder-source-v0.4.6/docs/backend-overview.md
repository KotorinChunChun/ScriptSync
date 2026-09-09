# VoiceMixer バックエンド仕様・開発補足

配置・運用は [AGENTS](../../AGENTS.md)、最新の起動・検証コマンドは [コマンド一覧](../BUILD_COMMANDS.md) を正本とします。旧 README から利用者向け本文に収まらない内部仕様を移管した資料です。

Windows向けAudio Mixer / RVC RouterのRustバックエンド実装です。

本実装は、元の「Audio Mixer / RVC Router Rustバックエンド 機能・要件まとめ」を基礎にしつつ、2026-08-23までに確定した設計変更を反映しています。特にRVCは専用バックエンドではなく、将来追加される任意の音声変換処理と同じ **Transform Plugin** として扱います。

## 現在の主要仕様

- Audio Graph内部: 48,000 Hz / Float32 / Stereo
- 既定内部Block Size: 480 frames = 10 ms
- Block Sizeは設定ファイルから変更可能（64〜4096 frames）
- 入力: 物理マイク / Endpoint Loopback / PID / EXE追跡 / Edge PCM / 音声ファイル / Replay
- Transform: Multi Input / 汎用Transform Plugin / Built-in Limiter・Compressor・Side-chain Ducking
- 出力: Windows再生デバイス（物理・仮想）/ 録音出力 / イベント出力
- Recorder / File Input: WAV / FLAC / MP3
- Process Filter: EXE名 / EXE Path / PID / Windows System Soundsをモデル化
- VB-CABLE等のRender/CaptureペアはFrontendから明示設定
- 「このデバイスを聴く」はWindows Endpoint Property Storeからbest-effort検出
- Node単位の障害局所化、Peak/RMS、明示Graph Loopと暗黙Loop警告
- Audio Thread上でファイルI/O・ネットワーク待機・デバイス列挙を行わない構成

## Transform Plugin

新規のRVC実装は `NodeType::TransformPlugin` として接続してください。

外部プラグインは既定で設定ファイルの隣にある `plugins/` を探索します。検索パスは `settings.plugin_search_paths` で追加・変更できます。

詳細は次を参照してください。

- `docs/Transform Plugin 開発仕様書.md`
- `dev/examples/rvc-process-plugin-template/`

Transform Plugin API v1は **1 Audio Input / 1 Audio Output** です。ManifestにはPortを明示しており、将来のAPIバージョンで複数入出力へ拡張できる構造です。

## Limiter

LimiterはBuilt-in Transform Pluginです。

既定値:

- Ceiling: -1.0 dBFS
- Lookahead: 3 ms
- Release: 100 ms
- 自動挿入: しない

Limiterを使わなくてもMulti Inputのクリッピング検知は常時行います。

## 設定

サンプルは `dev/examples/config/default.json` にあります。

```json
{
  "settings": {
    "block_frames": 480,
    "meter_fps": 30,
    "auto_reconnect_devices": true,
    "plugin_search_paths": ["plugins"],
    "limiter_defaults": {
      "enabled": false,
      "ceiling_db": -1.0,
      "lookahead_ms": 3.0,
      "release_ms": 100.0
    },
    "device_preferences": {}
  }
}
```

相対Plugin検索パスは設定ファイルが存在するディレクトリを基準に解決します。

## Windows Process Filterに関する注意

Filter ONはEndpoint Loopback取得後のPCMから特定アプリを除去する処理ではありません。対象Processを選定し、Process Loopback群を内部Mixする `FilteredProcessComposite` へ切り替わります。そのためFilter ON時のCaptureは特定Render Endpointへ厳密には束縛されません。

Windows System SoundsはAudio Sessionとして識別可能なフィルター対象として扱いますが、現在の公開Process Loopback APIだけでSystem Sounds Session単独のPCMを完全に分離することはできません。実Captureの制約は `FilteredCaptureStatus.limitations` としてFrontendへ通知します。

またProcess LoopbackのProcess Tree単位というWindows API上の性質により、親Processを対象にした状態で特定の子Processだけを除外する、といったルールは完全には表現できない場合があります。FrontendはBackendが返すlimitationsを表示してください。

## RVCについて

`src/src-tauri/src/rvc/` および旧RVC Commandは旧設計との互換用Adapterとして残しています。**新規RVC実装はここへ直接組み込まず、Transform Plugin仕様に従うProcess Pluginとして実装してください。**

RVCプラグイン内部でPyTorch / ONNX / CUDA等のどれを利用するかはプラグイン側の責務です。Backendとの境界はPlugin Manifest、JSON Lines制御Protocol、共有メモリAudio Ringです。

`plugins/rvc-cuda/`には、NVIDIA CUDAとKahoA/KahoBを使用する実RVC Process Pluginがあります。Python/CUDA runtime、RVC推論コード、モデルは大容量のためGit管理外ですが、`pwsh -NoProfile -File .\dev/scripts\stage-rvc-cuda-plugin.ps1`でVoiceMixer配下へ同梱できます。詳細は[`plugins/rvc-cuda/README.md`](../../plugins/rvc-cuda/README.md)を参照してください。

## 検証ダッシュボード

設定値を変えながらバックエンド機能を再検証できるローカルページを用意しています。

```powershell
pwsh -File .\dev/scripts\start-validation-dashboard.ps1
```

起動後に `http://127.0.0.1:8787` を開いてください。Block Frames、入力Sample Rate、信号振幅、Limiter設定、E2E音声長、VB-CABLE Probe長などを変更し、「全機能を検証する」で再実行できます。

設定、Audio Core、Graph、Registered Node、動的Routing、Replay / Replay Cache、Recorder、Volume/Mute Profile、Transform Plugin、Edge PCM、WAV / FLAC / MP3、波形・Trim、Tauri Frontend契約を自動検証します。既定では約10秒のWAVを生成してPass-through RVC Process Pluginへ通し、VB-CABLEへ小振幅の既知信号をRenderして対応Captureで検出します。物理デバイスの聴感、切断・復旧、開発保留中のEdge拡張などは結果画面に「手動」として表示します。

起動スクリプトはRVC Pass-through Pluginを先にビルドします。実機信号を送らない場合は画面の `Windows System Probe` または `VB-CABLE Signal Probe` をOFFにしてください。

## UI非依存のマイク録音

画面を起動・操作せず、指定したWindows Capture Endpointを直接WAVへ録音できます。デバイス列挙と録音結果はJSONで標準出力へ返します。

```powershell
cargo run --manifest-path .\src/src-tauri\Cargo.toml --bin microphone-recorder -- --list
cargo run --manifest-path .\src/src-tauri\Cargo.toml --bin microphone-recorder -- --device "MIC (BRIDGE CAST V2)" --output ".\src/src-tauri\target\microphone.wav" --duration-ms 10000
```

録音APIは指定されたEndpoint IDだけをWASAPI共有モードで開き、別のマイクへ暗黙に切り替えません。出力は48 kHz / Float32 / StereoのWAVです。結果にはWASAPI worker状態、再接続回数、ドロップsample数、録音sample数、Peak、RMS、非無音sample数が含まれます。誤上書きを防ぐため、既存の出力ファイルは上書きしません。

## UI非依存の切断・再接続記録

指定したCapture / Render Endpointの切断と再接続を監視し、復旧時間と`auto_reconnect_devices`の試行結果をJSONで返します。人はケーブルを抜き差しするだけです。

```powershell
cargo run --manifest-path .\src/src-tauri\Cargo.toml --bin reconnect-probe -- --list
cargo run --manifest-path .\src/src-tauri\Cargo.toml --bin reconnect-probe -- --device "USB" --timeout-ms 120000
```

## Tauri 2アプリ

v16 FrontendをRust Backendへ接続したVoiceMixerは次で起動できます。

```powershell
cargo run --manifest-path .\src/src-tauri\Cargo.toml --features tauri --bin voicemixer
```

通常ブラウザでv16 HTMLを直接開いた場合はDemoデータへフォールバックし、Tauri実行時だけ永続BackendのCommand/Eventを使用します。

アプリ名の横にある「プロジェクト」コンボボックスから、現在のプロジェクト、Backend構成の再読込、サンプルプロジェクトを切り替えられます。「新規」で空のGraphを作成し、「別名保存」で自由なプロジェクト名を付けられます。通常のプロジェクト設定ファイルはアプリ設定フォルダの `routing-profiles\`（通常は `%APPDATA%\jp.voicemixer.desktop\routing-profiles\`）にJSONで保存されます。

「音声切替」のFull Profileは、Graph、配置、登録Node定義、Shortcut、OBS接続、Replay Cacheをまとめて保存・適用します。Gain / Volume / Muteだけを保存する「音量設定」は、素早い音量切替用として引き続き別管理です。旧schema v1のRouting ProfileはGraphだけを適用し、現在のShortcut等を保持します。

Node管理の「ショートカット」で、対象Nodeの操作をグローバルキーへ割り当てられます。一覧、作成、編集、削除、有効切替に加え、OS登録状態と発火結果を画面で確認できます。

「サンプル: 現在環境テスト構成」を選ぶと、現在のWindows環境からVB-CABLEのRender/Capture EndpointとRVC Pass-through Mockを検出し、次の停止状態Graphを一括生成できます。専用ボタンはありません。

組み込みプロジェクトは「サンプル: マイク入力モニター」「サンプル: ファイル再生・録音」「サンプル: 配信マイクチェーン」の3つです。前者は既定の入力デバイス、既定の通信入力デバイス、既定の再生デバイス、既定の通信再生デバイスを持ちます。`default://console` と `default://communications` は実行時にWindowsの既定Endpointへ解決されるため、別のPCへ設定ファイルを移しても実デバイスIDを固定しません。ファイル再生・録音は同梱の「こんにちは。10秒間のマイクテストです.wav」を既定再生デバイスへ出力し、同時にRecorderへ録音します。配信マイクチェーンは既定マイクをゲート→ノイズ抑制→EQ→コンプレッサー→Limiter→既定再生へつなぎます。

入力ノードの設定では、自動、WASAPI共有（イベント／ポーリング）、WASAPI排他（イベント／ポーリング）、WinMM WaveInを選択できます。設定した方式、実際に動作中の方式、接続エラーを確認でき、デバイス再検出と再接続も画面内から実行できます。入力デバイス一覧は名前で絞り込め、接続中の全EndpointのPeakを表示します。「モニター開始」を押している間だけ、選んだ入力をWindowsの既定再生デバイスで確認できます。

```text
10秒テスト音声 → RVC Pass-through Mock → Multi Input → VB-CABLE テスト出力
VB-CABLE テストCapture → テスト録音
```

ロードは現在のGraph配置を置換しますが、既存の登録済みノード定義は保持します。設定ファイル隣の `test-profile/` に48 kHz / Float32 / Stereo / 10秒のWAVと録音先を作成し、ロード直後は自動再生・自動録音を開始しません。プロファイル選択から再実行すると専用登録定義を更新・再利用するため、登録は重複しません。

## ビルド・検証

対象はWindowsです。PowerShellから次を実行してください。

```powershell
pwsh -File .\dev/scripts\check.ps1
```

このスクリプトはFrontendの型・契約・単体・UI検証とRustの書式・ビルド・回帰テストを実行します。最新の実測値と実機未確認範囲は現行版の実装報告書を参照してください。

## ドキュメント

- `docs/USERS_GUIDE.md`: 利用者向けガイド
- `docs/DEVELOPERS_GUIDE.md`: 開発環境、ビルド、検証手順
- `docs/BUILD_COMMANDS.md`: ビルド・実行コマンドの用途別一覧
- `docs/FAQ.md`: よくある質問とトラブルシューティング
- `docs/CHANGELOG.md`: 更新履歴
- `dev/history/design/Audio Mixer - RVC Router Rustバックエンド 機能・要件まとめ.md`: 元設計
- `dev/history/design/設計変更追補_v0.1.0.md`: 今回確定した変更点
- `docs/Transform Plugin 開発仕様書.md`: プラグイン開発者向け仕様
- `docs/Frontend向け_仮想オーディオデバイスペア連携.md`: Frontend側への連携仕様
- `dev/history/v0.4.2/v0.4.2-req.md` / `dev/history/v0.4.2/v0.4.2-imp.md` / `dev/history/v0.4.2/v0.4.2-task.md`: v0.4.2の要求・実装・進捗
- `dev/history/v0.4.3/v0.4.3-req.md` / `dev/history/v0.4.3/v0.4.3-imp.md`: v0.4.3の要求・実装計画と検証報告
- `dev/history/v0.2.1/v0.2.1-validation.md`: v0.2.1の検証状況（実機未確認は工程05へ引き継ぎ）

## 設定プロジェクトの独立実行

保存済みプロジェクトを画面なしで動かす `voicemixer-runner.exe` を追加しました。通知領域から有効・無効・終了を操作でき、複数同時起動に対応します。[起動・検証手順](../standalone-runner.md)を参照してください。

## 利用ケース別の起動・テスト

[ケース別の起動手順](../runner-scenarios.md)と[テスト計画・結果一覧](../../dev/testing/runner-results.md)を用意しました。ファイル・メモリ処理は自動確認でき、物理機器・外部接続はテスト待ちとして区別しています。
