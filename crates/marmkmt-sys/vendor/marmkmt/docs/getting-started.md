# 導入・プラグイン実装ガイド

## ビルド

Visual Studio 2022 と CMake を使用します。SDK は Git サブモジュールです。

```powershell
git submodule update --init --recursive
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
```

成果物は `build/Release/marmkmt.lib` です。

## DLL への組み込み

プラグイン DLL は次をすべて含めます。

1. `marmkmt.lib` と静的リンクしたコード
2. 対象言語の C ABI ラッパーとプラグイン実装
3. `mg_plugin_init`
4. marmkmt が実装する Margrete export (`MargretePluginGetInfo` と `MargretePluginCommandCreate`)

MSVC では、スタティックライブラリが除去されないよう `/WHOLEARCHIVE:marmkmt.lib` を指定します。
または、2 つの Margrete export を最終 DLL から明示的に参照・公開してください。

## 初期化

marmkmt はプラグイン情報または command が最初に必要になった時点で、`mg_plugin_init` を
スレッドセーフに一度だけ呼びます。`DllMain` からは呼びません。

```c
#include "marmkmt.h"

static const MgHostApi *host;

MgResult MG_CALL mg_plugin_init(
    uint32_t requested_abi_version,
    const MgHostApi *host_api,
    MgPluginApi *out_plugin_api)
{
    if (!host_api || !out_plugin_api)
        return MG_ERR_NULL_POINTER;
    if (requested_abi_version != MG_ABI_VERSION)
        return MG_ERR_UNSUPPORTED_ABI;

    host = host_api;
    out_plugin_api->struct_size = sizeof(*out_plugin_api);
    out_plugin_api->abi_version = MG_ABI_VERSION;
    out_plugin_api->plugin_data = NULL;
    out_plugin_api->get_plugin_info = plugin_get_info;
    out_plugin_api->create_command = plugin_create_command;
    out_plugin_api->destroy_command = plugin_destroy_command;
    out_plugin_api->get_command_name = plugin_get_command_name;
    out_plugin_api->invoke = plugin_invoke;
    return MG_OK;
}
```

5 個の callback はすべて必須です。`out_plugin_api` は呼び出し前にゼロ初期化され、
`struct_size` と `abi_version` には marmkmt が理解する値が設定されています。プラグインは、
自身が返すテーブルの値としてこれらを明示的に設定してください。`host_api` は初期化後も有効で、
保持できます。

## callback の役割

| callback | 契約 |
|---|---|
| `get_plugin_info` | プラグイン名、説明、開発者名を UTF-8 で返す |
| `create_command` | command の不透明ポインターを生成する |
| `destroy_command` | command を破棄する。失敗してはならない |
| `get_command_name` | command 名を UTF-8 で返す |
| `invoke` | `MgContext` と Host API を使って command を実行する |

文字列 callback は最初に必要容量を照会され、次に実データを書き込む 2 段階呼び出しです。
詳細は [実行時規約](runtime-contract.md#文字列) を参照してください。

## command の実行

`invoke` 内では通常、次の順で処理します。

1. `context_get_document` でドキュメントを取得する。
2. `document_get_chart` と `document_get_undo_buffer` で編集対象を取得する。
3. `undo_begin_recording` を呼ぶ。
4. ノートまたはイベントを作成・編集する。
5. 成功時は `undo_commit_recording`、失敗時は `undo_discard_recording` を呼ぶ。
6. `context_update` で表示へ反映する。
7. 所有ハンドルを `object_release` で解放してから戻る。

`invoke` の戻り値が `MG_OK` 以外なら、marmkmt はエラーをダイアログ表示して Margrete へ失敗を返します。

