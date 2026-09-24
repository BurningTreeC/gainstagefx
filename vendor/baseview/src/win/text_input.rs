//! Typing into a text field inside a host that filters the keyboard.
//!
//! Win32 posts a keystroke to the focused window's thread queue, and it is the
//! *host's* message loop that takes it out again. So a host sees every key
//! addressed to a plugin window before the plugin does, and many act on it:
//! they translate and dispatch the key-down, then keep the `WM_CHAR` it made
//! for themselves, because letters are their shortcuts. `keyboard.rs` holds
//! each key-down back until the `WM_CHAR` behind it arrives -- that is how it
//! learns which character the key made -- so under such a host every letter is
//! lost, while Delete and the arrows, which make no `WM_CHAR`, still work.
//!
//! While a text field is open, this installs a `WH_GETMESSAGE` hook on the
//! window's thread. The hook runs inside the host's own `GetMessage` or
//! `PeekMessage`, before the host has the message, and hands keystrokes
//! addressed to that window straight to it, leaving the host a `WM_NULL`. The
//! technique is JUCE's, and upstream baseview's since RustAudio/baseview#212,
//! with one difference: upstream keeps the hook for as long as any window is
//! open, which takes every key -- the space bar driving a host's transport
//! included -- whenever the plugin has focus. Here it exists only between
//! `set_text_input(true)` and `set_text_input(false)`.

use std::cell::Cell;
use std::ffi::c_int;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, MutexGuard};

use winapi::shared::minwindef::{DWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HHOOK, HWND};
use winapi::um::processthreadsapi::GetCurrentThreadId;
use winapi::um::winuser::{
    CallNextHookEx, GetFocus, GetWindowLongPtrW, IsWindow, SetFocus, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, GWLP_USERDATA, GWLP_WNDPROC, HC_ACTION, MSG,
    PM_REMOVE, WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_NULL,
};

use super::window::{wnd_proc, WindowState};

thread_local! {
    /// The window whose message this thread is handling, if any.
    ///
    /// `set_text_input` is called from inside a window handler, which cannot
    /// name its native window, so this is how the call finds it. A raw pointer
    /// needs no destructor, so this registers none -- which matters in a plugin
    /// DLL that can be unloaded while the host's thread lives on.
    static DISPATCHING: Cell<*const WindowState> = const { Cell::new(null()) };
}

/// Marks a window as the one being handled until dropped. Nests: a message
/// sent to another window during a callback restores this one afterwards.
pub(super) struct Dispatching(*const WindowState);

impl Dispatching {
    pub(super) fn enter(state: &WindowState) -> Self {
        Self(DISPATCHING.with(|current| current.replace(state)))
    }
}

impl Drop for Dispatching {
    fn drop(&mut self) {
        DISPATCHING.with(|current| current.set(self.0));
    }
}

/// See [`crate::set_text_input`].
pub fn set_text_input(active: bool) {
    let state = DISPATCHING.with(|current| current.get());
    if !state.is_null() {
        // Alive: the `wnd_proc` that set it holds a reference until it returns.
        unsafe { (*state).set_text_input(active) };
    }
}

/// A window's text-entry state.
pub(super) struct TextInput {
    active: Cell<bool>,
    /// This window's share of its thread's hook, held while `active`.
    hook: Cell<Option<HookRef>>,
    /// Where keyboard focus was before the text field took it.
    returned_to: Cell<HWND>,
}

impl TextInput {
    pub(super) fn new() -> Self {
        Self { active: Cell::new(false), hook: Cell::new(None), returned_to: Cell::new(null_mut()) }
    }

    pub(super) fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Returns whether anything changed.
    pub(super) fn set(&self, active: bool) -> bool {
        if self.active.replace(active) == active {
            return false;
        }
        // Dropping the old value releases this window's share of the hook.
        self.hook.set(if active { unsafe { HookRef::acquire() } } else { None });
        true
    }

    /// Keystrokes go to the focused window, and the hook only takes those
    /// addressed to this one -- so a text field needs the focus, whether or
    /// not the host would have given it.
    pub(super) unsafe fn take_focus(&self, hwnd: HWND) {
        let previous = GetFocus();
        if !self.active.get() || previous == hwnd {
            return;
        }
        // Read before, not from `SetFocus`'s result: focusing a window whose
        // top-level parent is inactive activates it first, and activation has
        // already moved the focus by the time `SetFocus` reports where it was.
        SetFocus(hwnd);
        // Clicking the host and back into the field keeps the first place to
        // return to.
        if self.returned_to.get().is_null() {
            self.returned_to.set(previous);
        }
    }

