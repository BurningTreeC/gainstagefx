This is RustAudio/baseview at commit
`2c1b1a7b0fef1a29a5150a6a8f6fef6a0cbab8c4`, the revision pinned by
gainstagefx's NIH-plug/Vizia GUI. Original MIT and Apache licenses are retained.

Local changes fix Windows mouse capture after dragging outside an embedded
plugin window:

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
- Cancellation delivery uses `try_borrow_mut` and a posted wake-up to avoid
  reentering a borrowed window handler. Pending releases precede later input.
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
