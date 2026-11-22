# heapless公告の確認と隔離検証（2026-11-22）

## 対象と判断

ScriptSyncの開発正本 `C:/develop/KotorinToys/tools/ScriptSync` のコミット `c6d20dd` を確認した。対象は `src-tauri/Cargo.lock` と `crates/scriptsync-recorder/Cargo.lock`。既存のTranscriptionServer連携は別worktreeの `feature/transcription-server`、`ce630a1` にある。

両lockはheapless 0.8.0を選択している。Windows x64の通常構成の依存経路は、アプリ → 録音crate → oxiaudio-encode 0.2.1 → flacenc 0.5.1 → heapless 0.8.0。録音crateの単体構成も同じ経路である。

公式の修正版はheapless 0.9.4。しかし、flacenc 0.5.1は `heapless = "^0.8.0"` を要求する。2026-11-22に公式crates.ioを照会した時点で、flacencの最新安定版は0.5.1、oxiaudio-encodeは0.2.1だった。lockだけを更新する互換パッチとしては採用できない。

隔離環境ではflacencの互換修正とheapless 0.9.4を検証する。製品への採用には上流ソースの改修・保持と必要なRustバージョンの引き上げが伴うため、承認が必要。

## 公告対象APIの利用

両manifestの `cargo tree --locked --offline --target x86_64-pc-windows-msvc -e features -i heapless` で、heaplessのdefaultとserdeが選択されることを確認した。アプリと録音crateからのheapless直接利用はない。

flacenc 0.5.1のRustソースにあるheapless参照を調べた。使われるコンテナーは固定長Vecで、FLACのwarm-up値、符号化用バイト、LPC係数を保持する。要素はi32、u8、f64などの数値型であり、LPCの型パラメーターもCopy制約を持つ。SPSC、IndexMap、Deque、HistoryBuf、SortedLinkedList、poolの利用は確認されなかった。

| 公告 | 対象APIと今回の確認 |
| --- | --- |
| RUSTSEC-2026-0574 | FindMutView::pop。対応するリンクリストを使っていない |
| RUSTSEC-2026-0575 | Spsc::peek。SPSCを使っていない |
| RUSTSEC-2026-0576 | IndexMap::truncate。IndexMapを使っていない |
| RUSTSEC-2026-0577 | パニックするDropと、各種clear・retain・truncate・IntoIter::drop・write。今回のVec要素はCopyの数値で、パニックするDropを持たない |
| RUSTSEC-2026-0578 | SortedLinkedList::zeroize。SortedLinkedListを使っていない |
| RUSTSEC-2026-0579 | Deque::try_from。使っていない。公告の関数別範囲は0.9.2以上0.9.4未満 |
| RUSTSEC-2026-0580 | arc_pool・object_pool・box_pool・旧pool。使っていない |
| RUSTSEC-2026-0581 | Deque::make_contiguous。Dequeを使っていない |
| RUSTSEC-2026-0582 | u16::MAX要素のIndexMap::insert。IndexMapを使っていない |

この確認は現行依存ソースの静的調査である。heapless 0.8.0そのものの修正を意味せず、lock全体を検査する監査では公告が残る。

## 隔離した変更

作業先は `C:/develop/KotorinToys/artifacts/worktrees/scriptsync-heapless-review-20261122`、ブランチは `feature/security-heapless-review-20261122`。候補と実行ログは `logs/heapless-review-20261122/` に保存する。

1. 製品の依存条件のまま `cargo update -p heapless --precise 0.9.4 --offline` を実行したところ、flacencの `^0.8.0` 制約で解決に失敗した。
2. flacencの公式0.5.1パッケージをSHA-256でlockと照合し、隔離コピーのheapless指定だけを `=0.9.4` へ変更した。Windows向けのコンパイルはdatatype.rsの2か所で失敗した。エラー型が `()` から `CapacityError` へ変わっている。
3. その2か所の `map_err(|()| ...)` を `map_err(|_| ...)` へ変更し、既存のエラー内容を保持した。候補のコンパイルは成功した。
4. 同じ候補をCargoのコマンド引数 `--config patch.crates-io.flacenc.path=...` で製品と録音crateへ適用し、既存試験で検証する。製品manifestへpatchは追加しない。

