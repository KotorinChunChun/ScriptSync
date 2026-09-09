# 効果音の定義

本プロジェクトで効果音の用途・音源・音量を変えた場合は、このファイルと `src/soundEffects.ts` を同時に更新する。

## 現在の割当

| ID | タイミング | 同梱ファイル | 原音名 | 音量 |
| --- | --- | --- | --- | --- |
| start | マイク収録を始める前。再録音・自動継続でも同じ | public/sounds/start.ogg | open_001.ogg | 0.20 |
| saved | 録音した新テイクのファイル保存成功後 | public/sounds/saved.ogg | confirmation_001.ogg | 0.20 |
| next | 採用成功後。次の自動録音開始より前 | public/sounds/next.ogg | confirmation_002.ogg | 0.20 |
| invalid | 操作できないF1～F4やリロードキー | public/sounds/invalid.ogg | error_001.ogg | 0.12 |

音量はHTMLAudioElementの0～1。無効操作の連打は300ms以内をまとめる。音が重ならないよう順番に再生し、音源読込失敗・再生拒否・1.8秒のタイムアウト時も録音操作をロックしない。開始・移動の音が終わってから収録へ進む。保存音は保存成功時だけ鳴らす。

## 出典・ライセンス

- 作者：Kenney
- 作品：Interface Sounds 1.0（2020年公開）
- 公式配布ページ：https://kenney.nl/assets/interface-sounds
- 取得元：https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip
- 確認日：2026-09-09
- ライセンス：CC0 1.0。公式ページと配布ZIPのLicense.txtで確認。
- 原文の同梱先：public/sounds/Kenney-LICENSE.txt
- 条文：https://creativecommons.org/publicdomain/zero/1.0/

元のOGGを変換せずコピーしている。CC0のためアプリへの同梱・再配布に使用する。音の好みは今回の仮選定であり、使用後の意見に合わせて差し替え可能。

## 差し替え手順と候補

1. 用途に対応するIDを選ぶ。短い開始音、明確な完了音、小さな拒否音を優先する。
2. 再配布可能な音源であることを確認し、ファイルをpublic/soundsへ置く。
3. src/soundEffects.tsのfile・volumeを変更する。同じファイル名への置換でもよい。
4. 本表の原音名、出典、ライセンス、確認日を更新し、必要なライセンス原文を同梱する。
5. 開始・保存・次へ・無効操作の聴き分け、音量、音源が壊れた場合の操作復帰を確認する。

同じ公式パック内のopen_002～004、confirmation_003～004、error_002～008を次の比較候補とする。差し替え候補は全件同梱せず、必要時に公式パックから選ぶ。音の長さを1.8秒以上へ変更する場合はタイムアウト設計も見直す。

2026-09-09改訂：通常録音画面のF1～F4にも同じ効果音を適用する。終了延長の待機中には保存音を鳴らさず、実際の保存完了後だけ鳴らす。音源・音量の変更はなし。
