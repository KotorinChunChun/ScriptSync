# 第三者ソフトウェア

ScriptSyncと、権利者本人が作成した録音基盤はMITです。依存パッケージ・効果音・第三者ソースに本体のMITを上書き適用しません。

## 配布版に含むもの

`licenses/third-party/manifest.json` に、Windows x64ビルドで使用するRust依存（ビルド用を含む）と本番npm依存の名前・版・元ライセンス・取得先・本文のハッシュを記録しています。`licenses/third-party/texts/` に上流のLICENSE/NOTICE等、`licenses/third-party/sources/` にMPL等の対応ソースの未変更crateアーカイブを同梱します。crateアーカイブのSHA-256はCargo.lockのchecksumと照合します。

Symphonia等のMPL-2.0部分は、アーカイブ内の元ソース・著作権表示・ライセンスを維持します。実行形式の受領者は、同梱アーカイブを展開して対応するソースを取得できます。取得元の公開URLもmanifestへ記載します。第三者部分について、元ライセンスが認める改変・再配布等の権利を本体の条件で制限しません。

4つのUI効果音はKenneyのInterface Sounds 1.0、CC0です。元の[表示](public/sounds/Kenney-LICENSE.txt)を維持し、生成した配布用文書にも含めます。

WebView2Loaderの静的ライブラリーはMicrosoft公式SDK 1.0.3650.58とSHA-256を照合し、SDKのLICENSE・NOTICEを `licenses/third-party/texts/Microsoft-WebView2Loader-1.0.3650.58/` に含めています。RustラッパーのMITとは別に元の表示を維持します。WebView2 Runtime本体は同梱しません。

MSIXにはルートLICENSE・この案内・生成したlicenses一式を含めます。ライセンス文書は、インストール先の `licenses` フォルダーにもあります。配布構成と文書の一致は `npm run licenses:check` で検査します。

## MP3エンコードを有効にした派生ビルド

ScriptSync本体の配布構成はWAV/FLAC出力です。録音crate単体の既定feature `mp3-encode` は従来のMP3出力を維持し、LGPL-3.0のmp3lame-encoder / mp3lame-sysを使用します。本体はこのfeatureを有効にしません。

MP3を有効にして別のバイナリーを配布する場合は、この標準MSIX用の文書だけで完了とせず、使用するライセンス本文、著作権表示、対応ソース、必要な再リンク方法などを用意してください。元の[録音基盤の依存一覧](crates/scriptsync-recorder/THIRD-PARTY-NOTICES.txt)は受領時の履歴資料です。現行MSIXの採用一覧には使いません。

## 再生成とソースのビルド

WindowsのMSVC・Rust・Node.jsを用意し、`npm ci --ignore-scripts` と `cargo fetch --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc` を実行してから `npm run licenses:update` で再生成します。`npm run licenses:check` は不足・変更・余分な生成ファイルとlockの不一致を拒否します。通常のソースビルド・署名付き配布の手順は[開発者ガイド](docs/DEVELOPERS_GUIDE.md)を参照してください。署名秘密鍵はソースに含めません。
