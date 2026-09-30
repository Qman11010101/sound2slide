//! marmkmt ABI v1 の低水準バインディングです。
//!
//! コミット 5a802178fec637f578bdcfc8105962ddb8413a66 時点の marmkmt.h から生成しています。
//! このクレートでは、所有権や値の妥当性を意図的に保証していません。

#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_void};

pub const MG_ABI_VERSION: u32 = 1;
pub const MG_MARGRETE_SDK_VERSION: u32 = 2;
pub const MG_MARGRETE_SDK_COMMIT: &str = "b8d0c87125e090880a75f6f0ff57a773510299a8";

pub type MgResult = i32;
pub type MgBool = i32;
pub type MgInt = i32;
pub type MgEventKind = i32;
pub type MgNativeWindowHandle = *mut c_void;

pub const MG_FALSE: MgBool = 0;
pub const MG_TRUE: MgBool = 1;
pub const MG_OK: MgResult = 0;
pub const MG_ERR_NULL_POINTER: MgResult = -1;
pub const MG_ERR_INVALID_ARGUMENT: MgResult = -2;
pub const MG_ERR_INVALID_HANDLE: MgResult = -3;
pub const MG_ERR_WRONG_THREAD: MgResult = -4;
pub const MG_ERR_BUFFER_TOO_SMALL: MgResult = -5;
pub const MG_ERR_UNSUPPORTED_ABI: MgResult = -6;
pub const MG_ERR_PLUGIN_INIT_FAILED: MgResult = -7;
pub const MG_ERR_SDK_FAILURE: MgResult = -8;
pub const MG_ERR_PANIC: MgResult = -9;
pub const MG_ERR_INTERNAL: MgResult = -10;
pub const MG_ERR_COMMAND_POISONED: MgResult = -11;
pub const MG_ERR_NOT_FOUND: MgResult = -12;

pub const MG_NOTE_TYPE_UNKNOWN: MgInt = 0;
pub const MG_NOTE_TYPE_TAP: MgInt = 1;
pub const MG_NOTE_TYPE_EXTAP: MgInt = 2;
pub const MG_NOTE_TYPE_FLICK: MgInt = 3;
pub const MG_NOTE_TYPE_DAMAGE: MgInt = 4;
pub const MG_NOTE_TYPE_HOLD: MgInt = 5;
pub const MG_NOTE_TYPE_SLIDE: MgInt = 6;
pub const MG_NOTE_TYPE_AIR: MgInt = 7;
pub const MG_NOTE_TYPE_AIRHOLD: MgInt = 8;
pub const MG_NOTE_TYPE_AIRSLIDE: MgInt = 9;
pub const MG_NOTE_TYPE_AIRCRUSH: MgInt = 10;
pub const MG_NOTE_TYPE_CLICK: MgInt = 11;

pub const MG_NOTE_LONG_ATTR_NONE: MgInt = 0;
pub const MG_NOTE_LONG_ATTR_BEGIN: MgInt = 1;
pub const MG_NOTE_LONG_ATTR_STEP: MgInt = 2;
pub const MG_NOTE_LONG_ATTR_CONTROL: MgInt = 3;
pub const MG_NOTE_LONG_ATTR_CURVE_CONTROL: MgInt = 4;
pub const MG_NOTE_LONG_ATTR_END: MgInt = 5;
pub const MG_NOTE_LONG_ATTR_END_NOACT: MgInt = 6;

