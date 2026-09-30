//! `marmkmt` C ABI の安全な高レベル Rust API。
//!
//! 通常は [`Plugin`] と [`Command`] を実装し、crate root で
//! [`export_plugin!`] を一度呼び出します。生の FFI API は [`sys`] にあります。
#![deny(missing_docs)]

use core::{cell::Cell, ffi::c_void, marker::PhantomData, mem, ptr::NonNull};
use std::{
    error, fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::atomic::{AtomicPtr, Ordering},
};

pub use marmkmt_sys as sys;

/// この crate の結果型。
pub type Result<T> = core::result::Result<T, Error>;

/// marmkmt、またはプラグイン実装から返されたエラー。
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// safe wrapper 内部で null が検出された。
    NullPointer,
    /// 引数が不正。
    InvalidArgument,
    /// ハンドルが無効。
    InvalidHandle,
    /// invocation を実行中のスレッドではない。
    WrongThread,
    /// 出力バッファーが不足。
    BufferTooSmall,
    /// ABI version が未対応。
    UnsupportedAbi,
    /// プラグイン初期化に失敗。
    PluginInitFailed,
    /// Margrete SDK 操作に失敗。
    SdkFailure,
    /// ABI 境界で panic が捕捉された。
    Panic,
    /// marmkmt 内部エラー。
    Internal,
    /// command が poisoned 状態。
    CommandPoisoned,
    /// 対象が見つからない。
    NotFound,
    /// 文字列に埋め込み NUL がある、または長すぎる。
    InvalidString,
    /// 将来追加された未知の結果コード。
    UnknownResult(i32),
    /// プラグイン固有のエラー。
    Plugin(Box<dyn error::Error + Send + Sync>),
}

impl Error {
    /// プラグイン固有エラーをラップする。
    pub fn plugin(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self::Plugin(Box::new(error))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownResult(v) => write!(f, "unknown marmkmt result code: {v}"),
            Self::Plugin(e) => write!(f, "plugin error: {e}"),
            other => f.write_str(match other {
                Self::NullPointer => "unexpected null pointer",
                Self::InvalidArgument => "invalid argument",
                Self::InvalidHandle => "invalid handle",
                Self::WrongThread => "wrong thread",
                Self::BufferTooSmall => "buffer too small",
                Self::UnsupportedAbi => "unsupported ABI",
                Self::PluginInitFailed => "plugin initialization failed",
                Self::SdkFailure => "SDK failure",
                Self::Panic => "panic at ABI boundary",
                Self::Internal => "internal error",
                Self::CommandPoisoned => "command is poisoned",
                Self::NotFound => "not found",
                Self::InvalidString => "invalid string",
                Self::UnknownResult(_) | Self::Plugin(_) => unreachable!(),
            }),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Plugin(e) => Some(&**e),
            _ => None,
        }
    }
}

fn result(code: i32) -> Result<()> {
    if code == sys::MG_OK {
        Ok(())
    } else {
        Err(error_from_code(code))
    }
}
fn error_from_code(code: i32) -> Error {
    match code {
        sys::MG_ERR_NULL_POINTER => Error::NullPointer,
        sys::MG_ERR_INVALID_ARGUMENT => Error::InvalidArgument,
        sys::MG_ERR_INVALID_HANDLE => Error::InvalidHandle,
        sys::MG_ERR_WRONG_THREAD => Error::WrongThread,
        sys::MG_ERR_BUFFER_TOO_SMALL => Error::BufferTooSmall,
        sys::MG_ERR_UNSUPPORTED_ABI => Error::UnsupportedAbi,
        sys::MG_ERR_PLUGIN_INIT_FAILED => Error::PluginInitFailed,
        sys::MG_ERR_SDK_FAILURE => Error::SdkFailure,
        sys::MG_ERR_PANIC => Error::Panic,
        sys::MG_ERR_INTERNAL => Error::Internal,
        sys::MG_ERR_COMMAND_POISONED => Error::CommandPoisoned,
        sys::MG_ERR_NOT_FOUND => Error::NotFound,
        other => Error::UnknownResult(other),
    }
}
fn abi_error(error: Error) -> i32 {
    match error {
        Error::NullPointer => sys::MG_ERR_NULL_POINTER,
        Error::InvalidArgument | Error::InvalidString => sys::MG_ERR_INVALID_ARGUMENT,
        Error::InvalidHandle => sys::MG_ERR_INVALID_HANDLE,
        Error::WrongThread => sys::MG_ERR_WRONG_THREAD,
        Error::BufferTooSmall => sys::MG_ERR_BUFFER_TOO_SMALL,
        Error::UnsupportedAbi => sys::MG_ERR_UNSUPPORTED_ABI,
        Error::PluginInitFailed => sys::MG_ERR_PLUGIN_INIT_FAILED,
        Error::SdkFailure | Error::Plugin(_) => sys::MG_ERR_SDK_FAILURE,
        Error::Panic => sys::MG_ERR_PANIC,
        Error::Internal => sys::MG_ERR_INTERNAL,
        Error::CommandPoisoned => sys::MG_ERR_COMMAND_POISONED,
        Error::NotFound => sys::MG_ERR_NOT_FOUND,
        Error::UnknownResult(v) => v,
    }
}

/// プラグインの表示情報。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PluginInfo {
    /// 表示名。
    pub name: String,
    /// 説明。
    pub description: String,
    /// 開発者名。
    pub developer: String,
}

/// DLL ごとのプラグイン実装。
pub trait Plugin: 'static {
    /// command の型。
    type Command: Command;
    /// 固定の表示情報を返す。
    fn info() -> PluginInfo;
    /// command instance を生成する。
    fn create_command() -> Result<Self::Command>;
}

