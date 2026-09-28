//! Host keyboard interception, enabled only while a text field owns input.
use std::{collections::{HashMap, HashSet}, ffi::c_int, ptr, sync::{LazyLock, Mutex}};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx,
        TranslateMessage, HC_ACTION, HHOOK, MSG, PM_REMOVE, WH_GETMESSAGE,
        WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_NULL},
};
use super::BaseviewWindow;
use crate::wrappers::win32::window::wnd_proc;

struct Installed { hook: HHOOK, windows: HashSet<isize> }
// Handles are only stored/compared here. Each hook is installed and removed on its UI thread.
unsafe impl Send for Installed {}
static HOOKS: LazyLock<Mutex<HashMap<u32, Installed>>> = LazyLock::new(Mutex::default);

pub(crate) struct KeyboardHookHandle { thread: u32, window: isize }
pub(crate) fn init_keyboard_hook(hwnd: HWND) -> Option<KeyboardHookHandle> {
    let thread = unsafe { GetCurrentThreadId() };
    let mut hooks = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
    if let std::collections::hash_map::Entry::Vacant(entry) = hooks.entry(thread) {
        let hook = unsafe { SetWindowsHookExW(WH_GETMESSAGE, Some(callback), ptr::null_mut(), thread) };
        if hook.is_null() { return None; }
        entry.insert(Installed { hook, windows: HashSet::new() });
    }
    hooks.get_mut(&thread)?.windows.insert(hwnd as isize);
    Some(KeyboardHookHandle { thread, window: hwnd as isize })
}
impl Drop for KeyboardHookHandle {
    fn drop(&mut self) {
        let mut hooks = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hook) = hooks.get_mut(&self.thread) {
            hook.windows.remove(&self.window);
            if hook.windows.is_empty() {
                unsafe { UnhookWindowsHookEx(hook.hook); }
                hooks.remove(&self.thread);
            }
        }
    }
}
unsafe extern "system" fn callback(code:c_int, wparam:WPARAM, lparam:LPARAM) -> isize {
    if code == HC_ACTION as i32 && wparam & PM_REMOVE as usize != 0 {
        let msg = &mut *(lparam as *mut MSG);
        // Alt/menu messages remain with the host.
        if matches!(msg.message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
            let ours = HOOKS.lock().unwrap_or_else(|e|e.into_inner())
                .get(&GetCurrentThreadId()).is_some_and(|h| h.windows.contains(&(msg.hwnd as isize)));
            // Never hold the registry lock across native dispatch: callbacks can close a dialog.
            if ours {
                if msg.message == WM_KEYDOWN { TranslateMessage(msg); }
                wnd_proc::<BaseviewWindow>(msg.hwnd, msg.message, msg.wParam, msg.lParam);
                msg.message=WM_NULL; msg.hwnd=ptr::null_mut(); msg.wParam=0; msg.lParam=0;
            }
        }
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}
