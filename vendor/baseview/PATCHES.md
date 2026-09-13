This is RustAudio/baseview at commit
`2c1b1a7b0fef1a29a5150a6a8f6fef6a0cbab8c4`, the revision pinned by
gainstagefx's NIH-plug/Vizia GUI. Original MIT and Apache licenses are retained.

Local changes fix Windows mouse capture and reentrant UI/resize dispatch in
embedded plugin windows:

- `src/win/mouse.rs` tracks individual buttons instead of a counter that can
  become stale after a missed release or duplicate press.
- `src/win/window.rs` handles `WM_CAPTURECHANGED`, `WM_CANCELMODE` and focus
  loss. It emits releases for held buttons so Vizia closes the active drag and
  host parameter gesture. Normal mouse-up clears bookkeeping before native
  `ReleaseCapture`, whose synchronous capture-change message must not emit a
  duplicate release. Capture is released only if this window still owns it.
- The existing UI frame timer checks native capture and physical button state
  while a button is held. It handles missing mouse-up without requiring another
  click, including swapped primary/secondary buttons. This does not add a
  thread or touch audio processing.
- Event delivery uses a FIFO queue and `try_borrow_mut` so native messages sent
  during `on_event` or `on_frame` cannot reenter the borrowed window handler.
  This includes capture cancellation, cursor boundary events, and host-driven
  `WM_SIZE`. Pending releases precede later input. A posted wake-up also covers
  cancellation during callbacks outside the normal window message dispatch.
- Nested `WM_TIMER` ticks skip rendering until the current callback returns.
  Deferred resize tasks run only outside callbacks, and their drain cannot
  reenter itself through `SetWindowPos`. Otherwise native messages can execute
  a later resize before an earlier one completes, leaving the old size last.
- Each native dispatch retains an `Rc` until it returns. Nested destruction
  releases the HWND's reference without deallocating an active callback's state.
- Cursor boundary tracking emits `CursorEntered`/`CursorLeft` before movement,
  including leaving and reentering while native capture is held. `WM_MOUSELEAVE`
  handles uncaptured departures. Vizia clears the root's `OVER` flag outside the
  window and requires an enter event to resume hit-testing; repairing mouse-up
  alone did not restore this state.
- Deferred resizing leaves the committed `WindowInfo` unchanged until `WM_SIZE`
  reports the actual client rectangle. Updating it before `SetWindowPos` caused
  the resize notification to be suppressed, leaving Vizia's canvas/layout at
  the old size until reopening.
- `src/win/window_tests.rs` exercises hidden native windows and actual Windows
  message dispatch. `src/lib.rs` also enables the platform-independent button
  tests on Linux. `src/win/mod.rs` declares the new helper.

All other upstream implementation files, including Linux/macOS window handling,
are unchanged. The root Cargo patch also unifies NIH-plug's optional standalone
backend onto this revision; the standalone feature is compile-checked. The
plugin GUI itself already used this revision before the patch.

Validation commands:

```sh
cargo test --manifest-path vendor/baseview/Cargo.toml --lib
cargo test --manifest-path vendor/baseview/Cargo.toml --target x86_64-pc-windows-gnu --lib --no-run
# Run the resulting Windows test executable on Windows/Wine:
# baseview-<hash>.exe --test-threads=1
cargo check --target x86_64-pc-windows-gnu --lib
cargo check --features standalone
```

Windows behavior follows Microsoft's documented
[capture-loss notification](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-capturechanged)
and [physical button state](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate).
Native tests were run through Wine; testing a drag in Windows REAPER remains a
host integration check, distinct from these message-dispatch regressions.

The boundary and resize regressions cover all four client edges during a
held drag, reentry without capture, and repeated zoom changes at 100% and 150%
display scaling. Additional regressions reproduce a host resize during a frame,
nested mouse/timer messages, deferred resize ordering, and nested destruction.
The host-resize test aborted with `RefCell already borrowed` before the dispatch
fix. The ordering test observed 300x150 followed by 250x125 for requests made in
the opposite order before protecting the task drain. Both pass under Wine with
the fixes. The Windows packaging job runs the native backend tests with
OpenGL enabled before bundling. Wine validation does not replace testing the
released VST3/CLAP in the affected Windows host.

Native resize reentrancy follows Microsoft's documented
[window notifications from SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)
and [recursive window-manager calls](https://support.microsoft.com/en-au/topic/recursive-calls-to-window-manager-functions-may-fail-unexpectedly-43ad67f3-44a6-7f86-622a-a55f169ce5ac).
