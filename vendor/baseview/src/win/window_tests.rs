//! Native message-dispatch regressions. Run on Windows (or Wine with a window
//! system), with --test-threads=1. Windows stay hidden; no audio host is needed.
use super::*;
use crate::EventStatus;
use winapi::um::winuser::SendMessageW;

struct Recorder(Rc<RefCell<Vec<MouseEvent>>>);
impl WindowHandler for Recorder {
    fn on_frame(&mut self, _: &mut crate::Window) {}
    fn on_event(&mut self, _: &mut crate::Window, event: Event) -> EventStatus {
        if let Event::Mouse(event) = event { self.0.borrow_mut().push(event); }
        EventStatus::Captured
    }
}

struct Fixture {
    state: Rc<WindowState>,
    events: Rc<RefCell<Vec<MouseEvent>>>,
}

impl Fixture {
    fn new() -> Self {
        unsafe {
            let class = register_wnd_class();
            assert_ne!(class, 0);
            // No WS_VISIBLE: exercise native capture/message dispatch without
            // putting test windows on the user's desktop.
            let hwnd = CreateWindowExW(0, class as _, [0u16].as_ptr(), 0,
                0, 0, 200, 100, null_mut(), null_mut(), null_mut(), null_mut());
            assert!(!hwnd.is_null(), "native test window could not be created");
            let events = Rc::new(RefCell::new(Vec::new()));
            let state = Rc::new(WindowState {
                hwnd, window_class: class,
                window_info: RefCell::new(WindowInfo::from_logical_size(Size::new(200.0, 100.0), 1.0)),
                _parent_handle: None,
                keyboard_state: RefCell::new(KeyboardState::new()),
                pressed_buttons: Cell::new(Buttons::default()),
                pending_releases: Cell::new(Buttons::default()),
                handler: RefCell::new(Some(Box::new(Recorder(Rc::clone(&events))))),
                _drop_target: RefCell::new(None),
                scale_policy: WindowScalePolicy::ScaleFactor(1.0),
                dw_style: 0,
                deferred_tasks: RefCell::new(VecDeque::new()),
                #[cfg(feature = "opengl")]
                gl_context: None,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Rc::into_raw(Rc::clone(&state)) as _);
            Self { state, events }
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
    assert!(!first.state.pending_releases.get().is_empty());
    drop(borrowed);
    first.send(BV_RELEASE_LOST_BUTTONS, 0, 0);
    assert_eq!(first.releases(), [MouseButton::Left]);
    assert!(first.state.pending_releases.get().is_empty());
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
