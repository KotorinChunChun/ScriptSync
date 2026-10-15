# セキュリティ診断・改善実装レポート（2026-10-15）

## 対象と方法
feature/presentation-prototype。Tauriアプリ（内部版0.0.6）、Rust storage/exchange/media、Nodeプレゼン試作、npm依存を確認。独立crate scriptsync-recorder も同じGit内。既存 dev/templates/PROJECT_RULE.md と workflow-scenarios を保持。追跡テキスト236ファイルの既知トークン/秘密鍵署名候補0、機密名追跡0。利用者の台本/録音/dev/sampleを不使用。
変更検証は .agents/skills/scriptsync-project-validation/SKILL.md に従う。

## 指摘・改善
| ID | 深刻度・条件 | 証拠 | 改善方針・実装状態 |
|---|---|---|---|
| SS-01 | 上流critical、当アプリでは開発時の条件付き。別のprototype pollutionがテストプロセス内で成立 | package-lock.json の tinypool 1.1.1、package.json:29 | tinypoolのworker optionを経由し実行につながる。[上流情報](https://github.com/advisories/GHSA-5gmw-xhrv-c9v3)。1系の最新も1.1.1で修正なし。npm提示はVitest 5へのmajor更新。互換性判断が必要なため保留。 |
| SS-02 | 上流moderate。Vitestのbrowser mockサーバーに悪意のある要求が到達 | package.json:29、package-lock.json の @vitest/mocker | [任意ファイル読取](https://github.com/advisories/GHSA-82fw-gwwq-j7x9)。本作業の既定はvitest run/jsdomでbrowser modeを確認していない。上流は4.1.11以上、major更新を保留。 |
| SS-03 | 上流high。不正なindexed source mapをビルド/処理する場合 | package-lock.json の source-map-js 1.2.1 | [イベントループ停止](https://github.com/advisories/GHSA-68fv-2mgg-jv7q)。1.2.2への互換修正を予定。 |
| SS-04 | 中。別途HTML/script注入が成立した場合 | src-tauri/tauri.conf.json:24 | CSP無効のため防御層不足。現時点で注入経路の実証なし。音声protocol/dev環境を含むCSPの設計・実機検証を別作業で行う。未実装。 |

## 確認できた防御
src-tauri/src/storage.rs:53-77 は相対パス/ADS/リンクによるルート外参照を拒否。exchange.rs:86-98 はエントリー読込を上限付きで処理。media.rs:28-37 はセッション登録トークンのみを参照し、:88-94 は応答量上限を適用。
tools/presentation/server.mjs:14-16,31-43 はloopback限定、Host/Origin/Fetch Metadata検証、ランダムセッショントークンを要求。APIキー・OBSパスワードを報告書に含めない。

## 診断時の依存監査
npm audit --json: critical 2 / high 1 / moderate 1 / low 0、計4パッケージ。critical数には依存経由のvitest集約を含み、独立した製品RCEを2件発見した意味ではない。production依存とdev依存を区別する。cargo-auditは未導入。後述のOSVによるRust lock照合を実施。

## 実装・検証状況
まず本レポートを差分・参照検査してコミット。続いてsource-map-jsをnpm update --ignore-scriptsで更新し、npm test / npm run build / 再auditで検証予定。実施結果はこの文書と現行作業の dev/history/v0.0.7/v0.0.7-imp.md に追記。ネイティブ録音・GUI・外部サービス接続は対象外。Git履歴・一般パスワードまで漏洩がない保証はない。

## 2026-10-15 改善実装結果

- [x] source-map-js 1.2.1→1.2.2をlockで更新。npm update source-map-js --ignore-scriptsを使用。npmが落とした他platformのlibc属性を元に戻し、対象依存以外の意味差分を残していない。
- [x] npm test: 33ファイル・225件成功。npm run build成功、npm run check:staticは30項目成功。
- [x] 再npm audit: high 1→0、critical 2 / moderate 1は残存。npm audit --omit=devは0。
- [ ] tinypool 1.xの最新は1.1.1で修正がなく、Vitest major更新に関わるため保留。vitest.config.ts:2はjsdomで、browser modeは使用していない。
- [x] 文書・lock差分検査。ネイティブGUI/録音/実機の再試験・公開はしていない。

## Rust依存の追加診断（2026-10-15）

OSV querybatchへCargo.lockの公開registry package名/版を照合した。src-tauriは580件で10 packageに指摘、録音crateの独立lockは151件でwasapiに指摘。alias重複や保守終了の勧告も含み、全件を実行可能な脆弱性として数えない。

- SS-05（高優先の互換修正）: src-tauri/Cargo.lockのrustls 0.23.44にはRUSTSEC-2026-0285/0346/0347/0348/0349、quinn-proto 0.11.17にはRUSTSEC-2026-0338/0340/0343。rustlsにはTLS境界不備・Debugによる秘密出力・ECH拒否後の送信・証明書解凍メモリ増大などの条件別問題、quinnにはpeer-trigger panic。src-tauri/Cargo.toml:27のreqwest/rustls-tlsからTLSは使用する。HTTP/3は有効にしていないのでQUIC指摘の実行時到達性は未実証。0.23.46/0.11.18へ互換更新して検証する。
- SS-06（判断保留）: wasapi 0.24.0、[RUSTSEC-2026-0332](https://rustsec.org/advisories/RUSTSEC-2026-0332.html)はWaveFormat::parseの範囲外読取。修正版0.25.0は0.x minor更新で録音デバイス互換検証を要するため保留。利用入力から実際の範囲外読取成立は未実証。
- glib 0.18.5のVariantStrIter不健全性（RUSTSEC-2024-0429）はLinux系依存。proc-macro-errorとunic系5 crateは保守終了等の通知。target/featureと上流Tauri依存経路の整理後に更新する。Windowsでの発現を実証していない。

原データ: https://api.osv.dev/v1/querybatch / https://rustsec.org/advisories/ 。Cargo-audit未導入でもこのlock照合は実施済み。実稼働バイナリの解析とは区別する。

## 2026-10-15 Rust/TLS依存の改善実装結果

- [x] src-tauri/Cargo.lockのrustls 0.23.44→0.23.46、quinn-proto 0.11.17→0.11.18をcargo update --preciseで更新。既存ユーザー差分なしを事前確認。
- [x] 差分は2crateの版/checksumと、Cargoの再解決でtempfileから既存getrandom 0.4.3→0.3.4への依存辺1箇所。新たなcrate追加や製品コード変更なし。
- [x] OSV querybatchで更新した両版の指摘0。Windows cargo treeでrustls→reqwest→本体を確認。quinn-protoは現在のWindows feature treeでは未使用で、lockの予防更新。wasapi 0.24.0は録音crate経由で有効、0.25移行は実デバイス互換判断を要し保留。
- [x] cargo test --manifest-path src-tauri/Cargo.toml --lib --locked --target-dir dev/history/v0.0.7/logs/security-audit-20261015/cargo-target: コンパイル成功、152件成功・0失敗・外部環境10件ignore。
- [x] 独立targetはGit対象外を確認。通常targetの実行アプリの停止・置換はしない。前段の225 UI試験/フロントbuild成功はそのまま有効。
- [ ] 実GUI・録音/可聴出力、TLS実サービス相手の攻撃再現、配布物/公開更新は未実施。稼働配布物へ自動適用したわけではない。
