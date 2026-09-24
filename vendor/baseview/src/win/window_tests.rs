//! Native message-dispatch regressions. Run on Windows (or Wine with a window
//! system), with --test-threads=1. Windows stay hidden; no audio host is needed.
use super::*;
use crate::EventStatus;
use winapi::um::winuser::SendMessageW;

struct Recorder(
    Rc<RefCell<Vec<MouseEvent>>>,
    Rc<RefCell<Vec<WindowInfo>>>,
    Rc<RefCell<Vec<keyboard_types::KeyboardEvent>>>,
);
impl WindowHandler for Recorder {
    fn on_frame(&mut self, _: &mut crate::Window) {}
    fn on_event(&mut self, _: &mut crate::Window, event: Event) -> EventStatus {
        match event {
            Event::Mouse(event) => self.0.borrow_mut().push(event),
            Event::Window(WindowEvent::Resized(info)) => self.1.borrow_mut().push(info),
            Event::Keyboard(event) => self.2.borrow_mut().push(event),
            _ => {}
        }
        EventStatus::Captured
    }
}

struct Fixture {
    state: Rc<WindowState>,
    events: Rc<RefCell<Vec<MouseEvent>>>,
    sizes: Rc<RefCell<Vec<WindowInfo>>>,
    keys: Rc<RefCell<Vec<keyboard_types::KeyboardEvent>>>,
}

impl Fixture {
    fn new() -> Self {
        Self::inside(null_mut())
    }

