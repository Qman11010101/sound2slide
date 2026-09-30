/**
 * @file marmkmt.h
 * @brief Margrete Plugin SDK v2 を安定した C ABI から利用するための公開 API。
 *
 * marmkmt は x64 Windows 向けの static library である。プラグイン DLL は本
 * ヘッダーで宣言する mg_plugin_init() を実装し、marmkmt をリンクする。
 *
 * @par ハンドルの寿命
 * MgContext は MgPluginApi::invoke の実行中だけ有効な借用ハンドルであり、解放
 * してはならない。それ以外の API から返されるハンドルは所有ハンドルであり、
 * invoke が戻る前に MgHostApi::object_release で解放する必要がある。append や
 * delete を呼んでも、呼び出し側が持つ参照の解放責任は移動しない。
 *
 * @par スレッド
 * すべてのハンドル操作は MgPluginApi::invoke が呼ばれたスレッドで行う。
 * ハンドルを別スレッドへ渡してはならない。
 *
 * @par 文字列
 * C ABI の文字列は検証済み UTF-8 である。長さと容量は終端 NUL を含む byte
 * 数で表す。出力文字列は、buffer を NULL、capacity を 0 にして必要量だけを
 * 問い合わせることができる。
 *
 * @copyright Copyright (c) 2026 Kjuman Enobikto
 * @par License
 * MIT License
 */
