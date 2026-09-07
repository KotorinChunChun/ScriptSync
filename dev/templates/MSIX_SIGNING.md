# KotorinChunChun 共通MSIX署名

このPCの現在のWindowsユーザーで開発するアプリは、Store外のMSIX署名に
KotorinChunChun共通証明書を使う。この証明書は特定のアプリ専用ではない。
秘密鍵を各リポジトリへ複製せず、Windowsの`Cert:\CurrentUser\My`を参照する。
別Windowsユーザーには秘密鍵を自動共有しない。Store提出用のIdentityとMicrosoftによる署名は別経路のまま維持する。

## 共通設定と現在の証明書

- 設定: `%LOCALAPPDATA%\KotorinChunChun\MSIX\signing.json`
- Publisher: `CN=KotorinChunChun`
- 拇印: `475BABC3C3D1E871D90503D45CC5236216D85EE8`
- 有効期限: 2031-08-10 14:00:09（日本時間）
- 鍵: RSA 3072 bit、WindowsのMicrosoft Software Key Storage Providerで保持
- 証明書の表示名（Friendly Name）は識別に使用しない。Publisherと拇印で固定する。

既存の共通証明書を再利用する。鍵・証明書の再発行、秘密鍵の輸出、信頼ストアの変更は不要。
JSONにはschemaVersion、storeLocation、storeName、thumbprint、publisherだけを保存する。秘密鍵やパスワードは含めない。

```json
{
  "schemaVersion": 1,
  "storeLocation": "CurrentUser",
  "storeName": "My",
  "thumbprint": "475BABC3C3D1E871D90503D45CC5236216D85EE8",
  "publisher": "CN=KotorinChunChun"
}
```

## 新しいアプリへの導入と署名

1. 共通テンプレート`C:\develop\default\dev\templates\MsixSigning.psm1`をプロジェクトの署名スクリプトと同じディレクトリへコピーする。版管理するコードは各repoに置き、署名設定は共通JSONだけを参照する。
2. Store外MSIXのManifest Publisherを`CN=KotorinChunChun`にする。Identity Nameはアプリごとに固有に保つ。
3. 既存のガード付きリリース処理で、ビルド前に下記の関数を呼び、署名時はその拇印をSignToolへ渡す。Friendly Name検索、`/a`による自動選択、個別PFX、証明書の自動発行へフォールバックしない。
4. 配布CERとMSIX署名者の拇印一致を検証する。公開してよいのはCERだけ。新アプリ用に秘密鍵を作らない。

```powershell
Import-Module (Join-Path $PSScriptRoot 'MsixSigning.psm1') -Force
$certificate = Get-MsixSigningCertificate -Publisher $manifestPublisher
try {
    # 既存の明示承認済みリリース処理内で使用する。
    & $signTool sign /fd SHA256 /s My /sha1 $certificate.Thumbprint $msixPath
    if ($LASTEXITCODE -ne 0) { throw 'MSIX署名に失敗しました。' }
} finally { $certificate.Dispose() }
```

## AIへMSIX作成を依頼するとき

移植後のPCでAIへMSIX作成を依頼するときは、次の条件を伝える。

```text
このPCでは、Store外MSIXの署名にKotorinChunChun共通署名証明書を使用してください。
PublisherはCN=KotorinChunChun、証明書はCurrentUser\Myの秘密鍵付き証明書を使い、
%LOCALAPPDATA%\KotorinChunChun\MSIX\signing.jsonの拇印を参照してください。
証明書の自動発行、Friendly Nameによる自動選択、個別PFXの使用やコピーは行わず、
アプリ既存の署名用ビルドスクリプトを使用してください。TrustedPeopleへの登録、インストール、
公開は別途明示した場合だけ行ってください。
```

AIには、今回の範囲が「ローカルで署名済みMSIXを生成するだけ」なのか、検証端末へのインストールや公開まで含むのかも明示する。

