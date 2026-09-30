# C ABI リファレンス

## ABI と呼び出し規約

公開境界は C ABI です。Windows では C ABI 関数と callback に `MG_CALL` (`__cdecl`) を使用します。
Margrete 側の 2 つの export は SDK の規定どおり `extern "C"`、`__stdcall` で marmkmt が実装します。
C++ 型、例外、RTTI、テンプレート、標準ライブラリ型を C ABI 境界へ出してはいけません。

`MG_ABI_VERSION` は `1`、`MG_MARGRETE_SDK_VERSION` は `2` です。公開 API テーブル先頭の
`struct_size` は呼び出し側が確保した byte 数、`abi_version` は実装する ABI バージョンです。
ABI 1 のフィールド順序や意味は固定です。将来の追加はテーブル末尾で行います。

## 基本型と結果コード

`MgResult`、`MgBool`、`MgInt`、`MgEventKind` はすべて `int32_t` です。`MgBool` は入力では
0 を偽、非 0 を真として受け取り、出力では `MG_FALSE` または `MG_TRUE` に正規化されます。

| 値 | 意味 |
|---|---|
| `MG_OK` | 成功 |
| `MG_ERR_NULL_POINTER` | 必須ポインターが NULL |
| `MG_ERR_INVALID_ARGUMENT` | 値、範囲、型が不正 |
| `MG_ERR_INVALID_HANDLE` | 無効、解放済み、または型違いのハンドル |
| `MG_ERR_WRONG_THREAD` | 生成元とは異なるスレッドからの操作 |
| `MG_ERR_BUFFER_TOO_SMALL` | 文字列バッファー不足 |
| `MG_ERR_UNSUPPORTED_ABI` | 未対応の ABI version |
| `MG_ERR_PLUGIN_INIT_FAILED` | callback テーブルを含む初期化の失敗 |
| `MG_ERR_SDK_FAILURE` | Margrete SDK 操作の失敗 |
| `MG_ERR_PANIC` | ABI 境界で捕捉した例外または panic |
| `MG_ERR_INTERNAL` | marmkmt 内部の予期しない失敗 |
| `MG_ERR_COMMAND_POISONED` | 以前に例外または panic を起こした command |
| `MG_ERR_NOT_FOUND` | 親、基点、イベントなどの検索対象なし |

必須 out ポインターは NULL にできません。Host API は処理前に object out を NULL、scalar out を
0 に初期化し、失敗時に部分的な結果を返しません。

## 公開データ構造

- `MgNoteInfo`: type、long 属性、方向、拡張属性、座標、tick、timeline ID などのノート値
- `MgEventTimelineSpeedInfo`: timeline ID、tick、speed
- `MgEventNoteSpeedModifierInfo`: tick、speed
- `MgEventBpmInfo`: tick、BPM
- `MgEventBeatChangeInfo`: bar、beats per bar、beat unit
- `MgPluginInfoBuffers`: プラグイン情報を受け取る 3 組の UTF-8 バッファー

構造体には `#pragma pack(1)` を適用しません。x64 でのレイアウトは実装時の static assertion と
テストで検証されています。フィールド定義と `MG_NOTE_*` / `MG_EVENT_KIND_*` の値は
[`include/marmkmt.h`](../include/marmkmt.h) を参照してください。浮動小数点の speed と BPM は有限値だけを受け付けます。

## `MgPluginApi`

プラグインが `mg_plugin_init` で返す callback テーブルです。`plugin_data` は全 callback の
第 1 引数へそのまま渡されます。command は C ABI では不透明な `void *` です。

## `MgHostApi`

marmkmt がプラグインへ渡す関数テーブルです。

| 分類 | 関数プレフィックス / 主な操作 |
|---|---|
| 所有権・通知 | `object_add_ref`, `object_release`, `report_error` |
| context | document、main window、current tick の取得、`context_update` |
| document | chart、Undo buffer の取得 |
| Undo | begin、commit、discard、undo、redo、状態照会 |
| chart / note | 作成、列挙、追加、削除、値操作、親子操作、clone、反転 |
| chart / event | 4 種の event の作成、追加、削除、検索 |
| event | kind / ID / 値の取得と設定、replace、copy |

イベントは timeline speed、note speed modifier、BPM、beat change の 4 種です。型固有の API に
異なる kind のハンドルを渡すと `MG_ERR_INVALID_ARGUMENT` になります。全関数の正確な宣言は
[`MgHostApi`](../include/marmkmt.h) の定義を参照してください。

## Margrete SDK への対応

marmkmt は C ABI の不透明ハンドルを Margrete SDK v2 interface に変換し、SDK の参照カウントを
管理します。`queryInterface` や GUID は公開せず、利用可能な interface を型別 Host API と
`chart_create_event` / `event_get_kind` で表現します。

