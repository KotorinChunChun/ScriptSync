# 署名付きMSIXの作成・配布

v0.0.4以降の既定配布形式は、既存の共通証明書で署名したx64 MSIXです。アプリ版 `0.0.4` はMSIXでは `0.0.4.0` になります。NSISや単体EXEの生成だけではMSIXの作成・署名完了と扱いません。

[パッケージ作成スクリプト](../dev/scripts/package-ScriptSync-msix.ps1)は、Windows SDKのMakeAppxとSignToolを使用します。署名設定はPC共通の `%LOCALAPPDATA%\KotorinChunChun\MSIX\signing.json`、証明書は `CurrentUser\My`、Publisherは `CN=KotorinChunChun` です。秘密鍵の新規作成・輸出・信頼登録は行いません。

## 現行チェックアウトから作成する

関連試験に通った最新ソースから、リリースEXEをビルドしてからMSIXを作成します。以下はPowerShell 7.4以降で実行します。

```powershell
cd C:\develop\ScriptSync
npm run tauri build -- --no-bundle
pwsh -File dev/scripts/package-ScriptSync-msix.ps1 `
  -SourceDirectory C:\develop\ScriptSync `
  -ExecutablePath C:\develop\ScriptSync\src-tauri\target\release\scriptsync.exe `
  -OutputDirectory C:\develop\ScriptSync\release\msix-v0.0.4
