# Talk編集ファイルの形式と検証範囲

2026-09-08調査。現時点の出力は試験対応であり、R12の実機受け入れは未完了。

## 根拠

- [Talk公式マニュアル](https://voisona.com/talk/manual/)：独自プロジェクト拡張子はtstprj。
- [公式インポート説明](https://manual.voisona.com/ja/talk/pc/2b6e9bc7efb18007bf95c5cb6fb1cad8)：CCS/CCSTからセリフをインポートできるが、Talk独自パラメーターの保持仕様ではない。
- [LibSasaraの公開形式メモ](https://github.com/InuInu2022/LibSasara/blob/master/LibSasara.VoiSona/Model/Talk/TstPrj.cs)と[パラメーター定義](https://github.com/InuInu2022/LibSasara/blob/master/LibSasara.VoiSona/Model/Talk/Utterance.cs)：ValueTreeの属性・型・階層・値変換を確認。
- [公開最小サンプル](https://github.com/InuInu2022/LibSasara/blob/master/test/file/template.tstprj)：1405 bytesの読込→書込がバイト一致。調査サンプルはoutput/researchのみで配布物に含めない。

## 実装

独立したRust実装でValueTreeを読み書きする。既定話者の田中さんに限定し、APIから取得した実際の話者版、解析済みTSML、音素長、スタイル比率、速度・音量・高さ・声質・抑揚を格納する。TSMLのアクセント・読みをそのまま保持し、原音・API結果は変更しない。ハスキー値が0以外なら失敗を明示する。TSML欠落、音素数不一致、非有限値、破損データを拒否する。

| 項目 | ファイルへの格納 | Talkでの保持確認 |
|---|---|---|
| 局所補正済み表記・読み・アクセント | textとtsml | 未検証 |
| 話者・版 | Voice（田中さん） | 未検証 |
| 音素長・間 | PhonemeOriginalDuration | 未検証 |
| スタイル | FrameStyle | 未検証 |
| 話速・音量・高さ・声質・抑揚 | 対応パラメーター | 未検証 |
| ハスキー | 0のみ | 未検証 |

生成時に自己再読込で構造一致を検証し、元の比較情報と音声hashをsource.jsonへ保存する。画面からTalkが保存したファイルも再読込して内容を確認できる。ただし自己再読込成功はTalk互換性の証明ではない。

## 未完了の受け入れ

computer-useのTalk画面取得が `Computer Use app approval timed out` で終了し、画面の読込→読み・アクセント・間の編集→別名保存→再読込→再合成を確認できていない。既存Talkウィンドウには未保存表示があり、内容を変更していない。操作承認が利用可能になったら専用検証プロジェクトで実施する。正式対応を示す表示には変更しない。
