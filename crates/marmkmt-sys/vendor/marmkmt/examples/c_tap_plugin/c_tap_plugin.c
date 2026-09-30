#include "marmkmt.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/*
 * このファイルは、marmkmt の公開 C ABI だけを使う最小構成のプラグイン例である。
 * Margrete Plugin SDK の C++ ヘッダーを直接 include する必要はない。
 *
 * DLL の入口は次のように分担される。
 *
 *   - mg_plugin_init: この C ファイルが実装し、callback 一式を marmkmt へ登録する
 *   - MargretePluginGetInfo / MargretePluginCommandCreate:
 *     marmkmt.lib が実装し、c_tap_plugin.def から DLL export する
 *
 * C ABI を越える関数と callback には必ず MG_CALL を付けること。また、C++ 例外や
 * 他言語の unwind 可能な panic を ABI 境界の外へ出してはならない。
 */

/*
 * command ごとの状態を保持する型。今回は状態を必要としないが、create/destroy の
 * 対応と command の寿命を示すため、空ではない構造体を確保している。実用的な
 * プラグインでは、設定値など SDK ハンドル以外の永続データをここへ保持できる。
 */
typedef struct TapCommand
{
    int unused;
} TapCommand;

/*
 * Host API テーブルは mg_plugin_init の後も有効なので保持できる。一方、MgContext、
 * MgDocument、MgChart などのハンドルをここへ保存してはいけない。これらのハンドルは
 * invocation とスレッドに紐づき、invoke が戻るまでに解放する必要がある。
 */
static const MgHostApi *g_host;

/*
 * marmkmt の文字列 callback は、必要量の照会と実データ取得の 2 段階で呼ばれる。
 * required と capacity はどちらも終端 NUL を含む byte 数であり、文字数ではない。
 */
static MgResult copy_string(const char *source, char *buffer, uint32_t capacity, uint32_t *required)
{
    size_t length;

    if (source == NULL || required == NULL)
        return MG_ERR_NULL_POINTER;

    length = strlen(source) + 1;
    if (length > UINT32_MAX)
        return MG_ERR_INVALID_ARGUMENT;

    /* 容量不足の場合でも、呼び出し側が再確保できるよう必要量は必ず返す。 */
    *required = (uint32_t)length;

    /* buffer == NULL かつ capacity == 0 はエラーではなくサイズ照会である。 */
    if (buffer == NULL && capacity == 0)
        return MG_OK;
    if (buffer == NULL)
        return MG_ERR_NULL_POINTER;
    if (capacity < length)
        return MG_ERR_BUFFER_TOO_SMALL;

    memcpy(buffer, source, length);
    return MG_OK;
}

static MgResult MG_CALL get_plugin_info(void *plugin_data, MgPluginInfoBuffers *buffers)
{
    MgResult result;
    (void)plugin_data;

    if (buffers == NULL)
        return MG_ERR_NULL_POINTER;
    /* 受信した構造体が、この実装で参照する末尾フィールドまで存在するか確認する。 */
    if (buffers->struct_size < sizeof(*buffers))
        return MG_ERR_INVALID_ARGUMENT;

    /* 3 つの文字列はすべて UTF-8。copy_string が照会呼び出しにも対応する。 */
    result = copy_string("C Tap Example", buffers->name, buffers->name_capacity, buffers->name_required);
    if (result != MG_OK)
        return result;
    result = copy_string("Adds a tap note at the current tick.", buffers->description, buffers->description_capacity,
                         buffers->description_required);
    if (result != MG_OK)
        return result;
    return copy_string("marmkmt examples", buffers->developer, buffers->developer_capacity,
                       buffers->developer_required);
}

static MgResult MG_CALL create_command(void *plugin_data, void **out_command)
{
    TapCommand *command;
    (void)plugin_data;

    if (out_command == NULL)
        return MG_ERR_NULL_POINTER;
    /* 失敗時の out parameter は NULL のまま返す。 */
    *out_command = NULL;

    /* 返した不透明ポインターは、後で必ず destroy_command に渡される。 */
    command = (TapCommand *)calloc(1, sizeof(*command));
    if (command == NULL)
        return MG_ERR_INTERNAL;
    *out_command = command;
    return MG_OK;
}

static void MG_CALL destroy_command(void *plugin_data, void *command)
{
    (void)plugin_data;

    /* destroy callback は no-fail であり、エラーや例外を外へ出してはならない。 */
    free(command);
}

static MgResult MG_CALL get_command_name(void *plugin_data, void *command, char *buffer, uint32_t capacity,
                                         uint32_t *required)
{
    (void)plugin_data;
    if (command == NULL)
        return MG_ERR_NULL_POINTER;

    /* command 名にもプラグイン情報と同じ 2 段階の文字列規約が適用される。 */
    return copy_string("Add tap note", buffer, capacity, required);
}

