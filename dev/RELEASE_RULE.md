# ScriptSync リリース・アップロード手順

MdVisualizerのdev/RELEASE_RULE.mdとpackaging/release-config.jsonを参考に、同じWebsiteのScriptSync専用配布先を使用する。ScriptSyncの署名済みMSIXを配布し、SPFxは対象外。

- Website: C:\develop\excel-chunchun
- ローカル配置: www/download/ScriptSync/
- 公開先: https://excel-chunchun.sakura.ne.jp/download/ScriptSync/
- 対象: ScriptSync_<MSIX版>_x64.msix、ScriptSync.cer、ScriptSync.appinstaller
- 署名・生成: [MSIX手順](../docs/MSIX.md)。共通証明書を使用。
- 認証: Website既存の証明書検証付きFTPS接続をpublish-ScriptSync-ftps.pyから使用。秘密情報は複製・表示せず、平文FTPへ切り替えない。
- stable、他製品、共通配布トップページは自動更新しない。

## 操作

PowerShell 7.4以降。まずnpm run release:msixで成果物を生成する。次の例の成果物ディレクトリは対象ビルドへ置き換える。

```powershell
cd C:\develop\KotorinToys\tools\ScriptSync
npm run release:plan
$artifact = 'release/msix-v0.0.5/20260912-221010'
pwsh -File dev/scripts/publish-ScriptSync.ps1 -Mode Prepare -ArtifactDirectory $artifact
pwsh -File dev/scripts/publish-ScriptSync.ps1 -Mode Stage -ArtifactDirectory $artifact
```

Prepareはローカルの署名・CER・Identity・版を検査してApp Installerと公開ページ・アイコンを成果物へまとめる。StageはWebsiteのScriptSync専用5ファイルだけを更新する。同版の異なるMSIXは上書きしない。Websiteの既存変更を保持し、対象5ファイルだけを確認・コミットする。

実公開を依頼された場合に、以下を実行する。PythonがPATHにない場合は-PythonPathに実行ファイルを指定する。

```powershell
pwsh -File dev/scripts/publish-ScriptSync.ps1 -Mode Publish -ArtifactDirectory $artifact -PublishConfirmed
pwsh -File dev/scripts/publish-ScriptSync.ps1 -Mode Verify -ArtifactDirectory $artifact
```

Publishはステージと成果物の一致・MSIX内部のライセンス/対応ソースを検査する。接続しないDryRunの後、Websiteのpublish-othello.pyの既存FTPS接続を利用して5ファイルを送る。Stage前のローカルhashと公開側を照合し、既存対象をすべて版別logsへ退避してから、パッケージ・CER・App Installer・アイコン・ページの順で原子的に置換する。公開後はFTPS読戻しとHTTP200、MIME、Content-Length、SHA-256を照合する。失敗時は完了扱いにせず、同じ成果物と基準で再実行する。

`-Mode DryRun` はStage済み5ファイルのhashと送信対象を検査するだけで接続しない。Stageは成果物内publish-baselinesへ配置先ごとの公開前hashを記録し、同じ成果物での再実行時に基準を上書きしない。公開側に未取得の変更がある場合や同版の異なるMSIXがある場合は送信前に停止する。

既存download/.htaccessにはMSIX・CER・App InstallerのMIME定義がある。配布記事への掲載が必要なリリースでは、Websiteの規約に従いScriptSyncの紹介・版・公開日・専用リンクを追加して確認する。

Planは読取専用。今回の「アップロードできるようにする」は経路整備であり公開済みとは記録しない。通常の「リリースしてください」はbetaのビルド・署名・Stage・対象コミット・Publish・公開検証を含むものとし、インストールとstable公開は明示指示がある場合だけ行う。

MSIXの末尾は再配布番号。dev/msix-version.jsonを正本とし、リリース後の再ビルドでは末尾を1増やしてコミットしてから生成する。公開済み0.0.5.7、次回再ビルド0.0.5.8。アプリ版は0.0.5を維持する。

## 専用リリースページ
2026-09-13のユーザー指定により、公開先は /download/ScriptSync/ とする。
紹介ページの編集元は dev/release-page/index.html。Websiteの www/download/ScriptSync/index.html へコピーする。icon.png は src-tauri/icons/Square150x150Logo.png からコピーする。共通CSS ../assets/styles.css を参照する。
公開対象はMSIX・CER・App Installer・index.html・icon.pngの5ファイル。ページの版と公開日を成果物に合わせ、接続しないDryRun後にFTPSで送信する。配布ファイルとアイコンの後にページを送信する。ページ・アイコンのHTTP200とSHA256、CSS取得、自動ブラウザー検証を確認する。
Websiteのdownloadは独立Gitリポジトリ。既存の.htaccess等の変更を含めず、ScriptSync専用ファイルだけをコミットする。