    /// Hand the focus back once the field has closed, so that the host's
    /// shortcuts work again without clicking it first.
    pub(super) unsafe fn return_focus(&self, hwnd: HWND) {
        // Reopened since this was queued: it still needs the focus, and the
        // place to return it to later.
        if self.active.get() {
            return;
        }
        let previous = self.returned_to.replace(null_mut());
        if !previous.is_null() && GetFocus() == hwnd && IsWindow(previous) != 0 {
            SetFocus(previous);
        }
    }
}

/// One hook per thread, shared by that thread's open text fields.
struct Installed {
    thread: DWORD,
    hook: HHOOK,
    users: usize,
}

// Only the handle crosses threads, and only to be compared and unhooked.
unsafe impl Send for Installed {}

static INSTALLED: Mutex<Vec<Installed>> = Mutex::new(Vec::new());

fn installed() -> MutexGuard<'static, Vec<Installed>> {
    // A panic here would unwind through `wnd_proc` into the host.
    INSTALLED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A share of the current thread's hook, released on drop. The last one out
/// removes the hook, so none outlives the windows that need it -- a hook left
/// pointing into an unloaded plugin would take the host down with it.
struct HookRef {
    thread: DWORD,
}

impl HookRef {
    unsafe fn acquire() -> Option<Self> {
        let thread = GetCurrentThreadId();
        let mut installed = installed();
        if let Some(entry) = installed.iter_mut().find(|entry| entry.thread == thread) {
            entry.users += 1;
            return Some(Self { thread });
        }
        // A null module is how a hook on a thread of this process is asked for.
        let hook = SetWindowsHookExW(WH_GETMESSAGE, Some(get_message), null_mut(), thread);
        if hook.is_null() {
            // Typing then works exactly as well as it did without this.
            return None;
        }
        installed.push(Installed { thread, hook, users: 1 });
        Some(Self { thread })
    }
}

impl Drop for HookRef {
    fn drop(&mut self) {
        let mut installed = installed();
        if let Some(at) = installed.iter().position(|entry| entry.thread == self.thread) {
            installed[at].users -= 1;
            if installed[at].users == 0 {
                unsafe { UnhookWindowsHookEx(installed.swap_remove(at).hook) };
            }
        }
    }
}

/// How many windows on this thread share its hook; zero when there is none.
#[cfg(test)]
pub(super) fn hook_users() -> usize {
    let thread = unsafe { GetCurrentThreadId() };
    installed().iter().find(|entry| entry.thread == thread).map_or(0, |entry| entry.users)
}

unsafe extern "system" fn get_message(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Only a message actually leaving the queue: a peek that leaves it there
    // will see it again when it is taken.
    if code == HC_ACTION && (wparam as UINT) & PM_REMOVE != 0 {
        let msg = &mut *(lparam as *mut MSG);
        if hand_on(msg) {
            // What the host's loop gets instead: the message that means nothing.
            *msg = MSG { hwnd: null_mut(), message: WM_NULL, wParam: 0, lParam: 0, ..*msg };
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

/// Deliver `msg` to its window if it is a keystroke for an open text field.
unsafe fn hand_on(msg: &MSG) -> bool {
    if !matches!(msg.message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
        return false;
    }
    // Only this copy of baseview's own windows. Another plugin's copy
    // registers a different `wnd_proc`, and its user data is not ours to read.
    let ours = wnd_proc as unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;
    if GetWindowLongPtrW(msg.hwnd, GWLP_WNDPROC) != ours as usize as isize {
        return false;
    }
    let state = GetWindowLongPtrW(msg.hwnd, GWLP_USERDATA) as *const WindowState;
    if state.is_null() || !(*state).text_input.is_active() {
        return false;
    }
    // The host's `TranslateMessage` will now only ever see the `WM_NULL`, so
    // this does its job: the `WM_CHAR` it posts is taken by this hook in turn,
    // with dead keys, Shift and Caps Lock already applied by the layout.
    if msg.message == WM_KEYDOWN {
        TranslateMessage(msg);
    }
    wnd_proc(msg.hwnd, msg.message, msg.wParam, msg.lParam);
    true
}