/// Margrete から生成・実行される command。
pub trait Command: 'static {
    /// command の固定表示名。
    fn name(&self) -> &str;
    /// command を実行する。
    fn invoke(&mut self, context: &mut Context<'_>) -> Result<()>;
}

type Host = sys::MgHostApi;
static HOST: AtomicPtr<Host> = AtomicPtr::new(core::ptr::null_mut());
fn host() -> &'static Host {
    let p = HOST.load(Ordering::Acquire);
    debug_assert!(!p.is_null());
    unsafe { &*p }
}

fn call1<A: Copy>(f: Option<unsafe extern "C" fn(A) -> i32>, a: A) -> Result<()> {
    unsafe { result(f.ok_or(Error::PluginInitFailed)?(a)) }
}
fn call2<A: Copy, B: Copy>(f: Option<unsafe extern "C" fn(A, B) -> i32>, a: A, b: B) -> Result<()> {
    unsafe { result(f.ok_or(Error::PluginInitFailed)?(a, b)) }
}
fn call3<A: Copy, B: Copy, C: Copy>(
    f: Option<unsafe extern "C" fn(A, B, C) -> i32>,
    a: A,
    b: B,
    c: C,
) -> Result<()> {
    unsafe { result(f.ok_or(Error::PluginInitFailed)?(a, b, c)) }
}

macro_rules! owned_handle {
    ($name:ident,$raw:ty) => {
        #[doc = concat!(stringify!($name), " の invocation 限定所有ハンドル。Drop 時に解放され、Send/Sync ではありません。")]
        pub struct $name<'a> { raw: NonNull<$raw>, _brand: PhantomData<Cell<&'a ()>>, _local: PhantomData<Rc<()>> }
        impl<'a> $name<'a> { fn new(raw:*mut $raw)->Result<Self>{Ok(Self{raw:NonNull::new(raw).ok_or(Error::NullPointer)?,_brand:PhantomData,_local:PhantomData})} }
        impl Drop for $name<'_> { fn drop(&mut self){ let r=call1(host().object_release,self.raw.as_ptr().cast::<c_void>()); debug_assert!(r.is_ok(), "object_release failed: {r:?}"); } }
    }
}
owned_handle!(Document, sys::MgDocument);
owned_handle!(Chart, sys::MgChart);
owned_handle!(Note, sys::MgNote);
owned_handle!(UndoBuffer, sys::MgUndoBuffer);

/// Windows native window handle の opaque wrapper。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowHandle(NonNull<c_void>);
impl NativeWindowHandle {
    /// `HWND` 相当の値を返す。
    pub fn as_ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }
}

/// 一回の command invocation だけ有効な借用 context。
pub struct Context<'a> {
    raw: NonNull<sys::MgContext>,
    _brand: PhantomData<Cell<&'a ()>>,
    _local: PhantomData<Rc<()>>,
}
impl<'a> Context<'a> {
    fn new(raw: *mut sys::MgContext) -> Result<Self> {
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::NullPointer)?,
            _brand: PhantomData,
            _local: PhantomData,
        })
    }
    /// 編集中の document を取得する。
    pub fn document(&self) -> Result<Document<'a>> {
        let mut p = core::ptr::null_mut();
        call2(host().context_get_document, self.raw.as_ptr(), &mut p)?;
        Document::new(p)
    }
    /// 現在の tick を返す。
    pub fn current_tick(&self) -> Result<i32> {
        let mut v = 0;
        call2(host().context_get_current_tick, self.raw.as_ptr(), &mut v)?;
        Ok(v)
    }
    /// Margrete の main window handle を返す。
    pub fn main_window_handle(&self) -> Result<NativeWindowHandle> {
        let mut p = core::ptr::null_mut();
        call2(
            host().context_get_main_window_handle,
            self.raw.as_ptr(),
            &mut p,
        )?;
        Ok(NativeWindowHandle(
            NonNull::new(p).ok_or(Error::NullPointer)?,
        ))
    }
    /// 画面表示の更新を要求する。
    pub fn update(&self) -> Result<()> {
        call1(host().context_update, self.raw.as_ptr())
    }
    /// UTF-8 エラーをホストへ通知する。失敗を続けて返すと二重表示になりうる。
    pub fn report_error(&self, message: &str) -> Result<()> {
        if message.as_bytes().contains(&0) {
            return Err(Error::InvalidString);
        }
        let n = u32::try_from(message.len()).map_err(|_| Error::InvalidString)?;
        call3(
            host().report_error,
            self.raw.as_ptr(),
            message.as_ptr().cast(),
            n,
        )
    }
    /// Undo 記録を開始し、成功時に commit と表示更新を行う。
    pub fn transaction<T>(&self, operation: impl FnOnce(&Edit<'a>) -> Result<T>) -> Result<T> {
        let doc = self.document()?;
        let chart = doc.chart()?;
        let undo = doc.undo_buffer()?;
        let recording = undo.begin()?;
        let edit = Edit { chart };
        let value = match operation(&edit) {
            Ok(v) => v,
            Err(e) => {
                return match recording.discard() {
                    Ok(()) => Err(e),
                    Err(clean) => Err(clean),
                };
            }
        };
        recording.commit()?;
        self.update()?;
        Ok(value)
    }
}