pub const MG_NOTE_DIRECTION_NONE: MgInt = 0;
pub const MG_NOTE_DIRECTION_AUTO: MgInt = 1;
pub const MG_NOTE_DIRECTION_UP: MgInt = 2;
pub const MG_NOTE_DIRECTION_DOWN: MgInt = 3;
pub const MG_NOTE_DIRECTION_CENTER: MgInt = 4;
pub const MG_NOTE_DIRECTION_LEFT: MgInt = 5;
pub const MG_NOTE_DIRECTION_RIGHT: MgInt = 6;
pub const MG_NOTE_DIRECTION_UP_LEFT: MgInt = 7;
pub const MG_NOTE_DIRECTION_UP_RIGHT: MgInt = 8;
pub const MG_NOTE_DIRECTION_DOWN_LEFT: MgInt = 9;
pub const MG_NOTE_DIRECTION_DOWN_RIGHT: MgInt = 10;
pub const MG_NOTE_DIRECTION_ROTATE_LEFT: MgInt = 11;
pub const MG_NOTE_DIRECTION_ROTATE_RIGHT: MgInt = 12;
pub const MG_NOTE_DIRECTION_IN_OUT: MgInt = 13;
pub const MG_NOTE_DIRECTION_OUT_IN: MgInt = 14;

pub const MG_NOTE_EX_ATTR_NONE: MgInt = 0;
pub const MG_NOTE_EX_ATTR_INVERT: MgInt = 1;
pub const MG_NOTE_EX_ATTR_HAS_NOTE: MgInt = 2;
pub const MG_NOTE_EX_ATTR_EXJDG: MgInt = 3;
pub const MG_OPTION_AIRCRUSH_TRACE_LIKE: MgInt = 0;
pub const MG_OPTION_AIRCRUSH_HEAD_ONLY: MgInt = i32::MAX;

pub const MG_EVENT_KIND_TIMELINE_SPEED: MgEventKind = 1;
pub const MG_EVENT_KIND_NOTE_SPEED_MODIFIER: MgEventKind = 2;
pub const MG_EVENT_KIND_BPM: MgEventKind = 3;
pub const MG_EVENT_KIND_BEAT_CHANGE: MgEventKind = 4;

macro_rules! opaque_handle {
    ($($name:ident),+ $(,)?) => {$(
        #[repr(C)]
        pub struct $name {
            _private: [u8; 0],
        }
    )+};
}

