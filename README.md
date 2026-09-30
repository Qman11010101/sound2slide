# sound2slide

音声波形からスライドを生成する、Windows x64向けMargreteプラグインです。

## インストール

GitHub ReleasesからZIPをダウンロードして展開してください。
`sound2slide.dll` と `sound2slide.ini` をMargreteのプラグイン配置先に置き、
`third-party-library.txt` はライセンス通知として保管してください。

## ビルド

Rustのstableツールチェーン、Visual Studio 2022のC++ビルドツール、CMakeが必要です。
プロジェクトのルートで以下を実行してください。

```powershell
./scripts/test.ps1
./scripts/build.ps1
./scripts/package-release.ps1
```

`dist/sound2slide-windows-x64.zip` にDLL、INI、ライセンス通知を出力します。
ライセンス通知の生成時に、未導入なら `cargo-about` を自動インストールします。

marmkmtのC++ソース、Margrete Plugin SDK、marmkmt-rsのRustバインディングは
ソースツリーに同梱しています。サブモジュールの取得は不要です。
marmkmtは静的リンクされ、配布ZIPに別のライブラリDLLを追加する必要はありません。

## 開発

開発は `develop` で行い、PRで `main` にマージします。
Windows CIは整形、テスト、ビルド、ライセンス生成、ZIP作成を検証します。
`main` のCI成功後に、日本時間の `yyyy-mm-dd` をタグとするGitHub ReleaseへZIPを公開します。
同じ日の再リリースでは、同日のReleaseを1件のまま更新し、タグを `yyyy-mm-dd-2`、`yyyy-mm-dd-3` のように採番します。
ZIPとリリースノートを差し替え、以前のタグはソースの履歴として残します。
同じコミットのCI再試行では番号を増やしません。

## ライセンス

Copyright (c) 2026 Kjuman Enobikto

本体はMITライセンスです。全文は [LICENSE.md](LICENSE.md) を参照してください。
依存ライブラリと同梱フォントには個別のライセンスが適用されます。
配布物の `third-party-library.txt` にライセンス本文とソースの入手先を収録します。
