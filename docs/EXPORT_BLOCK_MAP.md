# 音声・字幕・ブロックマップ連携仕様（schema_version 1）

対象：ScriptSync v0.0.4の結合出力。プレゼンテーションソフト側で、音声ファイルの順序とスライド切り替え時刻を読み取るための仕様です。

## 出力一式

「音声ファイルを出力」で、プロジェクトのデータフォルダー内 `exports/<出力ID>/` に次を生成します。

```text
0001_第1部_はじめに.flac
0001_第1部_はじめに.vtt
0002_第1部_概要.flac
0002_第1部_概要.vtt
block-map.json
manifest.json
```

音声形式はFLACまたはWAV、字幕はUTF-8のWebVTTです。字幕形式VTTの場合は音声と字幕が同じベース名で1対1に対応します。「なし」の場合はVTTを生成せず、ブロックマップの字幕ファイル欄はnullになります。`block-map.json`は出力一式につき1ファイルです。`manifest.json`は既存の検証用情報を保持し、`block_map: "block-map.json"`から連携用ファイルを参照できます。

出力が成功してから取り込んでください。生成中や失敗したフォルダーの一部ファイルを完成品として扱わないでください。再出力は別の出力IDになります。

## ファイル名

`4桁以上の出力連番_階層表記_本文部分.拡張子`です。階層表記は設定により省略／日本語／2桁数字となります。連番は必ず付きます。

本文部分は自動名・手入力名とも、`< > : " / \ | ? *` と制御文字を除去し、先頭20 Unicodeコードポイントに制限します。末尾の空白・ピリオドも除去し、空になった場合は `script` を使用します。20文字の制限に連番・階層・拡張子は含みません。絵文字をUTF-16の途中で切断しませんが、結合文字のまとまり単位を保証するものではありません。Windowsの予約名と同じ本文でも連番が先頭に付くため、最終ファイル名は予約名そのものにはなりません。

マップのファイル名は、置換・短縮後に実際に生成した名前です。外部側で原稿から名前を再計算しないでください。保存先の親パスが極端に長い場合まで回避する規約ではありません。

## block-map.jsonの基本規約

- UTF-8 JSONオブジェクト。`schema`が`"scriptsync.block-map"`、`schema_version`が`1`であることを確認します。
- `blocks`の配列順が再生順です。各ブロックを1スライドに対応させる想定ですが、実際のプレゼンファイルやスライドIDは含みません。
- マップ内の音声・字幕名は、**このJSONのあるフォルダーからの相対ファイル名**です。プロジェクト内部のパスとは異なります。
- `*_seconds`は秒の数値、`*_frame`／`*_frames`は整数フレームです。1フレームは左右チャンネルを合わせた1時点であり、サンプル値の個数ではありません。
- すべての区間は開始を含み終了を含まない `[start, end)` です。厳密な時刻計算はフレーム値を正本にし、秒値は `frame / sample_rate` で得られます。
- 未知の追加フィールドは無視できます。未対応のschemaまたはschema_versionは、黙って別形式として解釈せず対応外として扱ってください。

### 最上位

| キー | 型 | 意味 |
| --- | --- | --- |
| schema | string | 固定値scriptsync.block-map |
| schema_version | integer | 現在1 |
| export_id | string | 出力回の識別子 |
| project_id | string | 元プロジェクトの識別子 |
| generation | integer | 出力に使用したプロジェクト保存世代 |
| split_unit | string | all / part / chapter / section / utterance / custom。順に全体／部H1／章H2／節H3／発話／手動 |
| sample_rate | integer | 現在48000 Hz |
| channels | integer | 現在2（ステレオ） |
| total_duration_frames | integer | 全ブロックの音声長合計。ファイル前後の挿入無音も含む |
| total_duration_seconds | number | 同じ合計の秒表示 |
| blocks | array | 出力順のブロック情報 |

### blocksの各要素

| キー | 型 | 意味 |
| --- | --- | --- |
| block_index | integer | 0始まりのブロック番号 |
| slide_number | integer | 1始まりの推奨スライド番号。実際のスライドIDではない |
| audio_file / subtitle_file | string | 実際に生成された同名音声・VTTの相対ファイル名 |
| audio_hash | string | 音声ファイルのSHA-256（小文字16進数） |
| duration_frames / duration_seconds | integer / number | このファイル全体の長さ。先頭・末尾と途中の無音を含む |
| sequence_start_frame / sequence_end_frame | integer | 全ファイルを追加の間隔なしで順に再生した場合の全体時間軸での区間 |
| sequence_start_seconds / sequence_end_seconds | number | 上記の秒表示 |
| utterance_start_seconds / utterance_end_seconds | number | ファイル内の最初のテイク区間開始／最後のテイク区間終了 |
| text | string | このブロック内の全読み上げ原稿。発話を改行で連結。短縮なし |
| utterances | array | 出力対象の発話区間をファイル内順序で格納 |

