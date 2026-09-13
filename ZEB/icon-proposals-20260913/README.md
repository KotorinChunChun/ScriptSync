# アイコン検討結果（2026-09-13）

02「開いた台本とマイク」を正式採用。01〜10の原案と比較図を検討資料として保存。
正式原本は ../../src-tauri/icons/app-icon.svg。プロジェクト書類用派生は project-icon.svg。

- アプリ・MSIX用: src-tauri/icons の PNG、ICO、ICNS。Tauri icon コマンドで原本から再生成。
- プロジェクト関連付け: project.ico / ProjectFileLogo.png。MSIXは後者を参照。
- Web用: public/icon.svg、favicon.ico、dev/release-page/icon.png、icon.svg。
- ストア掲載用: dev/release-page/store-icon-512.png（512px）。パッケージ用StoreLogo等はsrc-tauri/icons。
- 既存のAndroid/iOS素材も同じ原本で再生成。ストアへの提出・Webアップロードは今回行わない。

