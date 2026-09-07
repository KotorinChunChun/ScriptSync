# データ・連携仕様案

## 構成

Windows上のRust + Tauri v2アプリとする。TypeScript UIは表示と操作を担当し、状態遷移、保存、入力検証、合成・認識ワーカーの起動はRustバックエンドが担当する。重い処理をUIスレッドで実行しない。ASR・録音クレートの具体的な版と必要機器はP0で確定する。

| モジュール案 | 責務 |
|---|---|
| `src-tauri/src/domain` / `storage` | 原文・版・状態、原子的保存、入力検証 |
| `reference_audio` / `recorder` | 取込・録音・再生、音声ハッシュ、録り直し |
| `asr_adapter` | 音声→実発話転記、時刻、エンジン条件、失敗情報 |
| `normalization` / `alignment` | 比較用読み生成、差分、原文位置への対応候補 |
| `corrections` | 補正候補の承認、原文検査、混在原稿生成 |
| `amekoro_adapter` | EXE起動、plan生成、WAV・実条件取得 |
| `src/` UI / `evaluation` | 差分再生、採用、匿名比較、採点 |
| `analysis` / `export` | 後続のルール出力・Talk編集形式 |

## 実発話の文字起こしが最初の技術課題

通常のASRが「しじょう」という音を文脈から「市場」と書き起こし、それを読み変換器が「いちば」に戻すと、誤読を隠してしまう。漢字転記をあとからひらがなにした結果を、そのまま発話の証拠にはしない。原文をプロンプトとして与えて転記を原文へ寄せる経路も避ける。

ASRアダプターには原文や期待読みを渡さない経路を設け、直接かな・音素出力の可否、語の復元による誤り、時刻粒度をP0で評価する。音素認識も正解とは限らない。生転記、比較用かな、修正後かな、修正者、修正理由を別フィールドへ保存する。信頼度非提供はnullとし、架空の確率を付けない。

誤読音声が期待読みへ修復される場合は要聴取とし、手動かな入力を可能にする。既知の不正読みに対する検出が不十分なら「手動支援として継続／認識方式再選定」を報告し、自動検出を実証済みとしない。強制アライメントの時刻は位置推定にだけ用い、発音が正しかった証拠にしない。

## 正規化と原文位置対応

1. 原文は改変せずUTF-8で保存する。永続化する区間はUnicodeスカラー値単位の0始まり半開区間とする。Rustのバイト位置やJavaScriptのUTF-16位置との変換を境界で明示し、絵文字・結合文字を含む契約試験を行う。
2. 人間の確認済みかなを比較基準とする。片仮名→平仮名、幅・空白・句読点の比較用処理は版付き規則で行い、正規化前後の位置マップを保持する。
3. 長音、促音、拗音、濁音を無差別に同一化しない。「おう／おお」「は／わ」「へ／え」「を／お」は発話と表記の対応として明示管理し、助詞を原稿全体で機械置換しない。句読点は読み照合から除外できるが、合成文では保持する。
4. 原文の語／句区間と確認済みかな区間の対応を作る。形態素解析やAIは境界の候補生成に限り、人間が確認・修正できるようにする。数字、固有名詞、同語反復、非一対一対応を許容する。
5. 人間とAIの比較用かなに編集距離のアライメントを適用し、置換・挿入・脱落を返す。時刻と語句対応で原文候補範囲に戻す。複数の同等経路、反復、言い直し、境界不明はambiguousとして保留する。
6. 差分の一文字だけで不自然な表記になる場合は、確認済み語区間まで拡張する。文全体への拡大は自動で行わない。元の原文に対し採用区間を末尾から置換する。

照合一致は「かな列が一致した」という結果に留める。橋／箸のようなアクセント差、ASRの誤認識、合成のゆらぎは聴取評価で扱う。

## 保存契約（新ツール固有、version=1案）

| 記録 | 必須項目 |
|---|---|
| manuscript | id、revision、text、SHA256、segment一覧と原文区間 |
| reference | 音声相対パス・SHA256、原文版、転記生データ、確認済みかな、状態、確認日時 |
| reading_map | 原文区間、基準かな区間、対応方法、確認状態、正規化版 |
| transcription | 音声SHA256、ASR名・版・設定、raw、kana、時刻区間、手動修正履歴、confidenceまたはnull |
| correction | id、原文SHA256、start/end、original、replacement、基準読み区間、AI音声区間、理由、候補／採用／却下／保留 |
| variant | A/B/C/D、実送信文、plan、要求条件、実条件、音声SHA256、親候補、試行番号 |
| run | id、日時、アプリ版、実行環境、各工程状態・失敗、各モデル・辞書条件、fixture・設定ハッシュ |
| rating | run/case/variant、音声SHA256、匿名評価者、各評価軸・誤読位置、コメント、提示順seed、日時 |

