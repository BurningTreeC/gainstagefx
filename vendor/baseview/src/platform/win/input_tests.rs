//! Native regression tests. Run serially on the Windows CI runner.
use crate::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{Arc, Mutex};
use windows_sys::Win32::{Foundation::{HWND, LPARAM}, UI::WindowsAndMessaging::*};

struct Recorder { events: Arc<Mutex<Vec<Event>>> }
impl WindowHandler for Recorder {
    fn on_frame(&self) -> Result<(), HandlerError> { Ok(()) }
    fn resized(&self, _: WindowSize) -> Result<(), HandlerError> { Ok(()) }
    fn on_event(&self, event: Event) -> EventStatus {
        self.events.lock().unwrap().push(event); EventStatus::Ignored
    }
}
fn window() -> (Window, HWND, Arc<Mutex<Vec<Event>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let event_sink = events.clone();
    let hwnd = Arc::new(Mutex::new(0isize));
    let hwnd_sink = hwnd.clone();
    let window = Window::create(WindowSettings::new(), move |cx| {
        let RawWindowHandle::Win32(handle) = cx.window_handle().unwrap().as_raw() else { panic!("Win32 handle") };
        *hwnd_sink.lock().unwrap() = handle.hwnd.get();
        Ok(Recorder { events: event_sink })
    }).unwrap();
    let native = *hwnd.lock().unwrap() as HWND;
    assert!(!native.is_null());
    (window, native, events)
}
#[test]
fn capture_cancellation_finishes_each_pressed_button_once() {
    let (_window, hwnd, events) = window();
    unsafe {
        SendMessageW(hwnd, WM_LBUTTONDOWN, 0, 0);
        SendMessageW(hwnd, WM_LBUTTONDOWN, 0, 0);
        SendMessageW(hwnd, WM_RBUTTONDOWN, 0, 0);
        SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
        SendMessageW(hwnd, WM_CAPTURECHANGED, 0, 0);
    }
    let releases: Vec<_> = events.lock().unwrap().iter().filter_map(|e| match e {
        Event::Mouse(MouseEvent::ButtonReleased { button, .. }) => Some(*button), _ => None
    }).collect();
    assert_eq!(releases, [MouseButton::Left, MouseButton::Right]);
}
#[test]
fn captured_cursor_reports_leaving_and_reentering_client_bounds() {
    let (_window, hwnd, events) = window();
    unsafe {
        SendMessageW(hwnd, WM_MOUSEMOVE, 0, (10 << 16) | 10);
        SendMessageW(hwnd, WM_LBUTTONDOWN, 0, 0);
        SendMessageW(hwnd, WM_MOUSEMOVE, 0, ((10 << 16) | 0xfff6) as LPARAM);
        SendMessageW(hwnd, WM_MOUSEMOVE, 0, (10 << 16) | 10);
        SendMessageW(hwnd, WM_LBUTTONUP, 0, 0);
    }
    let crossings: Vec<_> = events.lock().unwrap().iter().filter_map(|e| match e {
        Event::Mouse(MouseEvent::CursorEntered) => Some(true),
        Event::Mouse(MouseEvent::CursorLeft) => Some(false), _ => None
    }).collect();
    assert_eq!(crossings, [true, false, true]);
}
