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
npm audit --json: critical 2 / high 1 / moderate 1 / low 0、計4パッケージ。critical数には依存経由のvitest集約を含み、独立した製品RCEを2件発見した意味ではない。production依存とdev依存を区別する。cargo-auditは未導入でRust全crateの脆弱性DB照合は未実施。

## 実装・検証状況
まず本レポートを差分・参照検査してコミット。続いてsource-map-jsをnpm update --ignore-scriptsで更新し、npm test / npm run build / 再auditで検証予定。実施結果はこの文書と現行作業の dev/history/v0.0.7/v0.0.7-imp.md に追記。ネイティブ録音・GUI・外部サービス接続は対象外。Git履歴・一般パスワードまで漏洩がない保証はない。
