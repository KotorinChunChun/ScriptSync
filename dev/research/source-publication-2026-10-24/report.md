# ScriptSync ソース公開前点検（2026-10-24）

## 結論

公開準備は未完了。今回の走査では秘密情報の混入を検出しなかったが、公開ライセンス・取り込み元の権利・第三者ライセンスの配布方法・依存の既知問題を整理する必要がある。これらを確認するまでリポジトリをpublicへ変更しない。

本調査は点検と報告のみ。ライセンスの選択・新規付与、製品コード修正、依存更新、履歴の削除・書換え、リポジトリ公開設定変更は実施していない。先行するリリース作業の0.0.6.4は署名済みMSIXの作成まで完了し、外部への公開は保留した。

## 対象と方法

- 元の実装確定点：`a006859299da62fc9c753c363bbb77b11039710f`。
- リリース準備の点検点：`50124df634742cc2a1789d62d69055b78ddf7f2a`、`feature/release-0.0.6.4`。実装確定点との差は版・日付・配布案内・リリース記録。
- 現行追跡ファイル355件、ローカルの全参照から到達可能な149コミット、blob 1,416件（24,718,751 bytes）を調査。GitHubの既存3ブランチを照合し、リポジトリはprivate、既存GitHub Releaseなしを確認した。
- Git追跡ファイル名、既知の秘密鍵・GitHub/OpenAI/AWS/Google/Slackトークン・JWT・URL埋込み認証・固定パスワード候補、履歴のメールアドレスを独自ルールで照合した。検出値はログへ出さず位置と分類だけを保存した。
- npm auditを現時点で再実行し、開発依存を含む全体と `--omit=dev` を分けて記録した。Rust wasapiの既存指摘は現行ソースと上流勧告を再確認した。
- 生データ・読取スクリプトは専用worktreeの `dev/history/v0.0.6.4/logs/release-20261024/` に保存し、Git対象外とした。

走査は既知パターンと追跡ファイルを対象とする。未知の形式の秘密・暗号化データ・参照不能なGitオブジェクト・GitHubのIssue/添付/Actionsログ・全外部サービスの安全性を網羅的に保証するものではない。

## 結果

| 優先度 | 指摘 | 根拠 | 公開前に必要な対応 |
| --- | --- | --- | --- |
| 高 | プロジェクト全体の公開ライセンスが未定義 | ルートLICENSEなし。package.json、src-tauri/Cargo.toml、録音crateのCargo.tomlにも自身のライセンス宣言なし | 権利者がソース閲覧・改変・再配布の許可範囲を決め、LICENSEとREADMEへ明記する。無断でMIT等を付与しない |
| 高 | 取り込んだ録音基盤の公開許諾をリポジトリ内で確定できない | [ORIGIN](../../../crates/scriptsync-recorder/ORIGIN.md):3–5は受領ソースからコピーし「公開ライセンスの新規付与は行わない」と記載。履歴には元のdev/sample配下のコードも存在する | 元ソースの権利者・公開許諾・利用条件を確認する。本人所有ならその由来と許可を記録する。無許諾と断定したものではない |
| 高 | 第三者ライセンスの配布対応が未整備 | [録音crateの一覧](../../../crates/scriptsync-recorder/THIRD-PARTY-NOTICES.txt):3はライセンス本文の代用ではないと明記。mp3lame-encoder 0.2.5、mp3lame-sys 0.1.11はLGPL-3.0、Symphonia系はMPL-2.0。手書きMSIXのエントリーに独立したNOTICE/LICENSEなし | 採用版と実際の組込み範囲を確定し、必要な著作権表示・ライセンス本文・対応ソースや再リンク方法等の提供方法を整理する。LGPL依存があることだけで公開不可とは断定しない |
| 高 | 公開後に開発する人へ渡す依存に既知の問題が残る | npm audit：critical 2 / high 1 / moderate 1。対象はVitest・tinypool・@vitest/mocker・Vite。Rust wasapi 0.24.0にも勧告あり | Viteの互換修正とVitest等の更新を検討・検証する。wasapiは録音デバイスを含む互換確認を伴って対応する。npmの件数は集約を含み、製品で独立した攻撃を4件実証した意味ではない |
| 中 | CSPが無効 | [Tauri設定](../../../src-tauri/tauri.conf.json):24の `csp: null` | 音声protocol・開発環境を含めたCSPを設計・検証する。今回HTML注入や攻撃成立を実証したものではない |
| 中 | 将来の誤追加を防ぐガードが限定的 | [.gitignore](../../../.gitignore)は.env・出力・private・録音用sampleの一部を除外するが、秘密鍵・実プロジェクト・録音を置ける全パターンを除外する構成ではない | 必要な合成fixtureを維持しつつ秘密鍵・端末設定・実原稿・録音の追加防止と、push前/CIの秘密検出を整備する |

