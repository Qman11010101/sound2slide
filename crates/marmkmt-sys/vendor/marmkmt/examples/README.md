# examples

## `c_tap_plugin`

marmkmt の C ABI だけを使って C11 で実装した最小プラグインです。現在の tick に幅 4 の
TAP ノートを追加し、操作を Undo 履歴へ記録します。

トップレベルから通常どおりビルドすると、次の DLL が生成されます。

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release --target marmkmt_c_tap_plugin
```

成果物は `build/examples/c_tap_plugin/Release/c_tap_plugin.dll` です。Margrete のプラグイン
ディレクトリ（Margrete 本体の `plugins`）へ配置して使用してください。Release DLL は
公式サンプルと同様に MSVC ランタイムを静的リンクするため、通常はこの DLL だけを配置できます。

サンプルの構成は次の 3 ファイルです。

- `c_tap_plugin.c`: `mg_plugin_init`、5 個の callback、ノート追加処理
- `c_tap_plugin.def`: Margrete が要求する 2 個の entry point の export
- `CMakeLists.txt`: marmkmt の静的リンク、`/WHOLEARCHIVE`、C++ linker の指定

サンプルをビルドしない場合は、configure 時に `-DMARMKMT_BUILD_EXAMPLES=OFF` を指定します。