/*
 * 現在の tick に TAP ノートを 1 個追加する。
 *
 * Host API から返る Context 以外のオブジェクトは所有ハンドルである。成功経路でも
 * 失敗経路でも object_release が必要なため、処理を単一の cleanup 節へ集約している。
 * append は note の所有権を移動しないので、追加後も note の release が必要である。
 */
static MgResult MG_CALL invoke(void *plugin_data, void *command, MgContext *context)
{
    MgDocument *document = NULL;
    MgChart *chart = NULL;
    MgUndoBuffer *undo = NULL;
    MgNote *note = NULL;
    MgNoteInfo info = {0};
    MgInt tick = 0;
    MgResult result;
    MgBool recording = MG_FALSE;
    (void)plugin_data;

    if (command == NULL || context == NULL)
        return MG_ERR_NULL_POINTER;

    /* Context は invoke 中だけ有効な借用ハンドルなので、解放してはならない。 */
    result = g_host->context_get_document(context, &document);
    if (result != MG_OK)
        goto cleanup;
    result = g_host->document_get_chart(document, &chart);
    if (result != MG_OK)
        goto cleanup;
    result = g_host->document_get_undo_buffer(document, &undo);
    if (result != MG_OK)
        goto cleanup;
    result = g_host->context_get_current_tick(context, &tick);
    if (result != MG_OK)
        goto cleanup;

    /*
     * 譜面の変更は Undo 記録で囲む。begin に成功した場合、invoke から戻る前に
     * commit または discard のどちらかを必ず呼ばなければならない。
     */
    result = g_host->undo_begin_recording(undo);
    if (result != MG_OK)
        goto cleanup;
    recording = MG_TRUE;

    result = g_host->chart_create_note(chart, &note);
    if (result != MG_OK)
        goto cleanup;

    /*
     * MgNoteInfo は将来フィールドが増えた場合にも既定値が安全になるようゼロ初期化済み。
     * この例では中央付近 (x=6) に幅 4 の通常 TAP を現在位置へ配置する。
     */
    info.type = MG_NOTE_TYPE_TAP;
    info.x = 6;
    info.width = 4;
    info.tick = tick;
    result = g_host->note_set_info(note, &info);
    if (result != MG_OK)
        goto cleanup;
    /* append 後も note は所有ハンドルのままなので cleanup で release する。 */
    result = g_host->chart_append_note(chart, note);
    if (result != MG_OK)
        goto cleanup;

    result = g_host->undo_commit_recording(undo);
    if (result != MG_OK)
        goto cleanup;
    recording = MG_FALSE;

    /* Undo を確定した後、ホストへ表示内容の更新を依頼する。 */
    result = g_host->context_update(context);

cleanup:
    /* 途中で失敗した場合は、記録中の変更を Undo 履歴へ残さず破棄する。 */
    if (recording)
        (void)g_host->undo_discard_recording(undo);

    /*
     * cleanup 中の release エラーは元の処理結果を上書きしない。すべて同じ invoke
     * スレッド上で解放し、Context 自体は借用ハンドルなので対象に含めない。
     */
    if (note != NULL)
        (void)g_host->object_release(note);
    if (undo != NULL)
        (void)g_host->object_release(undo);
    if (chart != NULL)
        (void)g_host->object_release(chart);
    if (document != NULL)
        (void)g_host->object_release(document);
    return result;
}

MgResult MG_CALL mg_plugin_init(uint32_t requested_abi_version, const MgHostApi *host_api, MgPluginApi *out_plugin_api)
{
    /*
     * marmkmt はプラグイン情報または command が初めて必要になった時に、この関数を
     * スレッドセーフに一度だけ呼ぶ。DllMain から呼ばれることはない。
     */
    if (host_api == NULL || out_plugin_api == NULL)
        return MG_ERR_NULL_POINTER;

    /* ABI の解釈違いを避けるため、要求版と Host API 自身の版を両方確認する。 */
    if (requested_abi_version != MG_ABI_VERSION || host_api->abi_version != MG_ABI_VERSION)
        return MG_ERR_UNSUPPORTED_ABI;

    /* この実装が利用する Host API 全フィールドが存在することを確認する。 */
    if (host_api->struct_size < sizeof(*host_api))
        return MG_ERR_INVALID_ARGUMENT;

    g_host = host_api;

    /*
     * out_plugin_api は marmkmt によって事前にゼロ初期化されている。このプラグインが
     * 実装する ABI バージョン、任意状態、必須の 5 callback を明示的に登録する。
     * plugin_data は各 callback の第 1 引数へそのまま渡されるが、この例では不要である。
     */
    out_plugin_api->struct_size = sizeof(*out_plugin_api);
    out_plugin_api->abi_version = MG_ABI_VERSION;
    out_plugin_api->plugin_data = NULL;
    out_plugin_api->get_plugin_info = get_plugin_info;
    out_plugin_api->create_command = create_command;
    out_plugin_api->destroy_command = destroy_command;
    out_plugin_api->get_command_name = get_command_name;
    out_plugin_api->invoke = invoke;
    return MG_OK;
}