LGPL依存はlockに存在するだけではない。録音crateのcodecs.rsがMP3エンコーダーを参照し、ScriptSyncのexport_blocks.rsから同じencode_audio_fileを呼ぶ。取得済みmp3lame-sysのWindows build.rsにはCソースの静的ビルド設定がある。最終バイナリへの残存範囲を断定せず、配布条件を確認すべき具体的な経路として記録する。

## 混入を確認しなかったもの

- 既知形式のAPIキー・秘密鍵・固定パスワード候補は現行・履歴とも0件。
- .env、署名設定、machine-settings.json、PFX/PEM/秘密鍵、実原稿・録音・ZIP/DB/ログに該当する機密名の追跡は見つからなかった。
- メール候補58件は画像名 `128x128@2x.png` とページ録画名 `page@....webm` による誤検出を全件確認した。コミットのメールはGitHubのnoreplyのみ。
- talk fixtureは由来文書上「いちばへ行きます。」の合成試験用。Kenneyの4効果音にはCC0の文書がある。これらを個人録音と扱わない。
- 元の作業領域の未コミットな開発テンプレート・運用サンプル・画像はリリース用worktreeへ取り込んでいない。今後も一括 `git add` で含めない。

## 依存監査の区別

`npm audit --omit=dev` は0件。ただしRust依存の安全性や製品全体の安全性を示す結果ではない。

wasapiの [RUSTSEC-2026-0332](https://rustsec.org/advisories/RUSTSEC-2026-0332.html) はWindowsのWaveFormat::parseの範囲外読取りを扱い、修正版は0.25.0以降。現行は0.24.0。アプリ利用入力から問題が成立することを本点検で実証していない。

開発依存の勧告は [tinypool](https://github.com/advisories/GHSA-5gmw-xhrv-c9v3)、[Vite](https://github.com/advisories/GHSA-698v-4r3x-3vcv) 等。以前の2026-10-15の診断結果をそのまま現在値とせず、今回のauditでhigh 1件を含む現況へ更新した。

公開とオープンソースとしての利用許可は別に判断する。[GitHubのライセンス案内](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository)を参照。LICENSE不在だけでWeb上の閲覧公開が直ちに違法になると判断したものではない。

## リリース作業の到達点

0.0.6.4の静的検査38項目、配布版タイトルのRust試験1件、TypeScript/Viteビルド、Rustリリースビルド、MakeAppx、SignTool、Authenticode/CMS署名検証と内包EXE照合が成功。公開準備試験の明示公開ガード・5ファイル限定・同版異内容拒否も成功。

- MSIX SHA-256：`86F20BF6FDEA0419B621DFED673E5B9D84025D0C889DA2F6F00EB118FEE93C22`
- 内包EXE SHA-256：`711755365BBFF3B040759E8D6735189431C21C0E80B277344F4424C79A071A01`
- ビルド元：`50124df634742cc2a1789d62d69055b78ddf7f2a`
- build-infoのdirtyはtrue。TauriによるCargo.toml再書込み後にGitが変更扱いとしていたが、ビルド後のgit hash-objectはHEADのblobと一致し、差分・ステージ差分が空であることを確認した。build-infoの記録は書き換えていない。

成果物は専用worktreeの `release/msix-v0.0.6/20261024-0.0.6.4/` に保持。GitHub Release作成・push・配布サイト更新・インストール・本体の採用版変更は未実施。署名成功と、ソース公開・第三者ライセンス対応の完了を混同しない。
