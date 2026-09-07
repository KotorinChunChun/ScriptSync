# Talk編集ファイルの形式と検証範囲

2026-09-08調査。出力は試験対応。Talk 1.3.8.2・田中さん2.0.1の既定条件で実機往復を確認済み。

## 根拠

- [Talk公式マニュアル](https://voisona.com/talk/manual/)：独自プロジェクト拡張子はtstprj。
- [公式インポート説明](https://manual.voisona.com/ja/talk/pc/2b6e9bc7efb18007bf95c5cb6fb1cad8)：CCS/CCSTからセリフをインポートできるが、Talk独自パラメーターの保持仕様ではない。
- [LibSasaraの公開形式メモ](https://github.com/InuInu2022/LibSasara/blob/master/LibSasara.VoiSona/Model/Talk/TstPrj.cs)と[パラメーター定義](https://github.com/InuInu2022/LibSasara/blob/master/LibSasara.VoiSona/Model/Talk/Utterance.cs)：ValueTreeの属性・型・階層・値変換を確認。
- [公開最小サンプル](https://github.com/InuInu2022/LibSasara/blob/master/test/file/template.tstprj)：1405 bytesの読込→書込がバイト一致。調査サンプルはoutput/researchのみで配布物に含めない。

## 実装

独立したRust実装でValueTreeを読み書きする。既定話者の田中さんに限定し、APIから取得した実際の話者版、解析済みTSML、音素長、スタイル比率、速度・音量・高さ・声質・抑揚を格納する。TSMLのアクセント・読みをそのまま保持し、原音・API結果は変更しない。ハスキー値が0以外なら失敗を明示する。TSML欠落、音素数不一致、非有限値、破損データを拒否する。

| 項目 | ファイルへの格納 | Talkでの保持確認 |
|---|---|---|
| 局所補正済み表記・読み・アクセント | textとSynthesisData/data | 読み変更・アクセント核変更を保存後も保持 |
| 話者・版 | Voice（田中さん）とlang | 2.0.1を保持 |
| 音素長・間 | PhonemeOriginalDurationとstart | 既定の音素長表示、開始0.100秒の編集を保持 |
| スタイル | FrameStyle | Normal=1、他0を保持 |
| 話速・音量・高さ・声質・抑揚 | 対応パラメーター | 既定値のみ確認。非既定値は未検証 |
| ハスキー | 0のみ | 確認済み |

生成時に自己再読込で構造一致を検証し、元の比較情報と音声hashをsource.jsonへ保存する。画面からTalkが保存したファイルも再読込して内容を確認できる。ただし自己再読込成功はTalk互換性の証明ではない。

## 実機往復検証（2026-09-08）

ユーザーの操作承認後に実施。旧形式6の出力は文章と読みを開けたものの、ボイスが未選択となった。Talk 1.3.8.2が保存した形式7と比較し、Track/lang=ja_JP、ContentsからClipsへの変更、Utterance/tsmlからSynthesisData/data（tsmlルート付き）への変更で修正した。旧形式の再読込にも対応する。

1. 既存の未保存プロジェクトを `output/tests/talk-original-preserved-20260908.tstprj` へ退避。
2. Rust生成の `output/tests/talk-format7.tstprj` を開き、田中さん、本文「いちばへ行きます。」、読み「イチバ」、アクセントと長さ1.425秒を確認。
3. Talkで読みを「イチ」に変更し、アクセント核を「チ」へ変更（最初の語のhlがhllからlhへ変化）。開始前の間を0.100秒へ変更。
4. `output/tests/talk-gui-edited-20260908.tstprj` へ別名保存。Talkを終了してウィンドウ消失を確認し、再起動して同ファイルを再読込。
5. 読み・アクセント・開始時刻の保持を確認し、再生とミックスダウンWAV書き出しを実行。
6. `output/tests/talk-gui-resynthesized-20260908.wav` は48 kHz、16 bit、ステレオ、66960フレーム（1.395秒）。ピーク11422、RMS約2052.87で非無音、クリップなし。

生成前とTalk保存後の小さな合成検証ファイルを `tests/fixtures/talk/` に格納。Rust回帰試験で生成構造一致、話者版、表記、読み変更、アクセント変更、開始時刻と旧形式再読込を検証する。既存ユーザープロジェクトとWAVは配布物に含めない。

試験対象は田中さん2.0.1の既定条件のみ。他のTalk版、話者、非既定の速度・音量・ピッチ・声質・抑揚・スタイル、発話内部のポーズ編集は未検証。人間による音質採点の合格を意味しない。生成時のsource.jsonのverificationは各ファイルに実施する自己再読込を表す。