    /// Embedded in `parent`, as a plugin's window is in the host's.
    fn inside(parent: HWND) -> Self {
        unsafe {
            let class = register_wnd_class();
            assert_ne!(class, 0);
            let style = if parent.is_null() { winapi::um::winuser::WS_POPUP } else { WS_CHILD };
            // No WS_VISIBLE: exercise native capture/message dispatch without
            // putting test windows on the user's desktop.
            let hwnd = CreateWindowExW(0, class as _, [0u16].as_ptr(), style,
                0, 0, 200, 100, parent, null_mut(), null_mut(), null_mut());
            assert!(!hwnd.is_null(), "native test window could not be created");
            let events = Rc::new(RefCell::new(Vec::new()));
            let sizes = Rc::new(RefCell::new(Vec::new()));
            let keys = Rc::new(RefCell::new(Vec::new()));
            let state = Rc::new(WindowState {
                hwnd, window_class: class,
                window_info: RefCell::new(WindowInfo::from_logical_size(Size::new(200.0, 100.0), 1.0)),
                _parent_handle: None,
                keyboard_state: RefCell::new(KeyboardState::new()),
                text_input: TextInput::new(),
                pressed_buttons: Cell::new(Buttons::default()),
                pending_releases: Cell::new(Buttons::default()),
                cursor_inside: Cell::new(false),
                handler: RefCell::new(Some(Box::new(Recorder(Rc::clone(&events), Rc::clone(&sizes), Rc::clone(&keys))))),
                pending_events: RefCell::new(VecDeque::new()),
                _drop_target: RefCell::new(None),
                scale_policy: WindowScalePolicy::ScaleFactor(1.0),
                dw_style: style,
                deferred_tasks: RefCell::new(VecDeque::new()),
                draining_tasks: Cell::new(false),
                #[cfg(feature = "opengl")]
                gl_context: None,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Rc::into_raw(Rc::clone(&state)) as _);
            Self { state, events, sizes, keys }
        }
    }

    fn send(&self, message: UINT, wparam: WPARAM, lparam: LPARAM) {
        unsafe { SendMessageW(self.state.hwnd, message, wparam, lparam); }
    }

    fn releases(&self) -> Vec<MouseButton> {
        self.events.borrow().iter().filter_map(|e| match e {
            MouseEvent::ButtonReleased { button, .. } => Some(*button), _ => None,
        }).collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) { unsafe { DestroyWindow(self.state.hwnd); } }
}

struct DuringFrame {
    recorder: Recorder,
    action: Box<dyn FnMut(&mut crate::Window)>,
}

impl WindowHandler for DuringFrame {
    fn on_frame(&mut self, window: &mut crate::Window) { (self.action)(window); }
    fn on_event(&mut self, window: &mut crate::Window, event: Event) -> EventStatus {
        self.recorder.on_event(window, event)
    }
}

impl Fixture {
    fn during_frame(&self, action: impl FnMut(&mut crate::Window) + 'static) {
        *self.state.handler.borrow_mut() = Some(Box::new(DuringFrame {
            recorder: Recorder(self.events.clone(), self.sizes.clone(), self.keys.clone()),
            action: Box::new(action),
        }));
    }
}

#[test]
fn host_resize_during_a_frame_is_delivered_after_the_callback() {
    let window = Fixture::new();
    let hwnd = window.state.hwnd;
    let sizes = window.sizes.clone();
    window.during_frame(move |_| unsafe {
        // A host can synchronously resize the child from request_resize().
        assert_ne!(SetWindowPos(hwnd, null_mut(), 0, 0, 300, 150,
            SWP_NOZORDER | SWP_NOMOVE), 0);
        assert!(sizes.borrow().is_empty(), "must not reenter the renderer");
    });
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(window.sizes.borrow().len(), 1);
    assert_eq!(window.sizes.borrow()[0].physical_size(), PhySize::new(300, 150));
}

#[test]
fn nested_message_does_not_run_a_deferred_resize_inside_a_callback() {
    let window = Fixture::new();
    let hwnd = window.state.hwnd;
    let sizes = window.sizes.clone();
    window.during_frame(move |window| unsafe {
        window.resize(Size::new(250.0, 125.0));
        // Native APIs can send even an unrelated message before on_frame ends.
        SendMessageW(hwnd, WM_USER + 99, 0, 0);
        assert!(sizes.borrow().is_empty());
    });
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(window.sizes.borrow().len(), 1);
    assert_eq!(window.sizes.borrow()[0].physical_size(), PhySize::new(250, 125));
}

#[test]
fn queued_resizes_finish_in_request_order() {
    let window = Fixture::new();
    window.during_frame(|window| {
        window.resize(Size::new(250.0, 125.0));
        window.resize(Size::new(300.0, 150.0));
    });
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    let sizes: Vec<_> = window.sizes.borrow().iter().map(|info| info.physical_size()).collect();
    assert_eq!(sizes, [PhySize::new(250, 125), PhySize::new(300, 150)]);
    assert_eq!(window.state.window_info.borrow().physical_size(), PhySize::new(300, 150));
}

#[test]
fn destruction_during_a_callback_keeps_dispatch_state_alive() {
    let window = Fixture::new();
    let hwnd = window.state.hwnd;
    let weak = Rc::downgrade(&window.state);
    window.during_frame(move |_| unsafe {
        assert_ne!(DestroyWindow(hwnd), 0);
        // The fixture owns one reference. The native callback must own another
        // after WM_NCDESTROY releases the reference formerly held by the HWND.
        assert!(weak.strong_count() > 1);
    });
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(Rc::strong_count(&window.state), 1, "dispatch reference leaked");
}

#[test]
fn nested_mouse_and_timer_messages_preserve_input_and_resume_frames() {
    let window = Fixture::new();
    let hwnd = window.state.hwnd;
    let frames = Rc::new(Cell::new(0));
    let count = frames.clone();
    window.during_frame(move |_| {
        count.set(count.get() + 1);
        if count.get() == 1 {
            unsafe {
                SendMessageW(hwnd, WM_MOUSEMOVE, 0, position(20, 20));
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, position(20, 20));
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, position(240, 30));
                SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, position(240, 30));
                SendMessageW(hwnd, WM_TIMER, WIN_FRAME_TIMER, 0);
                SendMessageW(hwnd, WM_MOUSEMOVE, 0, position(20, 20));
            }
        }
    });
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(frames.get(), 1, "nested timer must not start another frame");
    assert_eq!(window.releases(), [MouseButton::Left]);
    let crossings: Vec<_> = window.events.borrow().iter().filter_map(|event| match event {
        MouseEvent::CursorEntered => Some(true),
        MouseEvent::CursorLeft => Some(false),
        _ => None,
    }).collect();
    assert_eq!(crossings, [true, false, true]);
    assert!(unsafe { GetCapture() }.is_null());
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(frames.get(), 2, "frame processing must recover");
}