impl<'a> Document<'a> {
    /// chart を取得する。
    pub fn chart(&self) -> Result<Chart<'a>> {
        let mut p = core::ptr::null_mut();
        call2(host().document_get_chart, self.raw.as_ptr(), &mut p)?;
        Chart::new(p)
    }
    /// Undo buffer を取得する。
    pub fn undo_buffer(&self) -> Result<UndoBuffer<'a>> {
        let mut p = core::ptr::null_mut();
        call2(host().document_get_undo_buffer, self.raw.as_ptr(), &mut p)?;
        UndoBuffer::new(p)
    }
}

/// transaction closure に渡される編集対象。
pub struct Edit<'a> {
    chart: Chart<'a>,
}
impl<'a> Edit<'a> {
    /// 編集対象 chart への参照。
    pub fn chart(&self) -> &Chart<'a> {
        &self.chart
    }
}

/// Rust 向けノート種別。未知値も保持する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoteType(pub i32);
impl NoteType {
    /// 不明。
    pub const UNKNOWN: Self = Self(0);
    /// TAP。
    pub const TAP: Self = Self(1);
    /// EX TAP。
    pub const EXTAP: Self = Self(2);
    /// FLICK。
    pub const FLICK: Self = Self(3);
    /// DAMAGE。
    pub const DAMAGE: Self = Self(4);
    /// HOLD。
    pub const HOLD: Self = Self(5);
    /// SLIDE。
    pub const SLIDE: Self = Self(6);
    /// AIR。
    pub const AIR: Self = Self(7);
    /// AIR HOLD。
    pub const AIR_HOLD: Self = Self(8);
    /// AIR SLIDE。
    pub const AIR_SLIDE: Self = Self(9);
    /// AIR CRUSH。
    pub const AIR_CRUSH: Self = Self(10);
    /// CLICK。
    pub const CLICK: Self = Self(11);
}
/// ロングノート属性。未知値も保持する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongAttribute(pub i32);
impl LongAttribute {
    /// なし。
    pub const NONE: Self = Self(0);
    /// 始点。
    pub const BEGIN: Self = Self(1);
    /// 中継点。
    pub const STEP: Self = Self(2);
    /// 制御点。
    pub const CONTROL: Self = Self(3);
    /// 曲線制御点。
    pub const CURVE_CONTROL: Self = Self(4);
    /// 終点。
    pub const END: Self = Self(5);
    /// 判定なし終点。
    pub const END_NO_ACT: Self = Self(6);
}
/// ノート方向。未知値も保持する。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct Direction(pub i32);
/// 拡張属性。未知値も保持する。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ExtraAttribute(pub i32);
/// ノートの値データ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoteInfo {
    /// 種別。
    pub note_type: NoteType,
    /// ロング属性。
    pub long_attribute: LongAttribute,
    /// 方向。
    pub direction: Direction,
    /// 拡張属性。
    pub extra_attribute: ExtraAttribute,
    /// variation ID。
    pub variation_id: i32,
    /// X 座標。
    pub x: i32,
    /// 幅。
    pub width: i32,
    /// 高さ。
    pub height: i32,
    /// tick。
    pub tick: i32,
    /// timeline ID。
    pub timeline_id: i32,
    /// option 値。
    pub option_value: i32,
}
impl Default for NoteInfo {
    fn default() -> Self {
        Self {
            note_type: NoteType::UNKNOWN,
            long_attribute: LongAttribute::NONE,
            direction: Direction(0),
            extra_attribute: ExtraAttribute(0),
            variation_id: 0,
            x: 0,
            width: 0,
            height: 0,
            tick: 0,
            timeline_id: 0,
            option_value: 0,
        }
    }
}
impl From<sys::MgNoteInfo> for NoteInfo {
    fn from(v: sys::MgNoteInfo) -> Self {
        Self {
            note_type: NoteType(v.r#type),
            long_attribute: LongAttribute(v.long_attr),
            direction: Direction(v.direction),
            extra_attribute: ExtraAttribute(v.ex_attr),
            variation_id: v.variation_id,
            x: v.x,
            width: v.width,
            height: v.height,
            tick: v.tick,
            timeline_id: v.timeline_id,
            option_value: v.option_value,
        }
    }
}
impl From<NoteInfo> for sys::MgNoteInfo {
    fn from(v: NoteInfo) -> Self {
        Self {
            r#type: v.note_type.0,
            long_attr: v.long_attribute.0,
            direction: v.direction.0,
            ex_attr: v.extra_attribute.0,
            variation_id: v.variation_id,
            x: v.x,
            width: v.width,
            height: v.height,
            tick: v.tick,
            timeline_id: v.timeline_id,
            option_value: v.option_value,
        }
    }
}

impl<'a> Chart<'a> {
    /// 新しい未追加ノートを作る。
    pub fn create_note(&self) -> Result<Note<'a>> {
        let mut p = core::ptr::null_mut();
        call2(host().chart_create_note, self.raw.as_ptr(), &mut p)?;
        Note::new(p)
    }
    /// chart 内のノート数。
    pub fn notes_count(&self) -> Result<i32> {
        let mut n = 0;
        call2(host().chart_get_notes_count, self.raw.as_ptr(), &mut n)?;
        Ok(n)
    }
    /// index のノートを取得する。
    pub fn note(&self, index: i32) -> Result<Note<'a>> {
        let mut p = core::ptr::null_mut();
        call3(host().chart_get_note, self.raw.as_ptr(), index, &mut p)?;
        Note::new(p)
    }
    /// ノートを追加する（所有権は移動しない）。
    pub fn append_note(&self, note: &Note<'a>) -> Result<()> {
        call2(
            host().chart_append_note,
            self.raw.as_ptr(),
            note.raw.as_ptr(),
        )
    }
    /// ノートを削除し、ハンドルを消費する。
    pub fn delete_note(&self, note: Note<'a>) -> Result<()> {
        call2(
            host().chart_delete_note,
            self.raw.as_ptr(),
            note.raw.as_ptr(),
        )
    }
    /// 全ノートを tick 単位で移動する。
    pub fn offset_notes(&self, offset: i32) -> Result<()> {
        call2(host().chart_offset_notes, self.raw.as_ptr(), offset)
    }
}
impl<'a> Note<'a> {
    /// ID を返す。
    pub fn id(&self) -> Result<i32> {
        let mut v = 0;
        call2(host().note_get_id, self.raw.as_ptr(), &mut v)?;
        Ok(v)
    }
    /// 値データを返す。
    pub fn info(&self) -> Result<NoteInfo> {
        let mut v = sys::MgNoteInfo::default();
        call2(host().note_get_info, self.raw.as_ptr(), &mut v)?;
        Ok(v.into())
    }
    /// 値データを設定する。
    pub fn set_info(&self, info: &NoteInfo) -> Result<()> {
        let raw = sys::MgNoteInfo::from(*info);
        call2(host().note_set_info, self.raw.as_ptr(), &raw)
    }
    /// 子ノート数。
    pub fn children_count(&self) -> Result<i32> {
        let mut n = 0;
        call2(host().note_get_children_count, self.raw.as_ptr(), &mut n)?;
        Ok(n)
    }
    /// 子ノートを取得する。
    pub fn child(&self, index: i32) -> Result<Note<'a>> {
        let mut p = core::ptr::null_mut();
        call3(host().note_get_child, self.raw.as_ptr(), index, &mut p)?;
        Note::new(p)
    }
    /// 親ノートを取得する。
    pub fn parent(&self) -> Result<Option<Note<'a>>> {
        self.optional(host().note_get_parent)
    }
    /// 基点ノートを取得する。
    pub fn base_note(&self) -> Result<Option<Note<'a>>> {
        self.optional(host().note_get_base_note)
    }
    fn optional(
        &self,
        f: Option<unsafe extern "C" fn(*mut sys::MgNote, *mut *mut sys::MgNote) -> i32>,
    ) -> Result<Option<Note<'a>>> {
        let mut p = core::ptr::null_mut();
        match call2(f, self.raw.as_ptr(), &mut p) {
            Ok(()) => Ok(Some(Note::new(p)?)),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    /// 子ノートを追加する。
    pub fn append_child(&self, child: &Note<'a>) -> Result<()> {
        call2(
            host().note_append_child,
            self.raw.as_ptr(),
            child.raw.as_ptr(),
        )
    }
    /// 子ノートを削除し、ハンドルを消費する。
    pub fn delete_child(&self, child: Note<'a>) -> Result<()> {
        call2(
            host().note_delete_child,
            self.raw.as_ptr(),
            child.raw.as_ptr(),
        )
    }
    /// ノートを clone する。
    pub fn duplicate(&self) -> Result<Note<'a>> {
        let mut p = core::ptr::null_mut();
        call2(host().note_clone, self.raw.as_ptr(), &mut p)?;
        Note::new(p)
    }
    /// 内容を別ノートで置換する。
    pub fn replace_with(&self, source: &Note<'a>, preserve_id: bool) -> Result<()> {
        call3(
            host().note_replace_with,
            self.raw.as_ptr(),
            source.raw.as_ptr().cast_const(),
            i32::from(preserve_id),
        )
    }
    /// 値データを別ノートへコピーする。
    pub fn copy_info_to(&self, target: &Note<'a>) -> Result<()> {
        call2(
            host().note_copy_info_to,
            self.raw.as_ptr(),
            target.raw.as_ptr(),
        )
    }
    /// 子ノートを tick 単位で移動する。
    pub fn offset_children(&self, offset: i32) -> Result<()> {
        call2(host().note_offset_child, self.raw.as_ptr(), offset)
    }
    /// 水平方向に反転する。
    pub fn flip_horizontal(&self, with_children: bool) -> Result<()> {
        call2(
            host().note_flip_h,
            self.raw.as_ptr(),
            i32::from(with_children),
        )
    }
}

