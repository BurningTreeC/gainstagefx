//! Native message-dispatch regressions. Run on Windows (or Wine with a window
//! system), with --test-threads=1. Windows stay hidden; no audio host is needed.
use super::*;
use crate::EventStatus;
use winapi::um::winuser::SendMessageW;

struct Recorder(Rc<RefCell<Vec<MouseEvent>>>, Rc<RefCell<Vec<WindowInfo>>>);
impl WindowHandler for Recorder {
    fn on_frame(&mut self, _: &mut crate::Window) {}
    fn on_event(&mut self, _: &mut crate::Window, event: Event) -> EventStatus {
        match event {
            Event::Mouse(event) => self.0.borrow_mut().push(event),
            Event::Window(WindowEvent::Resized(info)) => self.1.borrow_mut().push(info),
            _ => {}
        }
        EventStatus::Captured
    }
}

struct Fixture {
    state: Rc<WindowState>,
    events: Rc<RefCell<Vec<MouseEvent>>>,
    sizes: Rc<RefCell<Vec<WindowInfo>>>,
}

impl Fixture {
    fn new() -> Self {
        unsafe {
            let class = register_wnd_class();
            assert_ne!(class, 0);
            // No WS_VISIBLE: exercise native capture/message dispatch without
            // putting test windows on the user's desktop.
            let hwnd = CreateWindowExW(0, class as _, [0u16].as_ptr(), winapi::um::winuser::WS_POPUP,
                0, 0, 200, 100, null_mut(), null_mut(), null_mut(), null_mut());
            assert!(!hwnd.is_null(), "native test window could not be created");
            let events = Rc::new(RefCell::new(Vec::new()));
            let sizes = Rc::new(RefCell::new(Vec::new()));
            let state = Rc::new(WindowState {
                hwnd, window_class: class,
                window_info: RefCell::new(WindowInfo::from_logical_size(Size::new(200.0, 100.0), 1.0)),
                _parent_handle: None,
                keyboard_state: RefCell::new(KeyboardState::new()),
                pressed_buttons: Cell::new(Buttons::default()),
                pending_releases: Cell::new(Buttons::default()),
                cursor_inside: Cell::new(false),
                handler: RefCell::new(Some(Box::new(Recorder(Rc::clone(&events), Rc::clone(&sizes))))),
                pending_events: RefCell::new(VecDeque::new()),
                _drop_target: RefCell::new(None),
                scale_policy: WindowScalePolicy::ScaleFactor(1.0),
                dw_style: winapi::um::winuser::WS_POPUP,
                deferred_tasks: RefCell::new(VecDeque::new()),
                draining_tasks: Cell::new(false),
                #[cfg(feature = "opengl")]
                gl_context: None,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Rc::into_raw(Rc::clone(&state)) as _);
            Self { state, events, sizes }
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
            recorder: Recorder(self.events.clone(), self.sizes.clone()),
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