#[test]
fn release_outside_bounds_ends_capture_once() {
    let window = Fixture::new();
    window.send(WM_LBUTTONDOWN, 1, 0);
    assert_eq!(unsafe { GetCapture() }, window.state.hwnd);
    let outside = ((-100i16 as u16 as usize) << 16 | (-40i16 as u16 as usize)) as LPARAM;
    window.send(WM_MOUSEMOVE, 1, outside);
    window.send(WM_LBUTTONUP, 0, outside);
    assert_eq!(window.releases(), [MouseButton::Left]);
    assert!(window.state.pressed_buttons.get().is_empty());
    assert!(unsafe { GetCapture() }.is_null());
    assert!(window.events.borrow().iter().any(|e| matches!(e,
        MouseEvent::CursorMoved { position, .. } if position.x == -40.0 && position.y == -100.0)));
}

#[test]
fn stolen_capture_releases_all_buttons_without_releasing_the_new_owner() {
    let first = Fixture::new();
    let second = Fixture::new();
    first.send(WM_LBUTTONDOWN, 1, 0);
    first.send(WM_RBUTTONDOWN, 3, 0);
    unsafe { SetCapture(second.state.hwnd); }
    assert_eq!(first.releases(), [MouseButton::Left, MouseButton::Right]);
    assert!(first.state.pressed_buttons.get().is_empty());
    first.send(WM_LBUTTONUP, 0, 0); // Late native release must be harmless.
    assert_eq!(first.releases().len(), 2);
    assert_eq!(unsafe { GetCapture() }, second.state.hwnd);
}

#[test]
fn capture_loss_during_a_callback_defers_instead_of_reborrowing_the_handler() {
    let first = Fixture::new();
    let second = Fixture::new();
    first.send(WM_LBUTTONDOWN, 1, 0);
    let borrowed = first.state.handler.borrow_mut();
    unsafe { SetCapture(second.state.hwnd); }
    assert!(first.releases().is_empty());
    assert!(!first.state.pending_events.borrow().is_empty());
    drop(borrowed);
    first.send(BV_RELEASE_LOST_BUTTONS, 0, 0);
    assert_eq!(first.releases(), [MouseButton::Left]);
    assert!(first.state.pending_events.borrow().is_empty());
}

#[test]
fn cancel_mode_is_idempotent_and_the_next_drag_works() {
    let window = Fixture::new();
    window.send(WM_LBUTTONDOWN, 1, 0);
    window.send(WM_CANCELMODE, 0, 0);
    window.send(WM_CANCELMODE, 0, 0);
    assert_eq!(window.releases(), [MouseButton::Left]);
    window.send(WM_LBUTTONDOWN, 1, 0);
    window.send(WM_LBUTTONUP, 0, 0);
    assert_eq!(window.releases(), [MouseButton::Left, MouseButton::Left]);
    assert!(unsafe { GetCapture() }.is_null());
}

#[test]
fn frame_recovers_a_missing_up_without_another_mouse_event() {
    let window = Fixture::new();
    // SendMessage does not hold the physical button, reproducing the stale
    // cached-down state after a native mouse-up was lost outside the window.
    window.send(WM_LBUTTONDOWN, 1, 0);
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
    assert_eq!(window.releases(), [MouseButton::Left]);
    assert!(window.state.pressed_buttons.get().is_empty());
    assert!(unsafe { GetCapture() }.is_null());
}

fn position(x: i16, y: i16) -> LPARAM {
    ((y as u16 as usize) << 16 | x as u16 as usize) as LPARAM
}

#[test]
fn captured_drag_leaves_and_reenters_before_mouse_up() {
    let window = Fixture::new();
    window.send(WM_MOUSEMOVE, 0, position(20, 20));
    window.send(WM_LBUTTONDOWN, 1, position(20, 20));
    // Exercise both positive and negative out-of-client coordinates.
    for (x, y) in [(240, 30), (30, 140), (-40, 30), (30, -40)] {
        window.send(WM_MOUSEMOVE, 1, position(x, y));
        assert_eq!(unsafe { GetCapture() }, window.state.hwnd);
        window.send(WM_MOUSEMOVE, 1, position(20, 20));
    }
    assert!(window.releases().is_empty(), "leaving while held must not end the drag");
    window.send(WM_LBUTTONUP, 0, position(20, 20));
    let crossings: Vec<_> = window.events.borrow().iter().filter_map(|event| match event {
        MouseEvent::CursorEntered => Some(true),
        MouseEvent::CursorLeft => Some(false),
        _ => None,
    }).collect();
    assert_eq!(crossings, [true, false, true, false, true, false, true, false, true]);
    assert_eq!(window.releases(), [MouseButton::Left]);
}