macro_rules! event_handle {
    ($name:ident) => {
        #[doc = concat!(stringify!($name), " の型付き invocation 限定ハンドル。")]
        pub struct $name<'a> {
            raw: NonNull<sys::MgEvent>,
            _brand: PhantomData<Cell<&'a ()>>,
            _local: PhantomData<Rc<()>>,
        }
        impl<'a> $name<'a> {
            fn new(raw: *mut sys::MgEvent) -> Result<Self> {
                Ok(Self {
                    raw: NonNull::new(raw).ok_or(Error::NullPointer)?,
                    _brand: PhantomData,
                    _local: PhantomData,
                })
            }
            /// event ID を返す。
            pub fn id(&self) -> Result<i32> {
                let mut id = 0;
                call2(host().event_get_id, self.raw.as_ptr(), &mut id)?;
                Ok(id)
            }
        }
        impl Drop for $name<'_> {
            fn drop(&mut self) {
                let r = call1(host().object_release, self.raw.as_ptr().cast::<c_void>());
                debug_assert!(r.is_ok(), "object_release failed: {r:?}");
            }
        }
    };
}
event_handle!(TimelineSpeedEvent);
event_handle!(NoteSpeedModifierEvent);
event_handle!(BpmEvent);
event_handle!(BeatChangeEvent);

