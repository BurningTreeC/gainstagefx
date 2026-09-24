/// Tell the window being handled that a text field in it has opened, or closed.
///
/// On Windows a host's message loop sees every keystroke before the plugin
/// does, and some hosts keep the characters for their own shortcuts. While a
/// text field is open its window takes its keystrokes before the host can,
/// and takes the keyboard focus, handing it back when the field closes; see
/// `win/text_input.rs`. Elsewhere this does nothing.
///
/// Call it from inside a [`WindowHandler`](crate::WindowHandler) callback,
/// which is where a UI toolkit's event handlers run. Anywhere else there is no
/// window for it to apply to, and it does nothing.
pub fn set_text_input(active: bool) {
    #[cfg(target_os = "windows")]
    crate::win::set_text_input(active);
    #[cfg(not(target_os = "windows"))]
    let _ = active;
}
