# 開発者ガイド

## 構成

- `src/`: Tauri WebViewで動作するTypeScript UI
- `src-tauri/src/`: Rustバックエンド、ドメイン、保存、外部プロセス連携
- `tests/fixtures/`: 契約試験・回帰試験用の非機密fixture
- `dev/`: 要求、実装計画、進捗、運用資料

## 起動

```powershell
npm install
npm run tauri dev
```

## 検証

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