#[test]
fn uncaptured_leave_allows_the_next_enter() {
    let window = Fixture::new();
    window.send(WM_MOUSEMOVE, 0, position(20, 20));
    window.send(winapi::um::winuser::WM_MOUSELEAVE, 0, 0);
    window.send(WM_MOUSEMOVE, 0, position(30, 30));
    let crossings: Vec<_> = window.events.borrow().iter().filter_map(|event| match event {
        MouseEvent::CursorEntered => Some(true),
        MouseEvent::CursorLeft => Some(false),
        _ => None,
    }).collect();
    assert_eq!(crossings, [true, false, true]);
}

#[test]
fn requested_resize_notifies_the_renderer_and_matches_the_native_client() {
    use winapi::um::winuser::GetClientRect;
    let window = Fixture::new();
    // Includes a non-integer display scale, separate from the user's zoom.
    for dpi in [1.0, 1.5] {
        *window.state.window_info.borrow_mut() =
            WindowInfo::from_logical_size(Size::new(200.0, 100.0), dpi);
        for zoom in [1.25, 0.75, 1.0] {
            window.sizes.borrow_mut().clear();
            let requested = Size::new(200.0 * zoom, 100.0 * zoom);
            let expected = WindowInfo::from_logical_size(requested, dpi);
            window.state.create_window().resize(requested);
            // Drain the real deferred resize path, outside the handler borrow.
            window.send(WM_USER + 99, 0, 0);
            let sizes = window.sizes.borrow();
            assert_eq!(sizes.len(), 1, "renderer missed the resize at DPI {dpi}, zoom {zoom}");
            assert_eq!(sizes[0].physical_size(), expected.physical_size());
            assert_eq!(sizes[0].scale(), dpi);
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            assert_ne!(unsafe { GetClientRect(window.state.hwnd, &mut rect) }, 0);
            assert_eq!(rect.right - rect.left, expected.physical_size().width as i32);
            assert_eq!(rect.bottom - rect.top, expected.physical_size().height as i32);
        }
    }
}

// ---------------------------------------------------------------------------
// Keyboard input past a host that filters it
// ---------------------------------------------------------------------------

use keyboard_types::{Key, KeyState};
use winapi::um::winuser::{PeekMessageW, SetFocus, PM_REMOVE};

/// Type letters at `hwnd` through a host's message loop, reduced to the one
/// habit that matters: it translates and dispatches key-downs as usual, but
/// keeps every `WM_CHAR` for itself instead of dispatching it -- as a host
/// does that takes letters for its own shortcuts. Each key is released only
/// after its press has been handled, as a finger would. Returns the keyboard
/// messages the host's loop saw.
fn type_through_host(hwnd: HWND, letters: &[u8]) -> Vec<UINT> {
    let mut seen = Vec::new();
    for &letter in letters {
        // Set 1 scan codes, which are not in alphabetical order.
        let scan: LPARAM = match letter {
            b'A' => 0x1E,
            b'B' => 0x30,
            b'C' => 0x2E,
            _ => unreachable!(),
        };
        let down = 1 | (scan << 16);
        let up = down | (1 << 30) | (1 << 31);
        for (message, lparam) in [(WM_KEYDOWN, down), (WM_KEYUP, up)] {
            unsafe { PostMessageW(hwnd, message, letter as WPARAM, lparam); }
            host_loop(&mut seen);
        }
    }
    seen
}