```

スクリプトは `package.json`、`src-tauri/tauri.conf.json`、EXEのファイル版を照合します。版番号が同じでも古いEXEを使用できてしまうため、ソース変更後は再ビルドが必要です。ビルド元コミット・作業差分の有無・EXEとMSIXのSHA-256を [v0.0.4実装記録](../dev/history/v0.0.4/v0.0.4-imp.md) に記録します。

同名MSIXは上書きしません。同じ版を再検証するときは、以前の成果物を保持したまま `-OutputDirectory` に新しい出力先を指定します。別の専用チェックアウトで作る場合は、`-SourceDirectory` と `-ExecutablePath` の両方をそのチェックアウトと対応するビルド成果物へ変更します。

## 成果物と署名検証

出力先に次を生成します。

| ファイル・フォルダー | 内容 |
| --- | --- |
| `ScriptSync_0.0.4.0_x64.msix` | 署名付き配布パッケージ |
| `ScriptSync.cer` | 署名に使用した証明書の公開部分。秘密鍵を含まない |
| `signature-verification.txt` | SignToolの検証結果 |
| `layout-<ID>/` | AppxManifest.xml・EXE・画像を含むパッケージ作成用の配置 |

MakeAppxによるパッケージ作成後、SHA-256で署名し、SignTool・Authenticodeの署名者・CMS暗号署名・公開CERとの一致を検査します。署名に使える証明書が見つからない場合は失敗し、別の証明書へ自動切替しません。

自己署名証明書を信頼ストアへ登録していないPCでは、SignToolの信頼チェーン検証が未信頼ルートとして失敗することがあります。スクリプトはこの状態と署名の不整合を分け、CMS署名も独立検証します。**未信頼ルートのまま署名を確認できたことは、インストール先で信頼されることを意味しません。**

インストール先では公開証明書の発行者と拇印を確認し、組織の方針に従って信頼を用意します。信頼登録・証明書ストア変更・インストール・外部公開は、このパッケージ作成スクリプトには含まれません。現在の署名コマンドはタイムスタンプサーバーを指定していないため、証明書の有効期限を含めて配布時に検証します。

## 実行環境とv0.0.4の同梱範囲

Windows 10 2004以降のx64とWebView2 Runtimeを前提にします。パッケージはEXE・アイコン・MSIX manifestを含みます。WebView2 Runtime、Python実行環境、認識モデル、Codex CLI、外部音声合成CLIを一括同梱する構成ではありません。

v0.0.4のローカル文字起こしは、設定画面の「文字起こし環境を導入／再試行」から利用PCのユーザー領域へPython・認識ライブラリ・モデルを準備します。初回はインターネット接続が必要です。認識用PythonアダプターはEXEへ埋め込まれており、旧版のようにビルド元 `tools/asr/venv` を配布先で再現する必要はありません。MSIX化後の環境導入・認識動作は、通常のEXEでの試験とは別に確認します。

Whisper後の任意の文章改善には、Codex CLIの導入・認証も必要です。合成比較には外部TextToSpeechCLI環境を使用します。詳細は [開発者ガイド](DEVELOPERS_GUIDE.md) と現行版の実装記録を参照してください。

## 配布前に区別して確認する項目

- 最新EXEとソースの対応、およびMSIX内部の版・Publisher・署名者。
- 実際のMSIXインストール、起動、更新後の既存プロジェクト読み込み。
- 設定の保存先、マイク・再生デバイス、文字起こし環境の導入。
- `.scriptsync` ファイルからの起動と既存ファイルを開く動作。

Tauriの `bundle.fileAssociations` と手書きのMSIX manifestは別管理です。MSIXの関連付けにはmanifestの `windows.fileTypeAssociation` 拡張が必要で、Tauri設定だけでは自動反映されません。実際に生成したmanifestとインストール後の挙動を確認します。

## 署名準備で停止する場合

- 共通設定がない：既存の署名設定が使用できる実行ユーザーか確認します。自動で新規証明書を発行しません。
- 共通設定の証明書が `CurrentUser\My` にない：設定を変更する前に、実行ユーザーとサンドボックスによる証明書ストアの見え方を確認します。
- 有効期間外・残り7日以下・秘密鍵なし・Code Signing用途なし：スクリプトは署名を中止します。既存の証明書運用に従って対処します。
- Windows SDKがない：MakeAppxとSignToolの両方を含むx64 SDKが必要です。
- 既存MSIXがある：上書きせず別の出力先へ生成します。

この文書は作成手順です。成果物の完成・署名成功・インストール成功は、実行した結果を実装記録で確認してください。

## 公式資料

- [MicrosoftのMSIX手動作成手順](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-manual-conversion)
- [MSIX manifestのファイル関連付け](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap-filetypeassociation)

## 通常の配布コマンド

```powershell
cd C:\develop\ScriptSync
npm run release:msix
```

`release/msix-v0.0.5/<日時>/`へビルドから署名・検証まで実行する。MSIXに`.scriptsync`関連付けを含む。Windowsの既定アプリ選択は利用者の設定に従う。単独のEXEやNSISの古いインストールをMSIX更新済みと誤認しない。

## v0.0.4の生成結果

2026-09-12、標準コマンドで `release/msix-v0.0.4/20260912-195728/` に生成。MakeAppx・署名・CMS暗号署名検証・内包EXE一致を確認。SignToolの信頼チェーン検証は未信頼ルートのため未成立。インストール確認と信頼ストア登録は未実施。詳細は[実装記録](../dev/history/v0.0.4/v0.0.4-imp.md)。

## Web配布

MarkdownViewerと同じbeta配布先を使用する。[リリース・アップロード手順](../dev/RELEASE_RULE.md)を参照。

## MSIX再配布番号
アプリ版0.0.5とMSIX版0.0.5.1を区別する。正本はdev/msix-version.json。初回配布はrevision=0、リリース後に修正して再ビルド・再配布するたびに末尾を1増やしてコミットしてから生成する。今回1、次回2。アプリ版が変わった場合はapplicationVersionを合わせrevision=0へ戻す。失敗したビルドの再試行では増やさない。
package.json・Cargo・Tauriのアプリ版は維持する。MSIX manifest・ファイル名・App Installerは共通関数から4桁版を取得する。

MSIX 0.0.5.2ではWhisperの実参照パスを固定し、埋込Pythonの検索パスを再試行時にも修復する。モデル取得前のimport検査ログはsetup-library-check.log。
