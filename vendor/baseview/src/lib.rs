#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "linux")]
mod x11;

// Keep native button-state regressions runnable on the development platform.
#[cfg(all(test, not(target_os = "windows")))]
#[path = "win/mouse.rs"]
mod windows_mouse_tests;

mod clipboard;
mod event;
mod keyboard;
mod mouse_cursor;
mod window;
mod window_info;
mod window_open_options;

#[cfg(feature = "opengl")]
pub mod gl;

pub use clipboard::*;
pub use event::*;
pub use mouse_cursor::MouseCursor;
pub use window::*;
pub use window_info::*;
pub use window_open_options::*;