fn host_loop(seen: &mut Vec<UINT>) {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            if matches!(msg.message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
                seen.push(msg.message);
            }
            if msg.message == WM_CHAR {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// The text the window was given, as a text field would insert it.
fn typed(window: &Fixture) -> String {
    window
        .keys
        .borrow()
        .iter()
        .filter(|e| e.state == KeyState::Down)
        .filter_map(|e| match &e.key {
            Key::Character(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

/// Open or close the window's text field from inside a frame callback, which
/// is where a toolkit's event handlers run.
fn text_field(window: &Fixture, open: bool) {
    window.during_frame(move |_| crate::set_text_input(open));
    window.send(WM_TIMER, WIN_FRAME_TIMER, 0);
}

/// The reported fault, reproduced: with no text field open, nothing changes,
/// and under this host every letter is lost -- while a key that makes no
/// `WM_CHAR`, like Delete, would get through.
#[test]
fn a_host_that_keeps_characters_loses_every_letter() {
    let window = Fixture::new();
    let seen = type_through_host(window.state.hwnd, b"A");
    assert_eq!(seen, [WM_KEYDOWN, WM_CHAR, WM_KEYUP], "the host must see the character");
    assert_eq!(typed(&window), "", "the key-down was held back for a WM_CHAR that never came");
}

#[test]
fn an_open_text_field_gets_its_letters_past_a_host_that_keeps_characters() {
    let window = Fixture::new();
    text_field(&window, true);
    let seen = type_through_host(window.state.hwnd, b"CAB");
    assert_eq!(typed(&window), "cab");
    assert!(seen.is_empty(), "the host saw {seen:?} while the field was open");
}

#[test]
fn closing_the_text_field_gives_the_host_its_keys_back() {
    let window = Fixture::new();
    text_field(&window, true);
    text_field(&window, false);
    let seen = type_through_host(window.state.hwnd, b"A");
    assert_eq!(seen, [WM_KEYDOWN, WM_CHAR, WM_KEYUP]);
    assert_eq!(typed(&window), "");
}

#[test]
fn only_the_window_with_the_open_field_takes_its_keys() {
    let typing = Fixture::new();
    let other = Fixture::new();
    text_field(&typing, true);
    let seen = type_through_host(other.state.hwnd, b"A");
    assert_eq!(seen, [WM_KEYDOWN, WM_CHAR, WM_KEYUP]);
}

#[test]
fn outside_a_callback_there_is_no_window_to_open_a_field_in() {
    let window = Fixture::new();
    crate::set_text_input(true);
    assert_eq!(super::super::text_input::hook_users(), 0);
    let seen = type_through_host(window.state.hwnd, b"A");
    assert_eq!(seen, [WM_KEYDOWN, WM_CHAR, WM_KEYUP]);
}

#[test]
fn the_text_field_takes_the_focus_and_gives_it_back() {
    // Stands in for the host's window, which had the keyboard first.
    let host = Fixture::new();
    let window = Fixture::inside(host.state.hwnd);
    unsafe { SetFocus(host.state.hwnd) };
    assert_eq!(unsafe { GetFocus() }, host.state.hwnd);

    text_field(&window, true);
    assert_eq!(unsafe { GetFocus() }, window.state.hwnd, "keys go to the focused window");

    // Using the host and clicking back into the field brings the keyboard back.
    unsafe { SetFocus(host.state.hwnd) };
    window.send(WM_LBUTTONDOWN, 0, position(5, 5));
    window.send(WM_LBUTTONUP, 0, position(5, 5));
    assert_eq!(unsafe { GetFocus() }, window.state.hwnd);

    text_field(&window, false);
    assert_eq!(unsafe { GetFocus() }, host.state.hwnd, "the host's shortcuts need it back");
}

#[test]
fn the_hook_lasts_exactly_as_long_as_an_open_field_needs_it() {
    use super::super::text_input::hook_users;
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(hook_users(), 0);
    text_field(&first, true);
    assert_eq!(hook_users(), 1);
    text_field(&first, true);
    assert_eq!(hook_users(), 1, "opening an open field again is not a second user");
    text_field(&second, true);
    assert_eq!(hook_users(), 2, "one hook per thread, shared");
    text_field(&first, false);
    assert_eq!(hook_users(), 1, "the second field is still open");
    // A window closed with its field still open must not leave the hook
    // behind: it would outlive the plugin whose code it points into.
    drop(second);
    assert_eq!(hook_users(), 0);
}