### utterancesの各要素

| キー | 型 | 意味 |
| --- | --- | --- |
| segment_id / take_id | string | 元の発話・採用テイク識別子。出力順の番号とは別 |
| text | string | 録音画面で編集した現在の読み上げ原稿。JSONとしてエスケープするが、VTTのタグ用エスケープはしない |
| hierarchy | integer array | 部・章・節・項の番号。0はその階層の番号なし |
| kind | integer | 0本文、1部、2章、3節、4項 |
| start_frame / end_frame | integer | この音声ファイル内のテイク区間 |
| start_seconds / end_seconds | number | 上記の秒表示 |

無効な発話は`utterances`と`text`に入りません。発話ごとの長さはend-startです。ファイル単位に分割した箇所では、ファイルをまたぐ発話間無音は追加しません。各ファイルの先頭・末尾無音は設定どおり含まれます。

## 時刻の解釈とスライド連携

`utterance_*`は**トリム後の採用テイクを配置した区間**です。実際の声の立ち上がりを検出した時刻ではありません。テイクに残した0.1秒などの無音や途中の間は含まれます。単語単位の音声アラインメントはありません。VTTは同じ区間をミリ秒表示へ丸めています。

スライドを音声ファイル単位で切り替える場合は、そのファイルの再生開始時に`slide_number`へ移動し、再生終了イベントで次のblockへ進めます。全体タイムラインで制御する場合は`sequence_start_seconds`を切り替え位置とします。実際のプレーヤーにはファイル切り替えの待ち時間が生じ得るため、壁時計だけで進めず再生位置・終了イベントと同期する方法を推奨します。

発話単位の表示はファイル内の`start_seconds`と`end_seconds`を使います。全体時刻が必要なら`sequence_start_seconds`を加算します。ファイルの先頭無音中からスライドを表示するか、最初のテイク区間から表示するかは、連携先で選択できます。

## 例（識別子・hashは説明用）

```json
{
  "schema": "scriptsync.block-map",
  "schema_version": 1,
  "export_id": "example-export",
  "project_id": "example-project",
  "generation": 12,
  "split_unit": "custom",
  "sample_rate": 48000,
  "channels": 2,
  "total_duration_frames": 115200,
  "total_duration_seconds": 2.4,
  "blocks": [
    {
      "block_index": 0,
      "slide_number": 1,
      "audio_file": "0001_はじめに.flac",
      "subtitle_file": "0001_はじめに.vtt",
      "audio_hash": "0000000000000000000000000000000000000000000000000000000000000000",
      "duration_frames": 115200,
      "duration_seconds": 2.4,
      "sequence_start_frame": 0,
      "sequence_end_frame": 115200,
      "sequence_start_seconds": 0.0,
      "sequence_end_seconds": 2.4,
      "utterance_start_seconds": 0.1,
      "utterance_end_seconds": 2.1,
      "text": "はじめに、概要をご説明します。",
      "utterances": [
        {
          "segment_id": "example-segment",
          "take_id": "example-take",
          "text": "はじめに、概要をご説明します。",
          "hierarchy": [1, 0, 0, 0],
          "kind": 0,
          "start_frame": 4800,
          "end_frame": 100800,
          "start_seconds": 0.1,
          "end_seconds": 2.1
        }
      ]
    }
  ]
}
```

この例は先頭無音0.1秒、テイク区間2秒、末尾無音0.3秒です。後続ブロックがあれば、そのsequence_start_frameは115200、sequence_start_secondsは2.4になります。

## 連携先の確認項目

1. schemaと版を確認し、blocks順に読み込む。
2. 相対ファイル名をJSONと同じフォルダーから解決し、必要に応じてaudio_hashを検証する。
3. durationと開始終了の範囲を検証する。未知の追加キーは許容する。
4. 本文やファイル名をHTML・コマンドとして実行せず、通常のデータとして扱う。
5. 実際のスライド番号との対応を利用者が確認する。ScriptSync自体はプレゼンファイルを操作しない。

実装は[src-tauri/src/export_blocks.rs](../src-tauri/src/export_blocks.rs)、操作手順は[利用者ガイド](USERS_GUIDE.md)を参照してください。