#ifndef MARMKMT_H
#define MARMKMT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C"
{
#endif

#if defined(_WIN32)
#define MG_CALL __cdecl /**< C ABI の呼び出し規約。 */
#else
#define MG_CALL
#endif

/** marmkmt C ABI のバージョン。 */
#define MG_ABI_VERSION 1u
/** 対応する Margrete Plugin SDK のバージョン。 */
#define MG_MARGRETE_SDK_VERSION 2u
/** ビルドに使用した Margrete Plugin SDK のコミット。 */
#define MG_MARGRETE_SDK_COMMIT "b8d0c87125e090880a75f6f0ff57a773510299a8"

    typedef int32_t MgResult;    /**< API の成否を表す結果コード。 */
    typedef int32_t MgBool;      /**< 0 が偽、非 0 が真の真偽値。 */
    typedef int32_t MgInt;       /**< ABI 固定幅の符号付き整数。 */
    typedef int32_t MgEventKind; /**< MG_EVENT_KIND_* のいずれか。 */

#define MG_FALSE 0
#define MG_TRUE 1

/** @name 結果コード
 * @{ */
#define MG_OK 0                      /**< 成功。 */
#define MG_ERR_NULL_POINTER -1       /**< 必須ポインターが NULL。 */
#define MG_ERR_INVALID_ARGUMENT -2   /**< 値、範囲、型などが不正。 */
#define MG_ERR_INVALID_HANDLE -3     /**< ハンドルが無効、解放済み、または型が不一致。 */
#define MG_ERR_WRONG_THREAD -4       /**< ハンドルを生成元以外のスレッドで使用した。 */
#define MG_ERR_BUFFER_TOO_SMALL -5   /**< 出力バッファーの容量が不足。 */
#define MG_ERR_UNSUPPORTED_ABI -6    /**< 要求された ABI バージョンに未対応。 */
#define MG_ERR_PLUGIN_INIT_FAILED -7 /**< プラグインの初期化に失敗。 */
#define MG_ERR_SDK_FAILURE -8        /**< Margrete SDK の操作に失敗。 */
#define MG_ERR_PANIC -9              /**< 言語ランタイムの例外または panic を捕捉。 */
#define MG_ERR_INTERNAL -10          /**< marmkmt 内部の予期しない失敗。 */
#define MG_ERR_COMMAND_POISONED -11  /**< 以前 panic した command は再実行不能。 */
#define MG_ERR_NOT_FOUND -12         /**< 検索対象が存在しない。 */
/** @} */

/** @name ノート種別
 * MgNoteInfo::type に指定する。
 * @{ */
#define MG_NOTE_TYPE_UNKNOWN 0
#define MG_NOTE_TYPE_TAP 1
#define MG_NOTE_TYPE_EXTAP 2
#define MG_NOTE_TYPE_FLICK 3
#define MG_NOTE_TYPE_DAMAGE 4
#define MG_NOTE_TYPE_HOLD 5
#define MG_NOTE_TYPE_SLIDE 6
#define MG_NOTE_TYPE_AIR 7
#define MG_NOTE_TYPE_AIRHOLD 8
#define MG_NOTE_TYPE_AIRSLIDE 9
#define MG_NOTE_TYPE_AIRCRUSH 10
#define MG_NOTE_TYPE_CLICK 11
/** @} */

/** @name ロングノート属性
 * MgNoteInfo::long_attr に指定する。
 * @{ */
#define MG_NOTE_LONG_ATTR_NONE 0
#define MG_NOTE_LONG_ATTR_BEGIN 1
#define MG_NOTE_LONG_ATTR_STEP 2
#define MG_NOTE_LONG_ATTR_CONTROL 3
#define MG_NOTE_LONG_ATTR_CURVE_CONTROL 4
#define MG_NOTE_LONG_ATTR_END 5
#define MG_NOTE_LONG_ATTR_END_NOACT 6
/** @} */

/** @name ノート方向
 * MgNoteInfo::direction に指定する。
 * @{ */
#define MG_NOTE_DIRECTION_NONE 0
#define MG_NOTE_DIRECTION_AUTO 1
#define MG_NOTE_DIRECTION_UP 2
#define MG_NOTE_DIRECTION_DOWN 3
#define MG_NOTE_DIRECTION_CENTER 4
#define MG_NOTE_DIRECTION_LEFT 5
#define MG_NOTE_DIRECTION_RIGHT 6
#define MG_NOTE_DIRECTION_UP_LEFT 7
#define MG_NOTE_DIRECTION_UP_RIGHT 8
#define MG_NOTE_DIRECTION_DOWN_LEFT 9
#define MG_NOTE_DIRECTION_DOWN_RIGHT 10
#define MG_NOTE_DIRECTION_ROTATE_LEFT 11
#define MG_NOTE_DIRECTION_ROTATE_RIGHT 12
#define MG_NOTE_DIRECTION_IN_OUT 13
#define MG_NOTE_DIRECTION_OUT_IN 14
/** @} */

/** @name ノート拡張属性
 * MgNoteInfo::ex_attr に指定する。
 * @{ */
#define MG_NOTE_EX_ATTR_NONE 0
#define MG_NOTE_EX_ATTR_INVERT 1
#define MG_NOTE_EX_ATTR_HAS_NOTE 2
#define MG_NOTE_EX_ATTR_EXJDG 3
/** @} */

/** @name AirCrush オプション値
 * MgNoteInfo::option_value に指定する。
 * @{ */
#define MG_OPTION_AIRCRUSH_TRACE_LIKE 0
#define MG_OPTION_AIRCRUSH_HEAD_ONLY INT32_MAX
/** @} */

/** @name イベント種別
 * MgEventKind および chart_create_event に指定する。
 * @{ */
#define MG_EVENT_KIND_TIMELINE_SPEED 1
#define MG_EVENT_KIND_NOTE_SPEED_MODIFIER 2
#define MG_EVENT_KIND_BPM 3
#define MG_EVENT_KIND_BEAT_CHANGE 4
    /** @} */

    typedef struct MgContext MgContext;       /**< command 呼び出しコンテキスト。 */
    typedef struct MgDocument MgDocument;     /**< 編集中のドキュメント。 */
    typedef struct MgChart MgChart;           /**< 譜面。 */
    typedef struct MgNote MgNote;             /**< ノート。 */
    typedef struct MgEvent MgEvent;           /**< 速度、BPM、拍子などのイベント。 */
    typedef struct MgUndoBuffer MgUndoBuffer; /**< Undo/Redo バッファー。 */
    typedef void *MgNativeWindowHandle;       /**< ネイティブウィンドウハンドル（Windows では HWND）。 */

    /** ノートの値データ。 */
    typedef struct MgNoteInfo
    {
        MgInt type;         /**< MG_NOTE_TYPE_*。 */
        MgInt long_attr;    /**< MG_NOTE_LONG_ATTR_*。 */
        MgInt direction;    /**< MG_NOTE_DIRECTION_*。 */
        MgInt ex_attr;      /**< MG_NOTE_EX_ATTR_*。 */
        MgInt variation_id; /**< バリエーション ID。 */
        MgInt x;            /**< 水平方向の位置。 */
        MgInt width;        /**< ノートの幅。 */
        MgInt height;       /**< ノートの高さ。 */
        MgInt tick;         /**< 時間位置（tick）。 */
        MgInt timeline_id;  /**< 所属するタイムライン ID。 */
        MgInt option_value; /**< ノート種別固有のオプション値。 */
    } MgNoteInfo;

    /** タイムライン速度イベントの値データ。 */
    typedef struct MgEventTimelineSpeedInfo
    {
        MgInt timeline_id; /**< 対象タイムライン ID。 */
        MgInt tick;        /**< 時間位置（tick）。 */
        double speed;      /**< 速度。有限値でなければならない。 */
    } MgEventTimelineSpeedInfo;

    /** ノート速度補正イベントの値データ。 */
    typedef struct MgEventNoteSpeedModifierInfo
    {
        MgInt tick;   /**< 時間位置（tick）。 */
        double speed; /**< 速度補正値。有限値でなければならない。 */
    } MgEventNoteSpeedModifierInfo;

    /** BPM イベントの値データ。 */
    typedef struct MgEventBpmInfo
    {
        MgInt tick; /**< 時間位置（tick）。 */
        double bpm; /**< BPM。有限値でなければならない。 */
    } MgEventBpmInfo;

    /** 拍子変更イベントの値データ。 */
    typedef struct MgEventBeatChangeInfo
    {
        MgInt bar;           /**< 小節番号。 */
        MgInt beats_per_bar; /**< 1 小節あたりの拍数（分子）。 */
        MgInt beat_unit;     /**< 拍の単位（分母）。 */
    } MgEventBeatChangeInfo;

    /**
     * プラグイン情報を書き込む UTF-8 バッファー群。
     *
     * 各 required には終端 NUL を含む必要 byte 数を書き込む。対応する buffer が
     * NULL かつ capacity が 0 の場合はサイズ照会として扱う。
     */
    typedef struct MgPluginInfoBuffers
    {
        uint32_t struct_size;           /**< 呼び出し側が確保した本構造体の byte 数。 */
        char *name;                     /**< プラグイン名の出力先。 */
        uint32_t name_capacity;         /**< name の容量（byte）。 */
        uint32_t *name_required;        /**< プラグイン名に必要な容量の出力先。 */
        char *description;              /**< 説明の出力先。 */
        uint32_t description_capacity;  /**< description の容量（byte）。 */
        uint32_t *description_required; /**< 説明に必要な容量の出力先。 */
        char *developer;                /**< 開発者名の出力先。 */
        uint32_t developer_capacity;    /**< developer の容量（byte）。 */
        uint32_t *developer_required;   /**< 開発者名に必要な容量の出力先。 */
    } MgPluginInfoBuffers;

    /**
     * marmkmt がプラグインへ提供する Host API テーブル。
     *
     * すべての関数は MG_OK または MG_ERR_* を返す。out 引数は必須であり、失敗時
     * には object out は NULL、scalar out は 0 に初期化される。返された所有
     * ハンドルは object_release で解放する。
     */
    typedef struct MgHostApi
    {
        uint32_t struct_size; /**< 利用可能なテーブル全体の byte 数。 */
        uint32_t abi_version; /**< テーブルが実装する ABI バージョン。 */
        /** 所有ハンドルの参照数を 1 増やす。 */
        MgResult(MG_CALL *object_add_ref)(void *object);
        /** 所有ハンドルの参照数を 1 減らす。MgContext には使用できない。 */
        MgResult(MG_CALL *object_release)(void *object);
        /** length byte の UTF-8 エラーメッセージをホストのダイアログに表示する。 */
        MgResult(MG_CALL *report_error)(MgContext *, const char *, uint32_t);
        /** コンテキストからドキュメントの所有ハンドルを取得する。 */
        MgResult(MG_CALL *context_get_document)(MgContext *, MgDocument **);
        /** ホストのメインウィンドウハンドルを取得する。 */
        MgResult(MG_CALL *context_get_main_window_handle)(MgContext *, MgNativeWindowHandle *);
        /** 現在の再生・編集位置を tick 単位で取得する。 */
        MgResult(MG_CALL *context_get_current_tick)(MgContext *, MgInt *);
        /** 変更内容をホストの表示へ反映する。 */
        MgResult(MG_CALL *context_update)(MgContext *);
        /** ドキュメントから譜面の所有ハンドルを取得する。 */
        MgResult(MG_CALL *document_get_chart)(MgDocument *, MgChart **);
        /** ドキュメントから Undo バッファーの所有ハンドルを取得する。 */
        MgResult(MG_CALL *document_get_undo_buffer)(MgDocument *, MgUndoBuffer **);
        /** Undo 記録を開始する。記録のネストはできない。 */
        MgResult(MG_CALL *undo_begin_recording)(MgUndoBuffer *);
        /** 記録中の変更を確定して Undo 履歴へ追加する。 */
        MgResult(MG_CALL *undo_commit_recording)(MgUndoBuffer *);
        /** 記録中の変更を破棄する。 */
        MgResult(MG_CALL *undo_discard_recording)(MgUndoBuffer *);
        /** 直前の変更を取り消す。 */
        MgResult(MG_CALL *undo_undo)(MgUndoBuffer *);
        /** 取り消した変更をやり直す。 */
        MgResult(MG_CALL *undo_redo)(MgUndoBuffer *);
        /** Undo 可能かを返す。 */
        MgResult(MG_CALL *undo_can_undo)(MgUndoBuffer *, MgBool *);
        /** Redo 可能かを返す。 */
        MgResult(MG_CALL *undo_can_redo)(MgUndoBuffer *, MgBool *);
        /** Undo 記録中かを返す。 */
        MgResult(MG_CALL *undo_is_recording)(MgUndoBuffer *, MgBool *);
        /** 未追加のノートを生成し、その所有ハンドルを返す。 */
        MgResult(MG_CALL *chart_create_note)(MgChart *, MgNote **);
        /** 譜面直下のノート数を取得する。 */
        MgResult(MG_CALL *chart_get_notes_count)(MgChart *, MgInt *);
        /** 0 始まりの index でノートの所有ハンドルを取得する。 */
        MgResult(MG_CALL *chart_get_note)(MgChart *, MgInt, MgNote **);
        /** ノートを譜面へ追加する。ハンドルの所有権は移動しない。 */
        MgResult(MG_CALL *chart_append_note)(MgChart *, MgNote *);
        /** ノートを譜面から削除する。成功後は対象ハンドルを release だけに使用できる。 */
        MgResult(MG_CALL *chart_delete_note)(MgChart *, MgNote *);
        /** 譜面内の全ノートを指定 tick 数だけ移動する。 */
        MgResult(MG_CALL *chart_offset_notes)(MgChart *, MgInt);
        /** ノート固有の ID を取得する。 */
        MgResult(MG_CALL *note_get_id)(MgNote *, MgInt *);
        /** ノートの値データを取得する。 */
        MgResult(MG_CALL *note_get_info)(MgNote *, MgNoteInfo *);
        /** ノートの値データを設定する。列挙値の範囲外は拒否される。 */
        MgResult(MG_CALL *note_set_info)(MgNote *, const MgNoteInfo *);
        /** 子ノート数を取得する。 */
        MgResult(MG_CALL *note_get_children_count)(MgNote *, MgInt *);
        /** 0 始まりの index で子ノートの所有ハンドルを取得する。 */
        MgResult(MG_CALL *note_get_child)(MgNote *, MgInt, MgNote **);
        /** 親ノートの所有ハンドルを取得する。親がなければ MG_ERR_NOT_FOUND。 */
        MgResult(MG_CALL *note_get_parent)(MgNote *, MgNote **);
        /** 子ノートを追加する。ハンドルの所有権は移動しない。 */
        MgResult(MG_CALL *note_append_child)(MgNote *, MgNote *);
        /** 子ノートを削除する。成功後は子ハンドルを release だけに使用できる。 */
        MgResult(MG_CALL *note_delete_child)(MgNote *, MgNote *);
        /** ノートを複製し、複製の所有ハンドルを返す。 */
        MgResult(MG_CALL *note_clone)(MgNote *, MgNote **);
        /** 第 1 引数の内容を第 2 引数で置換する。第 3 引数は子の再整列指定。 */
        MgResult(MG_CALL *note_replace_with)(MgNote *, const MgNote *, MgBool);
        /** 第 1 引数の値データを第 2 引数へコピーする。 */
        MgResult(MG_CALL *note_copy_info_to)(MgNote *, MgNote *);
        /** 長押し系列の基点ノートを取得する。なければ MG_ERR_NOT_FOUND。 */
        MgResult(MG_CALL *note_get_base_note)(MgNote *, MgNote **);
        /** すべての子ノートを指定 tick 数だけ移動する。 */
        MgResult(MG_CALL *note_offset_child)(MgNote *, MgInt);
        /** ノートを水平反転する。第 2 引数が真なら子にも再帰適用する。 */
        MgResult(MG_CALL *note_flip_h)(MgNote *, MgBool);
        /** 指定した MG_EVENT_KIND_* の未追加イベントを生成する。 */
        MgResult(MG_CALL *chart_create_event)(MgChart *, MgEventKind, MgEvent **);
        /** イベントを譜面へ追加する。ハンドルの所有権は移動しない。 */
        MgResult(MG_CALL *chart_append_event)(MgChart *, MgEvent *);
        /** イベントを削除する。成功後は対象ハンドルを release だけに使用できる。 */
        MgResult(MG_CALL *chart_delete_event)(MgChart *, MgEvent *);
        /** tick と timeline_id に一致するタイムライン速度イベントを取得する。 */
        MgResult(MG_CALL *chart_find_event_timeline_speed)(MgChart *, MgInt, MgInt, MgEvent **);
        /** tick に一致するノート速度補正イベントを取得する。 */
        MgResult(MG_CALL *chart_find_event_note_speed_modifier)(MgChart *, MgInt, MgEvent **);
        /** tick に一致する BPM イベントを取得する。 */
        MgResult(MG_CALL *chart_find_event_bpm)(MgChart *, MgInt, MgEvent **);
        /** bar に一致する拍子変更イベントを取得する。 */
        MgResult(MG_CALL *chart_find_event_beat_change)(MgChart *, MgInt, MgEvent **);
        /** イベントの MG_EVENT_KIND_* を取得する。 */
        MgResult(MG_CALL *event_get_kind)(MgEvent *, MgEventKind *);
        /** イベント固有の ID を取得する。 */
        MgResult(MG_CALL *event_get_id)(MgEvent *, MgInt *);
        /** タイムライン速度イベントの値データを取得する。 */
        MgResult(MG_CALL *event_timeline_speed_get_info)(MgEvent *, MgEventTimelineSpeedInfo *);
        /** タイムライン速度イベントの値データを設定する。 */
        MgResult(MG_CALL *event_timeline_speed_set_info)(MgEvent *, const MgEventTimelineSpeedInfo *);
        /** ノート速度補正イベントの値データを取得する。 */
        MgResult(MG_CALL *event_note_speed_modifier_get_info)(MgEvent *, MgEventNoteSpeedModifierInfo *);
        /** ノート速度補正イベントの値データを設定する。 */
        MgResult(MG_CALL *event_note_speed_modifier_set_info)(MgEvent *, const MgEventNoteSpeedModifierInfo *);
        /** BPM イベントの値データを取得する。 */
        MgResult(MG_CALL *event_bpm_get_info)(MgEvent *, MgEventBpmInfo *);
        /** BPM イベントの値データを設定する。 */
        MgResult(MG_CALL *event_bpm_set_info)(MgEvent *, const MgEventBpmInfo *);
        /** 拍子変更イベントの値データを取得する。 */
        MgResult(MG_CALL *event_beat_change_get_info)(MgEvent *, MgEventBeatChangeInfo *);
        /** 拍子変更イベントの値データを設定する。 */
        MgResult(MG_CALL *event_beat_change_set_info)(MgEvent *, const MgEventBeatChangeInfo *);
        /** 同じ kind の第 2 引数の内容で第 1 引数を置換する。 */
        MgResult(MG_CALL *event_replace_with)(MgEvent *, const MgEvent *);
        /** 同じ kind の第 1 引数の値データを第 2 引数へコピーする。 */
        MgResult(MG_CALL *event_copy_info_to)(MgEvent *, MgEvent *);
    } MgHostApi;

    /**
     * プラグインが mg_plugin_init() で marmkmt へ返す callback テーブル。
     *
     * すべての callback は必須である。callback から例外や unwind 可能な panic を
     * C ABI 境界の外へ出してはならない。文字列 callback は MgPluginInfoBuffers と
     * 同じサイズ照会規約を使用する。
     */
    typedef struct MgPluginApi
    {
        uint32_t struct_size; /**< プラグインが理解する本構造体の byte 数。 */
        uint32_t abi_version; /**< プラグインが実装する ABI バージョン。 */
        void *plugin_data;    /**< 全 callback へ渡される任意のプラグイン状態。 */
        /** プラグイン名、説明、開発者名を UTF-8 で書き込む。 */
        MgResult(MG_CALL *get_plugin_info)(void *, MgPluginInfoBuffers *);
        /** command インスタンスを生成して out_command へ書き込む。 */
        MgResult(MG_CALL *create_command)(void *, void **);
        /** create_command が生成した command を破棄する。失敗してはならない。 */
        void(MG_CALL *destroy_command)(void *, void *);
        /** command 名を UTF-8 で書き込み、out_required に必要 byte 数を返す。 */
        MgResult(MG_CALL *get_command_name)(void *, void *, char *, uint32_t, uint32_t *);
        /** command を実行する。context は callback が戻るまで有効な借用ハンドル。 */
        MgResult(MG_CALL *invoke)(void *, void *, MgContext *);
    } MgPluginApi;

    /**
     * marmkmt へプラグイン callback を登録する。
     *
     * marmkmt が最初にプラグイン情報または command を必要としたとき、スレッド
     * セーフに一度だけ呼び出す。DllMain からは呼び出されない。out_plugin_api は
     * 呼び出し前にゼロ初期化されており、プラグインは struct_size、abi_version、
     * plugin_data、および全 callback を設定する。
     *
     * @param requested_abi_version marmkmt が要求する ABI バージョン。
     * @param host_api 呼び出し後も有効な Host API テーブル。保持してよい。
     * @param out_plugin_api プラグインが設定する callback テーブル。
     * @retval MG_OK 登録に成功した。
     * @retval MG_ERR_UNSUPPORTED_ABI 要求された ABI に対応していない。
     * @return その他の MG_ERR_*。初期化失敗として扱われる。
     */
    MgResult MG_CALL mg_plugin_init(uint32_t requested_abi_version, const MgHostApi *host_api,
                                    MgPluginApi *out_plugin_api);

#ifdef __cplusplus
}
#endif
#endif