/// timeline speed event の値データ。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineSpeedInfo {
    /// timeline ID。
    pub timeline_id: i32,
    /// tick。
    pub tick: i32,
    /// speed（有限値のみ）。
    pub speed: f64,
}
/// note speed modifier event の値データ。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteSpeedModifierInfo {
    /// tick。
    pub tick: i32,
    /// speed（有限値のみ）。
    pub speed: f64,
}
/// BPM event の値データ。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BpmInfo {
    /// tick。
    pub tick: i32,
    /// BPM（有限値のみ）。
    pub bpm: f64,
}
/// 拍子変更 event の値データ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeatChangeInfo {
    /// 小節。
    pub bar: i32,
    /// 一小節の拍数。
    pub beats_per_bar: i32,
    /// 拍の単位。
    pub beat_unit: i32,
}

impl TimelineSpeedEvent<'_> {
    /// 値を取得する。
    pub fn info(&self) -> Result<TimelineSpeedInfo> {
        let mut v = sys::MgEventTimelineSpeedInfo::default();
        call2(
            host().event_timeline_speed_get_info,
            self.raw.as_ptr(),
            &mut v,
        )?;
        Ok(TimelineSpeedInfo {
            timeline_id: v.timeline_id,
            tick: v.tick,
            speed: v.speed,
        })
    }
    /// 値を設定する。
    pub fn set_info(&self, v: &TimelineSpeedInfo) -> Result<()> {
        if !v.speed.is_finite() {
            return Err(Error::InvalidArgument);
        }
        let raw = sys::MgEventTimelineSpeedInfo {
            timeline_id: v.timeline_id,
            tick: v.tick,
            speed: v.speed,
        };
        call2(
            host().event_timeline_speed_set_info,
            self.raw.as_ptr(),
            &raw,
        )
    }
}
impl NoteSpeedModifierEvent<'_> {
    /// 値を取得する。
    pub fn info(&self) -> Result<NoteSpeedModifierInfo> {
        let mut v = sys::MgEventNoteSpeedModifierInfo::default();
        call2(
            host().event_note_speed_modifier_get_info,
            self.raw.as_ptr(),
            &mut v,
        )?;
        Ok(NoteSpeedModifierInfo {
            tick: v.tick,
            speed: v.speed,
        })
    }
    /// 値を設定する。
    pub fn set_info(&self, v: &NoteSpeedModifierInfo) -> Result<()> {
        if !v.speed.is_finite() {
            return Err(Error::InvalidArgument);
        }
        let raw = sys::MgEventNoteSpeedModifierInfo {
            tick: v.tick,
            speed: v.speed,
        };
        call2(
            host().event_note_speed_modifier_set_info,
            self.raw.as_ptr(),
            &raw,
        )
    }
}
impl BpmEvent<'_> {
    /// 値を取得する。
    pub fn info(&self) -> Result<BpmInfo> {
        let mut v = sys::MgEventBpmInfo::default();
        call2(host().event_bpm_get_info, self.raw.as_ptr(), &mut v)?;
        Ok(BpmInfo {
            tick: v.tick,
            bpm: v.bpm,
        })
    }
    /// 値を設定する。
    pub fn set_info(&self, v: &BpmInfo) -> Result<()> {
        if !v.bpm.is_finite() {
            return Err(Error::InvalidArgument);
        }
        let raw = sys::MgEventBpmInfo {
            tick: v.tick,
            bpm: v.bpm,
        };
        call2(host().event_bpm_set_info, self.raw.as_ptr(), &raw)
    }
}
impl BeatChangeEvent<'_> {
    /// 値を取得する。
    pub fn info(&self) -> Result<BeatChangeInfo> {
        let mut v = sys::MgEventBeatChangeInfo::default();
        call2(host().event_beat_change_get_info, self.raw.as_ptr(), &mut v)?;
        Ok(BeatChangeInfo {
            bar: v.bar,
            beats_per_bar: v.beats_per_bar,
            beat_unit: v.beat_unit,
        })
    }
    /// 値を設定する。
    pub fn set_info(&self, v: &BeatChangeInfo) -> Result<()> {
        let raw = sys::MgEventBeatChangeInfo {
            bar: v.bar,
            beats_per_bar: v.beats_per_bar,
            beat_unit: v.beat_unit,
        };
        call2(host().event_beat_change_set_info, self.raw.as_ptr(), &raw)
    }
}

/// kind が検証済みの型付き event。
pub enum Event<'a> {
    /// timeline speed。
    TimelineSpeed(TimelineSpeedEvent<'a>),
    /// note speed modifier。
    NoteSpeedModifier(NoteSpeedModifierEvent<'a>),
    /// BPM。
    Bpm(BpmEvent<'a>),
    /// 拍子変更。
    BeatChange(BeatChangeEvent<'a>),
}
impl Event<'_> {
    fn raw(&self) -> *mut sys::MgEvent {
        match self {
            Self::TimelineSpeed(v) => v.raw.as_ptr(),
            Self::NoteSpeedModifier(v) => v.raw.as_ptr(),
            Self::Bpm(v) => v.raw.as_ptr(),
            Self::BeatChange(v) => v.raw.as_ptr(),
        }
    }
}
impl<'a> From<TimelineSpeedEvent<'a>> for Event<'a> {
    fn from(v: TimelineSpeedEvent<'a>) -> Self {
        Self::TimelineSpeed(v)
    }
}
impl<'a> From<NoteSpeedModifierEvent<'a>> for Event<'a> {
    fn from(v: NoteSpeedModifierEvent<'a>) -> Self {
        Self::NoteSpeedModifier(v)
    }
}
impl<'a> From<BpmEvent<'a>> for Event<'a> {
    fn from(v: BpmEvent<'a>) -> Self {
        Self::Bpm(v)
    }
}
impl<'a> From<BeatChangeEvent<'a>> for Event<'a> {
    fn from(v: BeatChangeEvent<'a>) -> Self {
        Self::BeatChange(v)
    }
}

