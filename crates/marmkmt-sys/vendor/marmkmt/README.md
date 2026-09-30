# marmkmt

[English](./README_en.md)

[ドキュメント](./docs/README.md)

[C 言語プラグインのサンプル](./examples/README.md)

`marmkmt` は、Margrete Plugin SDK v2 の C++ ABI を、[`include/marmkmt.h`](include/marmkmt.h)
で定義された安定した C ABI に変換するライブラリです。ビルド成果物は単一の x64 Windows スタティックライブラリであり、
ブリッジ DLL は生成されません。

SDK ヘッダーは `third_party/MargretePluginSDK` Git サブモジュールとして提供されており、
コミット `b8d0c87125e090880a75f6f0ff57a773510299a8` に固定されています。

サブモジュールを含めてクローンしてください。

```powershell
git clone --recurse-submodules <repository-url>
```

既にクローン済みの場合は、以下のコマンドで SDK を初期化してください。

```powershell
git submodule update --init --recursive
```

## Visual Studio 2022 でのビルド

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
```

スタティックライブラリは `build/Release/marmkmt.lib` に生成されます。
最終的なプラグイン DLL はこのライブラリとリンクし、公開ヘッダーで宣言された `mg_plugin_init` 関数を
提供する必要があります。ラッパー内のコードがアーカイブ内のすべてのシンボルを参照するとは限らないため、
最終的な DLL リンクではアーカイブ全体を保持するか（MSVC: `/WHOLEARCHIVE:marmkmt.lib`）、
`MargretePluginGetInfo` と `MargretePluginCommandCreate` の両方を明示的にエクスポート/参照する必要があります。
