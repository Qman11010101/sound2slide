# marmkmt

[日本語](./README.md)

[Documentation (Japanese)](./docs/README.md)

[C plugin example](./examples/README.md)

`marmkmt` is a library that converts the Margrete Plugin SDK v2 C++ ABI into the stable C ABI
defined by [`include/marmkmt.h`](include/marmkmt.h). The build artifact is a single x64
Windows static library; no bridge DLL is generated.

The SDK headers are provided as a `third_party/MargretePluginSDK` Git
submodule, pinned to commit `b8d0c87125e090880a75f6f0ff57a773510299a8`.

Clone including submodules:

```powershell
git clone --recurse-submodules <repository-url>
```

If already cloned, initialize the SDK with:

```powershell
git submodule update --init --recursive
```

## Build with Visual Studio 2022

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
```

The static library is generated at `build/Release/marmkmt.lib`. A final plugin
DLL must link this library and provide the `mg_plugin_init` function declared
by the public header. Since code within the wrapper may not reference all symbols from the archive, the final DLL link must retain the archive as a whole
(MSVC: `/WHOLEARCHIVE:marmkmt.lib`) or explicitly export/reference both
`MargretePluginGetInfo` and `MargretePluginCommandCreate`.