例：原文「市場で買い物をします。」の先頭だけを対象に、`start=0,end=2,original="市場",replacement="いちば"` と保存する。同文中の別の市場にはこの補正を流用しない。保存時は原文ハッシュとスライス一致を再検査する。

相対パスは実行ディレクトリ内だけを参照する。パス逸脱・重複ID・不正区間・古い原文版・音声ハッシュ違いを拒否。JSONは一時保存から原子的に確定し、前版とリビジョンを保持する。録音・原稿・AIログは公開Gitに含めない。

```text
output/tests/reading/runs/日時-GUID/
  run.json
  cases/<case-id>/
    manuscript.json
    reference/                 # 原音、転記、確認済み読みと対応
    corrections.json
    variants/<A|B|C>/<attempt>/ # plan、result、WAV、転記、ログ
  ratings/
  report.html
```

工程状態はpending/running/succeeded/failed/blocked/cancelled。音声生成成功と評価済みは独立。音声はデコード可能・長さ正・ハッシュ確定まで検査する。成功件数／必須件数と未評価件数を表示し、空HTMLを成功扱いしない。再開は入力・版・声条件・ASR条件・音声ハッシュが一致する工程だけを再利用する。

## amekoro-speech.exeとの連携

初期版は外部プロセス連携を採用し、雨衣ソースのコピーやGUI内への組込みを必須にしない。EXEパスとDPAPI設定パスはローカル設定とする。新リポジトリに認証ファイルをコピーせず、保存したWindowsユーザーで読み込む。設定の再保存を復号失敗の一律対処にしない。

現行コードで確認した契約：`synthesize --plan` はCodexへ再提案せず `proposal.reading.strip()` を発声する。新規`--text --reading-policy original`でも提案処理は通るため、厳密な固定比較にはplan経路を使う。新ツールは完全な検証済みplanを一度固定し、候補間でreadingだけを変える。

```powershell
# 下記は実装時の連携例。入力planと実行フォルダーの作成後に使う。
& 'C:\develop\amekoro\release\amekoro-speech.exe' synthesize --plan '<実行フォルダー>\A.plan.json' --settings 'C:\develop\amekoro\settings\voisona.dpapi' --output '<実行フォルダー>\A.wav' --result-json '<実行フォルダー>\A.result.json'
```

実コードではshellを使わず引数配列で起動し、stdout/stderrと終了コードを別々に保存する。CLIの通常エラーは1、引数解析エラーは2になり得る。外側のタイムアウトを設け、キャンセル時は部分生成を失敗として残す。Talkジョブが残る場合の処理はP0で確認する。

planは `format="amekoro.speech-plan"`、version=1/2、原文text、voiceのname/version、proposalのid/reading/model_expression/voice_expression/reason/tone_parametersが必要。初期連携はv1の確認済みplanを採用し、新ツール独自の補正証跡を別JSONへ保存する。v2を使う場合は既存validatorの追加読みフィールドとの整合を契約試験で確認する。新ツールのversionと混同しない。

声数値は既存toneの範囲に従い、既定値はalp=0、huskiness=0、intonation=1、pitch=0、speed=1、volume=0、style=""、style_intensity=0。具体的な固定話者名・版を導入環境から選び、未確定話者を公平比較に使わない。返却requested.text/global_parametersとactualの話者・値を保存・照合する。実条件が取得できなければunknownを明示し、厳密な比較合格にしない。

原文・合成読みとも500文字上限がある。かな展開後にも検査し、超過は句境界で共通分割する。A/B/Cそれぞれ異なる分割は比較条件を壊す。原文へのグローバル位置マップを維持する。GUI側240文字基準とは別制約。

`memory`は全合成完了後のWAVでストリーミングではない。初期版はfileに統一する。出力先の既存ファイルは拒否されるため、試行番号別の新規パスを使う。`AMEKORO_LOG_DIR`は子プロセスの実行フォルダーへ設定し、環境全体は記録しない。

Talkのanalyzed_text、phonemes、phoneme_durationsは取得された実データを保存できるが、かな照合やTalk編集ファイルの代替にはしない。元の発音詳細を補正後原稿へ使い回さない。
