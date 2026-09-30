use std::{cell::RefCell, marker::PhantomData, ptr, rc::Rc};

use marmkmt::NativeWindowHandle;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, SetLastError},
    System::Threading::GetCurrentProcessId,
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
        WindowsAndMessaging::{
            EnumWindows, GWLP_HWNDPARENT, GetPropW, GetWindowThreadProcessId, IsWindow,
            RemovePropW, SetPropW, SetWindowLongPtrW,
        },
    },
};

const BLOCK_PROPERTY: *const u16 = windows_sys::w!("sound2slide.ModalHost");

pub(crate) struct ModalHost {
    owner: HWND,
    blocked: RefCell<Vec<HWND>>,
    token: Box<u8>,
    _local: PhantomData<Rc<()>>,
}

impl ModalHost {
    pub(crate) fn block(owner: NativeWindowHandle) -> Result<Self, String> {
        Self::block_window(owner.as_ptr())
    }

    fn block_window(owner: HWND) -> Result<Self, String> {
        if unsafe { IsWindow(owner) } == 0 || !belongs_to_process(owner) {
            return Err("Margreteのウィンドウを取得できません".into());
        }
        let guard = Self {
            owner,
            blocked: RefCell::new(Vec::new()),
            token: Box::new(0),
            _local: PhantomData,
        };
        let mut windows = Vec::<HWND>::new();
        if unsafe { EnumWindows(Some(collect_windows), &mut windows as *mut _ as LPARAM) } == 0 {
            return Err("Margreteのウィンドウを列挙できません".into());
        }
        for window in windows {
            if unsafe { IsWindowEnabled(window) } == 0 {
                continue;
            }
            if unsafe { SetPropW(window, BLOCK_PROPERTY, guard.token_handle()) } == 0 {
                return Err("Margreteの操作をブロックできません".into());
            }
            guard.blocked.borrow_mut().push(window);
            unsafe { EnableWindow(window, 0) };
            if unsafe { IsWindowEnabled(window) } != 0 {
                return Err("Margreteの操作をブロックできません".into());
            }
        }
        Ok(guard)
    }

    pub(crate) fn attach(&self, window: &impl HasWindowHandle) -> Result<(), String> {
        let raw = window.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(raw) = raw.as_raw() else {
            return Err("拡張機能のウィンドウを取得できません".into());
        };
        self.attach_window(raw.hwnd.get() as HWND)
    }

    fn attach_window(&self, window: HWND) -> Result<(), String> {
        unsafe { SetLastError(0) };
        let previous = unsafe { SetWindowLongPtrW(window, GWLP_HWNDPARENT, self.owner as isize) };
        if previous == 0 && unsafe { GetLastError() } != 0 {
            return Err("拡張機能のウィンドウ所有者を設定できません".into());
        }
        Ok(())
    }

    pub(crate) fn restore(&self) {
        // HWNDs can be reused after destruction; the property identifies only windows we disabled.
        let windows = std::mem::take(&mut *self.blocked.borrow_mut());
        for window in windows.into_iter().rev() {
            if unsafe { GetPropW(window, BLOCK_PROPERTY) } == self.token_handle() {
                unsafe {
                    RemovePropW(window, BLOCK_PROPERTY);
                    EnableWindow(window, 1);
                }
            }
        }
    }

    fn token_handle(&self) -> *mut std::ffi::c_void {
        ptr::from_ref(self.token.as_ref()).cast_mut().cast()
    }
}

impl Drop for ModalHost {
    fn drop(&mut self) {
        self.restore();
    }
}

fn belongs_to_process(window: HWND) -> bool {
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(window, &mut process) };
    process == unsafe { GetCurrentProcessId() }
}

unsafe extern "system" fn collect_windows(window: HWND, data: LPARAM) -> i32 {
    if belongs_to_process(window) {
        unsafe { &mut *(data as *mut Vec<HWND>) }.push(window);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::Mutex,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GW_OWNER, GetWindow, WS_OVERLAPPED,
    };

    static WINDOWS: Mutex<()> = Mutex::new(());

    struct TestWindow(HWND);

    impl TestWindow {
        fn new(enabled: bool) -> Self {
            let window = unsafe {
                CreateWindowExW(
                    0,
                    windows_sys::w!("STATIC"),
                    windows_sys::w!("sound2slide test"),
                    WS_OVERLAPPED,
                    0,
                    0,
                    10,
                    10,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null(),
                )
            };
            assert!(!window.is_null());
            unsafe { EnableWindow(window, enabled.into()) };
            Self(window)
        }

        fn enabled(&self) -> bool {
            unsafe { IsWindowEnabled(self.0) != 0 }
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn blocks_all_host_windows_and_preserves_disabled_windows() {
        let _lock = WINDOWS.lock().unwrap();
        let owner = TestWindow::new(true);
        let second = TestWindow::new(true);
        let disabled = TestWindow::new(false);
        let guard = ModalHost::block_window(owner.0).unwrap();
        assert!(!owner.enabled());
        assert!(!second.enabled());
        assert!(!disabled.enabled());
        let plugin = TestWindow::new(true);
        guard.attach_window(plugin.0).unwrap();
        assert!(plugin.enabled());
        assert_eq!(unsafe { GetWindow(plugin.0, GW_OWNER) }, owner.0);
        guard.restore();
        assert!(owner.enabled());
        assert!(second.enabled());
        assert!(!disabled.enabled());
        drop(guard);
        assert!(owner.enabled());
    }

    #[test]
    fn restores_windows_after_error_and_panic() {
        let _lock = WINDOWS.lock().unwrap();
        let owner = TestWindow::new(true);
        let second = TestWindow::new(true);
        let fail = || -> Result<(), String> {
            let _guard = ModalHost::block_window(owner.0)?;
            assert!(!second.enabled());
            Err("UI creation failed".into())
        };
        assert!(fail().is_err());
        assert!(owner.enabled());
        assert!(second.enabled());
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = ModalHost::block_window(owner.0).unwrap();
            assert!(!second.enabled());
            panic!("UI panic");
        }));
        assert!(result.is_err());
        assert!(owner.enabled());
        assert!(second.enabled());
    }

    #[test]
    fn owner_destruction_also_destroys_the_plugin_window() {
        let _lock = WINDOWS.lock().unwrap();
        let owner = TestWindow::new(true);
        let second = TestWindow::new(true);
        let guard = ModalHost::block_window(owner.0).unwrap();
        let plugin = TestWindow::new(true);
        guard.attach_window(plugin.0).unwrap();
        drop(owner);
        assert_eq!(unsafe { IsWindow(plugin.0) }, 0);
        drop(guard);
        assert!(second.enabled());
    }

    #[test]
    fn invalid_window_handles_return_errors() {
        let _lock = WINDOWS.lock().unwrap();
        assert!(ModalHost::block_window(ptr::null_mut()).is_err());
        let owner = TestWindow::new(true);
        let guard = ModalHost::block_window(owner.0).unwrap();
        assert!(guard.attach_window(ptr::null_mut()).is_err());
        drop(guard);
        assert!(owner.enabled());
    }

    #[test]
    fn destroyed_windows_are_not_restored() {
        let _lock = WINDOWS.lock().unwrap();
        let owner = TestWindow::new(true);
        let second = TestWindow::new(true);
        let guard = ModalHost::block_window(owner.0).unwrap();
        let destroyed = second.0;
        drop(second);
        assert!(unsafe { GetPropW(destroyed, BLOCK_PROPERTY) }.is_null());
        drop(guard);
        assert!(owner.enabled());
    }
}