opaque_handle!(
    MgContext,
    MgDocument,
    MgChart,
    MgNote,
    MgEvent,
    MgUndoBuffer
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MgNoteInfo {
    pub r#type: MgInt,
    pub long_attr: MgInt,
    pub direction: MgInt,
    pub ex_attr: MgInt,
    pub variation_id: MgInt,
    pub x: MgInt,
    pub width: MgInt,
    pub height: MgInt,
    pub tick: MgInt,
    pub timeline_id: MgInt,
    pub option_value: MgInt,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MgEventTimelineSpeedInfo {
    pub timeline_id: MgInt,
    pub tick: MgInt,
    pub speed: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MgEventNoteSpeedModifierInfo {
    pub tick: MgInt,
    pub speed: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MgEventBpmInfo {
    pub tick: MgInt,
    pub bpm: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MgEventBeatChangeInfo {
    pub bar: MgInt,
    pub beats_per_bar: MgInt,
    pub beat_unit: MgInt,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MgPluginInfoBuffers {
    pub struct_size: u32,
    pub name: *mut c_char,
    pub name_capacity: u32,
    pub name_required: *mut u32,
    pub description: *mut c_char,
    pub description_capacity: u32,
    pub description_required: *mut u32,
    pub developer: *mut c_char,
    pub developer_capacity: u32,
    pub developer_required: *mut u32,
}

type Fn1<A> = Option<unsafe extern "C" fn(A) -> MgResult>;
type Fn2<A, B> = Option<unsafe extern "C" fn(A, B) -> MgResult>;
type Fn3<A, B, C> = Option<unsafe extern "C" fn(A, B, C) -> MgResult>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MgHostApi {
    pub struct_size: u32,
    pub abi_version: u32,
    pub object_add_ref: Fn1<*mut c_void>,
    pub object_release: Fn1<*mut c_void>,
    pub report_error: Fn3<*mut MgContext, *const c_char, u32>,
    pub context_get_document: Fn2<*mut MgContext, *mut *mut MgDocument>,
    pub context_get_main_window_handle: Fn2<*mut MgContext, *mut MgNativeWindowHandle>,
    pub context_get_current_tick: Fn2<*mut MgContext, *mut MgInt>,
    pub context_update: Fn1<*mut MgContext>,
    pub document_get_chart: Fn2<*mut MgDocument, *mut *mut MgChart>,
    pub document_get_undo_buffer: Fn2<*mut MgDocument, *mut *mut MgUndoBuffer>,
    pub undo_begin_recording: Fn1<*mut MgUndoBuffer>,
    pub undo_commit_recording: Fn1<*mut MgUndoBuffer>,
    pub undo_discard_recording: Fn1<*mut MgUndoBuffer>,
    pub undo_undo: Fn1<*mut MgUndoBuffer>,
    pub undo_redo: Fn1<*mut MgUndoBuffer>,
    pub undo_can_undo: Fn2<*mut MgUndoBuffer, *mut MgBool>,
    pub undo_can_redo: Fn2<*mut MgUndoBuffer, *mut MgBool>,
    pub undo_is_recording: Fn2<*mut MgUndoBuffer, *mut MgBool>,
    pub chart_create_note: Fn2<*mut MgChart, *mut *mut MgNote>,
    pub chart_get_notes_count: Fn2<*mut MgChart, *mut MgInt>,
    pub chart_get_note: Fn3<*mut MgChart, MgInt, *mut *mut MgNote>,
    pub chart_append_note: Fn2<*mut MgChart, *mut MgNote>,
    pub chart_delete_note: Fn2<*mut MgChart, *mut MgNote>,
    pub chart_offset_notes: Fn2<*mut MgChart, MgInt>,
    pub note_get_id: Fn2<*mut MgNote, *mut MgInt>,
    pub note_get_info: Fn2<*mut MgNote, *mut MgNoteInfo>,
    pub note_set_info: Fn2<*mut MgNote, *const MgNoteInfo>,
    pub note_get_children_count: Fn2<*mut MgNote, *mut MgInt>,
    pub note_get_child: Fn3<*mut MgNote, MgInt, *mut *mut MgNote>,
    pub note_get_parent: Fn2<*mut MgNote, *mut *mut MgNote>,
    pub note_append_child: Fn2<*mut MgNote, *mut MgNote>,
    pub note_delete_child: Fn2<*mut MgNote, *mut MgNote>,
    pub note_clone: Fn2<*mut MgNote, *mut *mut MgNote>,
    pub note_replace_with: Fn3<*mut MgNote, *const MgNote, MgBool>,
    pub note_copy_info_to: Fn2<*mut MgNote, *mut MgNote>,
    pub note_get_base_note: Fn2<*mut MgNote, *mut *mut MgNote>,
    pub note_offset_child: Fn2<*mut MgNote, MgInt>,
    pub note_flip_h: Fn2<*mut MgNote, MgBool>,
    pub chart_create_event: Fn3<*mut MgChart, MgEventKind, *mut *mut MgEvent>,
    pub chart_append_event: Fn2<*mut MgChart, *mut MgEvent>,
    pub chart_delete_event: Fn2<*mut MgChart, *mut MgEvent>,
    pub chart_find_event_timeline_speed:
        Option<unsafe extern "C" fn(*mut MgChart, MgInt, MgInt, *mut *mut MgEvent) -> MgResult>,
    pub chart_find_event_note_speed_modifier: Fn3<*mut MgChart, MgInt, *mut *mut MgEvent>,
    pub chart_find_event_bpm: Fn3<*mut MgChart, MgInt, *mut *mut MgEvent>,
    pub chart_find_event_beat_change: Fn3<*mut MgChart, MgInt, *mut *mut MgEvent>,
    pub event_get_kind: Fn2<*mut MgEvent, *mut MgEventKind>,
    pub event_get_id: Fn2<*mut MgEvent, *mut MgInt>,
    pub event_timeline_speed_get_info: Fn2<*mut MgEvent, *mut MgEventTimelineSpeedInfo>,
    pub event_timeline_speed_set_info: Fn2<*mut MgEvent, *const MgEventTimelineSpeedInfo>,
    pub event_note_speed_modifier_get_info: Fn2<*mut MgEvent, *mut MgEventNoteSpeedModifierInfo>,
    pub event_note_speed_modifier_set_info: Fn2<*mut MgEvent, *const MgEventNoteSpeedModifierInfo>,
    pub event_bpm_get_info: Fn2<*mut MgEvent, *mut MgEventBpmInfo>,
    pub event_bpm_set_info: Fn2<*mut MgEvent, *const MgEventBpmInfo>,
    pub event_beat_change_get_info: Fn2<*mut MgEvent, *mut MgEventBeatChangeInfo>,
    pub event_beat_change_set_info: Fn2<*mut MgEvent, *const MgEventBeatChangeInfo>,
    pub event_replace_with: Fn2<*mut MgEvent, *const MgEvent>,
    pub event_copy_info_to: Fn2<*mut MgEvent, *mut MgEvent>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MgPluginApi {
    pub struct_size: u32,
    pub abi_version: u32,
    pub plugin_data: *mut c_void,
    pub get_plugin_info:
        Option<unsafe extern "C" fn(*mut c_void, *mut MgPluginInfoBuffers) -> MgResult>,
    pub create_command: Option<unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> MgResult>,
    pub destroy_command: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub get_command_name: Option<
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_char, u32, *mut u32) -> MgResult,
    >,
    pub invoke: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut MgContext) -> MgResult>,
}

unsafe extern "C" {
    pub fn mg_plugin_init(
        requested_abi_version: u32,
        host_api: *const MgHostApi,
        out_plugin_api: *mut MgPluginApi,
    ) -> MgResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn value_layouts_match_abi_v1() {
        assert_eq!((size_of::<MgNoteInfo>(), align_of::<MgNoteInfo>()), (44, 4));
        assert_eq!(offset_of!(MgNoteInfo, option_value), 40);
        assert_eq!(
            (
                size_of::<MgEventTimelineSpeedInfo>(),
                align_of::<MgEventTimelineSpeedInfo>()
            ),
            (16, 8)
        );
        assert_eq!(offset_of!(MgEventTimelineSpeedInfo, speed), 8);
        assert_eq!(
            (
                size_of::<MgEventNoteSpeedModifierInfo>(),
                align_of::<MgEventNoteSpeedModifierInfo>()
            ),
            (16, 8)
        );
        assert_eq!(
            (size_of::<MgEventBpmInfo>(), align_of::<MgEventBpmInfo>()),
            (16, 8)
        );
        assert_eq!(
            (
                size_of::<MgEventBeatChangeInfo>(),
                align_of::<MgEventBeatChangeInfo>()
            ),
            (12, 4)
        );
    }

    #[test]
    fn api_table_layouts_match_abi_v1() {
        assert_eq!(
            (
                size_of::<MgPluginInfoBuffers>(),
                align_of::<MgPluginInfoBuffers>()
            ),
            (80, 8)
        );
        assert_eq!(offset_of!(MgPluginInfoBuffers, developer_required), 72);
        assert_eq!(
            (size_of::<MgPluginApi>(), align_of::<MgPluginApi>()),
            (56, 8)
        );
        assert_eq!(offset_of!(MgPluginApi, invoke), 48);
        assert_eq!((size_of::<MgHostApi>(), align_of::<MgHostApi>()), (456, 8));
        assert_eq!(offset_of!(MgHostApi, object_add_ref), 8);
        assert_eq!(offset_of!(MgHostApi, event_copy_info_to), 448);
    }
}