公式アーカイブとの比較で、候補の変更はCargo.tomlとsrc/component/datatype.rsの計3行だけだった。差分は `logs/heapless-review-20261122/flacenc-heapless094.patch` に保存した。

| 公式パッケージ | SHA-256 |
| --- | --- |
| flacenc 0.5.1 | `74c892c2b5fa08f967e8b5ad29121570b4762202f2402033ce08479ec65eccd0` |
| heapless 0.9.4 | `411fdef4df93751117bd9ef8caa0e7f99c65dafec5bd84313eb8e2fdd75e32f2` |

heapless 0.9.4のCargo.tomlはRust 1.88を要求する。録音crateが宣言するRust 1.85からの引き上げになる。今回の検証環境はRust 1.99.1。flacencのライセンスはApache-2.0で、改修版を製品へ含める場合は変更表示、ライセンス、配布時の対応ソース管理も更新する必要がある。

## 検証結果

候補のheapless 0.9.4とflacenc互換修正を使い、次を実行した。どちらのlockも変更されたパッケージはflacencとheaplessだけだった。

| 検証 | 結果 |
| --- | --- |
| 隔離したflacencのWindows向けcargo check | 成功 |
| ScriptSync本体のRustライブラリー試験 | 160件成功、外部環境などの10件は除外 |
| 録音crateの既定構成 | ライブラリー28件、CLI6件成功 |
| 録音crateのMP3なし構成 | ライブラリー29件、CLI6件成功 |
| Windowsの画面内包Debug EXEビルド | tauri/custom-protocol構成で成功 |
| 版・構成の静的チェック | 30項目成功 |

本体の試験にはFLACヘッダー、WAV/FLACの再デコードと無音間隔・音量、ローカル模擬HTTPへの接続、録音停止の遅延と取消、UIセッション再接続時の停止・保存を含む。実マイク、実再生機器、聴感、配布物の検証は実施していない。

再実行時は、作業先のPowerShell 7から次を実行する。録音crateのMP3なし構成には `--no-default-features` を追加する。

```powershell
$candidate = (Resolve-Path 'dev/history/v0.0.8/logs/heapless-review-20261122/flacenc-0.5.1').Path.Replace('\', '/')
$patch = "patch.crates-io.flacenc.path='$candidate'"
cargo tree --manifest-path src-tauri/Cargo.toml --offline --config $patch -i heapless
cargo tree --manifest-path crates/scriptsync-recorder/Cargo.toml --offline --config $patch -i heapless
cargo test --manifest-path src-tauri/Cargo.toml --offline --locked --config $patch --lib
cargo test --manifest-path crates/scriptsync-recorder/Cargo.toml --offline --locked --config $patch --target-dir src-tauri/target
cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --config $patch --features tauri/custom-protocol
```

上のコマンドは隔離候補用であり、復旧したlockでは、先に2つの `cargo tree` で候補を解決してから `--locked` の試験を行う。

## 復旧方法と採用前の確認

候補を指定するCargoの `--config` を外し、隔離作業で退避した2つのlockを元の場所へ戻す。退避ファイルは `logs/heapless-review-20261122/desktop-before.lock` と `recorder-before.lock`。復旧後にGitの基準コミットとバイト単位で比較する。正式な製品設定とGitへの採用対象は、復旧した状態を使う。

検証後はこの手順で復旧し、2つのlockが退避時と同一のSHA-256で、基準コミットとの差分もないことを確認した。heaplessの採用版は0.8.0のままで、0.9.4とflacencの互換修正は隔離候補として保管した。

0.9.4を製品へ採用するには、flacencの改修版をソース管理して継続保守する方法と、Rust 1.88以上への変更を承認する必要がある。公式flacencの互換対応を待つ場合は、上流の公開バージョンと依存条件を再確認する。

## 一次資料

- [RustSec RUSTSEC-2026-0575](https://rustsec.org/advisories/RUSTSEC-2026-0575.html)
- [heaplessの公告原文](https://github.com/rustsec/advisory-db/tree/main/crates/heapless)
- [flacenc 0.5.1の公式依存条件](https://crates.io/api/v1/crates/flacenc/0.5.1/dependencies)
- [heaplessの変更履歴](https://github.com/rust-embedded/heapless/blob/main/CHANGELOG.md)
