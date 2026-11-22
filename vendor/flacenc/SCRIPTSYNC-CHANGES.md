# ScriptSyncでの互換修正

2026-11-22。crates.ioのflacenc 0.5.1を元に、heaplessを0.8.0から0.9.4へ更新した。取得元と公式アーカイブのSHA-256は[origin.json](origin.json)に記録する。上流のApache-2.0と著作権表示は[LICENSE](LICENSE)のまま維持する。

- Cargo.tomlとCargo.toml.origのheapless依存を `=0.9.4` に変更。
- src/component/datatype.rsのVecエラー処理2箇所を `map_err(|()| ...)` から `map_err(|_| ...)` へ変更。エラーメッセージと音声処理は維持。
- 上流パッケージのソース・build.rs・ライセンス・説明を同梱。開発用Cargo.lock、CI設定、レジストリーの取得済みマーカーは配布しない。

両Cargoルートのpatch指定で同じソースを使用する。heaplessの最低Rust版に合わせ、録音crateの最低Rust版は1.88とする。[隔離検証](../../dev/history/v0.0.8/heapless-security-review-20261122.md)と[採用・公開記録](../../dev/history/v0.0.6.6/v0.0.6.6-imp.md)を参照。

復旧時は公開版0.0.6.5の署名済みMSIXへ戻す。ソースの復旧では本変更コミットをrevertし、両Cargo.toml、両Cargo.lock、ライセンス生成物を一緒に戻す。保存済み原稿・音声の形式変更はない。