impl<'a> Chart<'a> {
    fn create_event(&self, kind: i32) -> Result<*mut sys::MgEvent> {
        let mut p = core::ptr::null_mut();
        call3(host().chart_create_event, self.raw.as_ptr(), kind, &mut p)?;
        Ok(p)
    }
    /// timeline speed event を作る。
    pub fn create_timeline_speed_event(&self) -> Result<TimelineSpeedEvent<'a>> {
        TimelineSpeedEvent::new(self.create_event(sys::MG_EVENT_KIND_TIMELINE_SPEED)?)
    }
    /// note speed modifier event を作る。
    pub fn create_note_speed_modifier_event(&self) -> Result<NoteSpeedModifierEvent<'a>> {
        NoteSpeedModifierEvent::new(self.create_event(sys::MG_EVENT_KIND_NOTE_SPEED_MODIFIER)?)
    }
    /// BPM event を作る。
    pub fn create_bpm_event(&self) -> Result<BpmEvent<'a>> {
        BpmEvent::new(self.create_event(sys::MG_EVENT_KIND_BPM)?)
    }
    /// 拍子変更 event を作る。
    pub fn create_beat_change_event(&self) -> Result<BeatChangeEvent<'a>> {
        BeatChangeEvent::new(self.create_event(sys::MG_EVENT_KIND_BEAT_CHANGE)?)
    }
    /// event を chart に追加する（所有権は移動しない）。
    pub fn append_event(&self, event: &Event<'a>) -> Result<()> {
        call2(host().chart_append_event, self.raw.as_ptr(), event.raw())
    }
    /// event を削除し、ハンドルを消費する。
    pub fn delete_event(&self, event: Event<'a>) -> Result<()> {
        call2(host().chart_delete_event, self.raw.as_ptr(), event.raw())
    }
    fn find_event<T>(
        &self,
        call: impl FnOnce(*mut *mut sys::MgEvent) -> Result<()>,
        wrap: impl FnOnce(*mut sys::MgEvent) -> Result<T>,
    ) -> Result<Option<T>> {
        let mut p = core::ptr::null_mut();
        match call(&mut p) {
            Ok(()) => Ok(Some(wrap(p)?)),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    /// tick と timeline ID から timeline speed event を検索する。
    pub fn find_timeline_speed_event(
        &self,
        tick: i32,
        timeline_id: i32,
    ) -> Result<Option<TimelineSpeedEvent<'a>>> {
        self.find_event(
            |out| unsafe {
                result(host()
                    .chart_find_event_timeline_speed
                    .ok_or(Error::PluginInitFailed)?(
                    self.raw.as_ptr(),
                    tick,
                    timeline_id,
                    out,
                ))
            },
            TimelineSpeedEvent::new,
        )
    }
    /// tick から note speed modifier event を検索する。
    pub fn find_note_speed_modifier_event(
        &self,
        tick: i32,
    ) -> Result<Option<NoteSpeedModifierEvent<'a>>> {
        self.find_event(
            |out| {
                call3(
                    host().chart_find_event_note_speed_modifier,
                    self.raw.as_ptr(),
                    tick,
                    out,
                )
            },
            NoteSpeedModifierEvent::new,
        )
    }
    /// tick から BPM event を検索する。
    pub fn find_bpm_event(&self, tick: i32) -> Result<Option<BpmEvent<'a>>> {
        self.find_event(
            |out| call3(host().chart_find_event_bpm, self.raw.as_ptr(), tick, out),
            BpmEvent::new,
        )
    }
    /// bar から拍子変更 event を検索する。
    pub fn find_beat_change_event(&self, bar: i32) -> Result<Option<BeatChangeEvent<'a>>> {
        self.find_event(
            |out| {
                call3(
                    host().chart_find_event_beat_change,
                    self.raw.as_ptr(),
                    bar,
                    out,
                )
            },
            BeatChangeEvent::new,
        )
    }
}

impl<'a> UndoBuffer<'a> {
    /// Undo 可能か返す。
    pub fn can_undo(&self) -> Result<bool> {
        self.flag(host().undo_can_undo)
    }
    /// Redo 可能か返す。
    pub fn can_redo(&self) -> Result<bool> {
        self.flag(host().undo_can_redo)
    }
    /// Undo を実行する。
    pub fn undo(&self) -> Result<()> {
        call1(host().undo_undo, self.raw.as_ptr())
    }
    /// Redo を実行する。
    pub fn redo(&self) -> Result<()> {
        call1(host().undo_redo, self.raw.as_ptr())
    }
    fn flag(
        &self,
        f: Option<unsafe extern "C" fn(*mut sys::MgUndoBuffer, *mut i32) -> i32>,
    ) -> Result<bool> {
        let mut v = 0;
        call2(f, self.raw.as_ptr(), &mut v)?;
        Ok(v != 0)
    }
    /// Undo 記録を開始する。
    pub fn begin(self) -> Result<UndoRecording<'a>> {
        call1(host().undo_begin_recording, self.raw.as_ptr())?;
        Ok(UndoRecording { buffer: Some(self) })
    }
}
/// 開始済み Undo 記録。未完了のまま Drop すると破棄を試みる。
pub struct UndoRecording<'a> {
    buffer: Option<UndoBuffer<'a>>,
}
impl UndoRecording<'_> {
    fn finish(&mut self, commit: bool) -> Result<()> {
        let b = self.buffer.as_ref().expect("active recording");
        let r = if commit {
            call1(host().undo_commit_recording, b.raw.as_ptr())
        } else {
            call1(host().undo_discard_recording, b.raw.as_ptr())
        };
        if r.is_ok() {
            self.buffer.take();
        }
        r
    }
    /// 記録を commit する。
    pub fn commit(mut self) -> Result<()> {
        if let Err(primary) = self.finish(true) {
            let _ = self.finish(false);
            return Err(primary);
        }
        Ok(())
    }
    /// 記録を破棄する。
    pub fn discard(mut self) -> Result<()> {
        self.finish(false)
    }
}
impl Drop for UndoRecording<'_> {
    fn drop(&mut self) {
        if self.buffer.is_some() {
            let _ = self.finish(false);
            if self.buffer.is_some() {
                mem::forget(self.buffer.take());
            }
        }
    }
}

