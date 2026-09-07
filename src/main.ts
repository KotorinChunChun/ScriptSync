import { invoke } from "@tauri-apps/api/core";
import "./style.css";

const app = document.querySelector<HTMLElement>("#app");

if (!app) {
  throw new Error("アプリケーションの表示先が見つかりません。");
}

app.innerHTML = `
  <section class="shell">
    <header>
      <p class="eyebrow">ScriptSync v0.0.1</p>
      <h1>読みあわせ</h1>
      <p>人間の確認済み音声を基準に、合成音声の誤読候補を局所的に見つけます。</p>
    </header>
    <section class="panel" aria-labelledby="project-status-title">
      <h2 id="project-status-title">開発基盤</h2>
      <p id="backend-status" role="status">バックエンドを確認しています…</p>
      <p>現在は初期構成のみです。音声取込・照合・補正画面は後続フェーズで実装します。</p>
    </section>
  </section>
`;

const status = document.querySelector<HTMLElement>("#backend-status");

if ("__TAURI_INTERNALS__" in window) {
  void invoke<string>("health_check")
    .then((message) => {
      if (status) status.textContent = message;
    })
    .catch((error: unknown) => {
      if (status) status.textContent = `バックエンド接続失敗: ${String(error)}`;
    });
} else if (status) {
  status.textContent = "ブラウザプレビューで表示しています。";
}