共通JSONの初期登録は、同梱の`initialize-msix-signing.ps1`に既存の拇印とPublisherを渡す。
同じ設定への再実行は無変更。別証明書の設定があれば上書きを拒否する。`-WhatIf`で保存前に確認できる。
共通モジュールを更新した場合は、利用する各アプリと共通テンプレートへ同じ版を反映して検証する。

## エラーと更新互換性

- 設定欠落、未知の項目、別ストア、Publisher／拇印不一致、秘密鍵欠落、Code Signing用途なし、デジタル署名不可、有効期間外では停止する。
- 期限まで90日以下は警告、7日以下は停止する。自動更新・自動発行は行わない。
- 既存アプリは、更新互換性を確認したうえでManifest Publisherを`CN=KotorinChunChun`へ統一する。Publisherを変更するとPackage Familyや更新経路が変わる場合がある。
- Store版はPartner CenterのIdentityを使い、共通自己署名証明書へ置換しない。

## キーの保管とバックアップ

秘密鍵の実体はWindowsのユーザー鍵ストレージが管理する。証明書ストアの秘密鍵を削除したり、ユーザープロファイルを初期化したりしない。
現行キーはエクスポート可能な設定で作られている。今回その属性を変更せず、PFXバックアップも自動作成しない。
バックアップが既に存在することは確認していない。

PC故障・交換に備える場合は、明示的なバックアップ作業として、強いパスワードで暗号化したPFXを
アクセス制限されたオフライン媒体へ保存し、パスワードは別の安全な保管先で管理する。
パスワードをコマンドライン、ログ、JSONへ書かず、`Read-Host -AsSecureString`等の対話入力を使う。
リポジトリ・Web公開領域・平文PFX・秘密鍵と同じ場所のパスワード保存は避ける。復元確認は別の検証環境で行う。
共通キーの漏えいは、このキーで署名する全アプリに影響する。紛失・漏えい・更新では全アプリの移行計画が必要。

## 移植用バンドルの作成と展開

移植元PCで次のスクリプトを実行すると、秘密鍵付きPFX、移植先スクリプト、共通モジュール、初期設定スクリプト、公開メタデータを空の指定フォルダーへまとめる。
出力先はリポジトリ外のアクセス制限された場所を指定する。既存ファイルの上書きは行わない。

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File C:\develop\default\dev\templates\export-msix-signing-bundle.ps1
```

出力先を質問されたら、暗号化USB等の安全なフォルダーの絶対パスを入力する。PFX用パスワードは対話入力し、移植先へはPFXと別経路で伝える。
`-WhatIf`を付けると、PFX作成やファイル出力をせずに確認できる。

バンドルフォルダーを移植先PCへ安全に移した後、フォルダー内の`import-msix-signing-bundle.ps1`を実行する。

```powershell
   pwsh -NoProfile -ExecutionPolicy Bypass -File .\import-msix-signing-bundle.ps1
```

PFXのパスワードと証明書登録の確認（`YES`）を入力すると、現在のWindowsユーザーの`CurrentUser\My`へ登録し、拇印・Publisher・用途・有効期限を検証してから`signing.json`を作成する。
同じ証明書が既に登録済みなら再インポートせず、別の共通設定が存在する場合は上書きしない。
同時に、秘密情報を含まない共通テンプレートを既定の`C:\develop\default\dev\templates`へ自動同期する。
異なる既存ファイルは上書きせず停止する。別の配置先を使う場合は`-TemplateDirectory`で指定する。
このスクリプトは`TrustedPeople`等の信頼ストア、インストール済みアプリ、リポジトリ、Web公開領域を変更しない。

移植後にバンドルのPFXを削除する場合は、復元確認と別保管の完了後に、利用者が安全な手段で行う。PFXとパスワードを同じ場所へ残さない。

## 根拠

- [Microsoft: SignToolによる署名とPublisher一致条件](https://learn.microsoft.com/en-us/windows/win32/appxpkg/how-to-sign-a-package-using-signtool)
- [Microsoft: Package IdentityとPackage Family](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/package-identity-overview)