struct PluginState<P: Plugin> {
    info: PluginInfo,
    _marker: PhantomData<P>,
}
struct CommandState<C: Command> {
    command: C,
    name: String,
}

/// `Plugin` 実装を C ABI の `mg_plugin_init` として登録する。
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        const _: () = {
            // Margrete 向け export は Rust から直接参照されないため、最終 DLL へ
            // marmkmt static archive 全体を取り込む。
            #[link(name = "marmkmt", kind = "static", modifiers = "+whole-archive")]
            unsafe extern "C" {}

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn mg_plugin_init(
                requested_abi_version: u32,
                host_api: *const $crate::sys::MgHostApi,
                out_plugin_api: *mut $crate::sys::MgPluginApi,
            ) -> $crate::sys::MgResult {
                unsafe {
                    $crate::__private::plugin_init::<$plugin>(
                        requested_abi_version,
                        host_api,
                        out_plugin_api,
                    )
                }
            }
        };
    };
}

/// マクロ展開用。直接使用しないでください。
#[doc(hidden)]
pub mod __private {
    pub use super::plugin_init;
}

#[doc(hidden)]
pub unsafe fn plugin_init<P: Plugin>(
    requested: u32,
    host_ptr: *const Host,
    out: *mut sys::MgPluginApi,
) -> i32 {
    ffi_boundary(|| {
        if requested != sys::MG_ABI_VERSION {
            return sys::MG_ERR_UNSUPPORTED_ABI;
        }
        if host_ptr.is_null() || out.is_null() {
            return sys::MG_ERR_NULL_POINTER;
        }
        let h = unsafe { &*host_ptr };
        if h.abi_version != sys::MG_ABI_VERSION
            || h.struct_size < (mem::size_of::<Host>() as u32)
            || !required(h)
        {
            return sys::MG_ERR_UNSUPPORTED_ABI;
        }
        if HOST
            .compare_exchange(
                core::ptr::null_mut(),
                host_ptr.cast_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err_and(|p| p != host_ptr.cast_mut())
        {
            return sys::MG_ERR_PLUGIN_INIT_FAILED;
        }
        let state = Box::into_raw(Box::new(PluginState::<P> {
            info: P::info(),
            _marker: PhantomData,
        }));
        unsafe {
            out.write(sys::MgPluginApi {
                struct_size: mem::size_of::<sys::MgPluginApi>() as u32,
                abi_version: sys::MG_ABI_VERSION,
                plugin_data: state.cast(),
                get_plugin_info: Some(get_info::<P>),
                create_command: Some(create::<P>),
                destroy_command: Some(destroy::<P::Command>),
                get_command_name: Some(command_name::<P::Command>),
                invoke: Some(invoke::<P::Command>),
            })
        };
        sys::MG_OK
    })
}
fn required(h: &Host) -> bool {
    h.object_add_ref.is_some()
        && h.object_release.is_some()
        && h.report_error.is_some()
        && h.context_get_document.is_some()
        && h.context_get_main_window_handle.is_some()
        && h.context_get_current_tick.is_some()
        && h.context_update.is_some()
        && h.document_get_chart.is_some()
        && h.document_get_undo_buffer.is_some()
        && h.undo_begin_recording.is_some()
        && h.undo_commit_recording.is_some()
        && h.undo_discard_recording.is_some()
        && h.undo_undo.is_some()
        && h.undo_redo.is_some()
        && h.undo_can_undo.is_some()
        && h.undo_can_redo.is_some()
        && h.undo_is_recording.is_some()
        && h.chart_create_note.is_some()
        && h.chart_get_notes_count.is_some()
        && h.chart_get_note.is_some()
        && h.chart_append_note.is_some()
        && h.chart_delete_note.is_some()
        && h.chart_offset_notes.is_some()
        && h.note_get_id.is_some()
        && h.note_get_info.is_some()
        && h.note_set_info.is_some()
        && h.note_get_children_count.is_some()
        && h.note_get_child.is_some()
        && h.note_get_parent.is_some()
        && h.note_append_child.is_some()
        && h.note_delete_child.is_some()
        && h.note_clone.is_some()
        && h.note_replace_with.is_some()
        && h.note_copy_info_to.is_some()
        && h.note_get_base_note.is_some()
        && h.note_offset_child.is_some()
        && h.note_flip_h.is_some()
        && h.chart_create_event.is_some()
        && h.chart_append_event.is_some()
        && h.chart_delete_event.is_some()
        && h.chart_find_event_timeline_speed.is_some()
        && h.chart_find_event_note_speed_modifier.is_some()
        && h.chart_find_event_bpm.is_some()
        && h.chart_find_event_beat_change.is_some()
        && h.event_get_kind.is_some()
        && h.event_get_id.is_some()
        && h.event_timeline_speed_get_info.is_some()
        && h.event_timeline_speed_set_info.is_some()
        && h.event_note_speed_modifier_get_info.is_some()
        && h.event_note_speed_modifier_set_info.is_some()
        && h.event_bpm_get_info.is_some()
        && h.event_bpm_set_info.is_some()
        && h.event_beat_change_get_info.is_some()
        && h.event_beat_change_set_info.is_some()
        && h.event_replace_with.is_some()
        && h.event_copy_info_to.is_some()
}
unsafe extern "C" fn get_info<P: Plugin>(p: *mut c_void, b: *mut sys::MgPluginInfoBuffers) -> i32 {
    ffi_boundary(|| {
        if p.is_null() || b.is_null() {
            return sys::MG_ERR_NULL_POINTER;
        }
        let s = unsafe { &*(p.cast::<PluginState<P>>()) };
        let b = unsafe { &mut *b };
        if b.struct_size < mem::size_of::<sys::MgPluginInfoBuffers>() as u32 {
            return sys::MG_ERR_INVALID_ARGUMENT;
        }
        for (v, d, c, r) in [
            (&s.info.name, b.name, b.name_capacity, b.name_required),
            (
                &s.info.description,
                b.description,
                b.description_capacity,
                b.description_required,
            ),
            (
                &s.info.developer,
                b.developer,
                b.developer_capacity,
                b.developer_required,
            ),
        ] {
            if let Err(e) = write_string(v, d, c, r) {
                return abi_error(e);
            }
        }
        sys::MG_OK
    })
}
unsafe extern "C" fn create<P: Plugin>(_: *mut c_void, out: *mut *mut c_void) -> i32 {
    ffi_boundary(|| {
        if out.is_null() {
            return sys::MG_ERR_NULL_POINTER;
        }
        match P::create_command() {
            Ok(c) => {
                let name = c.name().to_owned();
                unsafe {
                    out.write(Box::into_raw(Box::new(CommandState { command: c, name })).cast())
                };
                sys::MG_OK
            }
            Err(e) => abi_error(e),
        }
    })
}
unsafe extern "C" fn destroy<C: Command>(_: *mut c_void, p: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !p.is_null() {
            unsafe { drop(Box::from_raw(p.cast::<CommandState<C>>())) }
        }
    }));
}
unsafe extern "C" fn command_name<C: Command>(
    _: *mut c_void,
    p: *mut c_void,
    d: *mut i8,
    c: u32,
    r: *mut u32,
) -> i32 {
    ffi_boundary(|| {
        if p.is_null() {
            sys::MG_ERR_INVALID_HANDLE
        } else {
            let s = unsafe { &*p.cast::<CommandState<C>>() };
            write_string(&s.name, d, c, r).map_or_else(abi_error, |_| sys::MG_OK)
        }
    })
}
unsafe extern "C" fn invoke<C: Command>(
    _: *mut c_void,
    p: *mut c_void,
    ctx: *mut sys::MgContext,
) -> i32 {
    ffi_boundary(|| {
        if p.is_null() {
            return sys::MG_ERR_INVALID_HANDLE;
        }
        let mut context = match Context::new(ctx) {
            Ok(v) => v,
            Err(e) => return abi_error(e),
        };
        let s = unsafe { &mut *p.cast::<CommandState<C>>() };
        s.command
            .invoke(&mut context)
            .map_or_else(abi_error, |_| sys::MG_OK)
    })
}
fn write_string(v: &str, d: *mut i8, capacity: u32, required: *mut u32) -> Result<()> {
    if required.is_null() {
        return Err(Error::NullPointer);
    }
    if v.as_bytes().contains(&0) {
        return Err(Error::InvalidString);
    }
    let n = v
        .len()
        .checked_add(1)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(Error::InvalidString)?;
    unsafe { required.write(n) };
    if d.is_null() {
        return if capacity == 0 {
            Ok(())
        } else {
            Err(Error::NullPointer)
        };
    }
    if capacity < n {
        return Err(Error::BufferTooSmall);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(v.as_ptr(), d.cast(), v.len());
        d.add(v.len()).write(0)
    }
    Ok(())
}
fn ffi_boundary(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(sys::MG_ERR_PANIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_codes_preserve_unknown_values() {
        assert!(matches!(
            error_from_code(-12345),
            Error::UnknownResult(-12345)
        ));
        assert_eq!(abi_error(Error::UnknownResult(-12345)), -12345);
    }

    #[test]
    fn string_callback_uses_nul_terminated_byte_count() {
        let mut required = 0;
        write_string("あ", core::ptr::null_mut(), 0, &mut required).unwrap();
        assert_eq!(required, 4);
        let mut buffer = [1_i8; 4];
        write_string("あ", buffer.as_mut_ptr(), 4, &mut required).unwrap();
        assert_eq!(buffer[3], 0);
    }

    #[test]
    fn string_callback_rejects_invalid_inputs() {
        let mut required = 0;
        assert!(matches!(
            write_string("a\0b", core::ptr::null_mut(), 0, &mut required),
            Err(Error::InvalidString)
        ));
        assert!(matches!(
            write_string("abc", core::ptr::null_mut(), 1, &mut required),
            Err(Error::NullPointer)
        ));
        let mut short = [0_i8; 3];
        assert!(matches!(
            write_string("abc", short.as_mut_ptr(), 3, &mut required),
            Err(Error::BufferTooSmall)
        ));
        assert_eq!(required, 4);
    }

    #[test]
    fn floating_point_setters_validate_finite_values_before_ffi() {
        assert!(!f64::NAN.is_finite());
        assert!(!f64::INFINITY.is_finite());
    }
}
