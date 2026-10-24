# ソース公開前の修正・再点検結果

2026-10-24。[初回点検](report.md)の指摘を修正し、0.0.6.5のソース公開準備を整えた。GitHubのpublicへの切替は実施しない。

| 指摘 | 対応・結果 |
| --- | --- |
| 本体LICENSEと録音基盤の権利 | ユーザーがMITと本人所有を回答。ルートLICENSE、package/Cargo、ORIGIN、README、要求のQAへ反映。第三者部分にMITを上書きしない |
| 第三者本文と配布条件 | Rust386・npm本番2・素材・SDKを列挙し719ファイルを生成。同梱が不足/変更しているMSIXは公開処理で拒否。MPL対応ソース18件のchecksumをCargo.lockと照合。Microsoft公式SDKの静的ローダーも照合しLICENSE/NOTICEを維持 |
| LGPLのMP3エンコーダー | ScriptSyncはWAV/FLAC構成として組込みを無効化。録音crate単体の既定MP3機能は維持し、派生配布でLGPL対応が必要なことを記載 |
| 既知依存の問題 | Vite7.3.7・Vitest5.0.5・wasapi0.25.0、TLSの修正版、Tokio1.53.3、zip9.1.0へ更新。暗号化ZIP等を含むRust回帰が成功。npm監査0、Rust386件は未解消の脆弱性/不健全性0 |
| CSP | self・IPC・専用音声に限定し、開発HMRは開発時だけ追加。実WebView2でinline script拒否と音声Range/デコード/時計を自動検証 |
| 秘密情報の誤追加 | ignore、UTF16を含む既知秘密検出、現行/全参照履歴/削除済み機密名の走査、値を出さない報告、読取権限のみのCIを追加。走査に指摘なし |
| 公開処理 | 平文FTPを使わず、証明書検証付きFTPS、基準hash、全既存対象の退避、原子的置換と読戻しを整備。5ファイル限定と同版異内容拒否をローカルで検証 |
| 画面操作に頼った確認 | 一括verifyと、専用設定・合成データでのWebView2/CDP自動試験を追加。マウス座標操作・Computer Use不使用。新規作成/取込の任意の明示パスも形式・絶対パスを検査する |

## 試験結果

- UI34ファイル232件、Rust本体157件、録音crate既定34件・MP3なし35件、秘密検出/CSP4件が成功。
- 実WebView2の10項目がDebug版と最終Release EXEで成功。CSP、実IPC、プロジェクト作成/取込、元を保持したテイク結合、PageDown時の原稿保護、音声protocol、無音WASAPI、編集バー/時刻/停止、設定中停止拒否・正常停止を確認した。
- 自動試験で発見した「停止後に古いpause応答が再生時計を戻す」競合を、世代照合と明示的なネイティブ停止で修正し、遅延応答の回帰を追加した。
- MSIX0.0.6.5の署名はValid。ビルド元 `c3d8ddfa3b7e4ec4128a08e310d1ffd8e9d0fb14`、dirty false。MSIX内のEXEはビルド成果物と一致し、719ファイルのライセンス/対応ソースを照合した。OPCで@・+がpercent-encodeされる名前もデコードして照合する。
- MSIX SHA-256：`5C808A3FA720ECDA57D86314F1C1CD5E9EEDBF11914ABBB7810BC53AD3C80DE1`
- EXE SHA-256：`07A4888986F107FD8068A9E2C8FFC3EE540F8E0559905627BEEFD1CF32E2E43F`

生データは[リリース記録](../../history/v0.0.6.5/v0.0.6.5-imp.md)が示す版別logsに保存し、Gitと公開アップロードから除外する。

## 再実行

[自動検証ガイド](../../../docs/automated-testing.md)を参照。`npm run verify` で基本検証、`npm run verify:native` で実WebView2まで、`npm run verify:native -- -AudioDevice` で実デバイスを使う無音再生と編集画面の時計まで検証する。`npm run test:native -- --exe <EXE>` で既存のリリースEXEを直接確認できる。

## 残る範囲

UNIC0.9.0の保守終了通知5件はTauriの間接依存で修正版がなく、公式のunmaintained分類を確認して脆弱性とは分けて表示する。上流の置換状況を追跡する。新しい勧告を広く無視せず、未知の指摘は失敗にする。

実マイクの音質、可聴評価、全機器、OSファイル選択画面、非アクティブ時のグローバルキー、MSIXの導入/更新は今回の成功範囲に含まない。未知の秘密形式・GitHubの添付等・全攻撃経路の網羅保証も行っていない。ライセンス本文に含まれる第三者の公開済み帰属情報は保持する。
