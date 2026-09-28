# Local baseview changes

Upstream: RustAudio/baseview `0fdebac0821370c2604005b8a81d6d96d714018d`
(manifest 0.3.4), inspected 2026-09-26. This replaces the old 0.1-era fork.

Only the following source changes are carried:

- Clone non-Copy `GlConfig` when constructing Windows, EGL and GLX contexts.
  The upstream revision otherwise fails to compile with OpenGL enabled.
- Track pressed Windows mouse buttons individually. Balance releases on capture
  loss, cancellation, lost focus and missed mouse-up; commit state before calling
  native APIs that can reenter the window procedure. Preserve entry/exit events
  while dragging with capture, including swapped mouse buttons.
- Scope the Windows keyboard hook to active text entry. `WindowContext::set_text_input`
  enables it for the preset name field and restores native focus afterwards.
  Host shortcuts outside the field, and system/Alt key messages, remain with the host.
- Report the restored size to the Windows handler if a host rejects a resize.
- Allow fixed-layout X11 editors to opt out of implicit parent-size following.
  REAPER caps its containing pane at the screen height even when it accepts a
  larger editor. Treating that viewport as the editor's size changed 150–200%
  zoom into roughly 145%. Explicit host resize requests still take effect.
- On X11, rebuild ancestry starting at the reparented window itself, refreshing
  its cached map state and subscribing to both its own and its children's
  structure events. REAPER maps/reparents intermediate FX-chain containers;
  retaining their old hidden state permanently suppresses editor drawing.
- Start the X11 frame source from the resulting visibility state, including
  after reparenting. Restart the notification chain after hiding, ignore old
  Present replies, and start at most one fallback timer. Mapping prematurely in
  `Show` or relying only on the `MapNotify` transition misses these host cases.

X11 teardown also skips `Show` after the host has requested shutdown, ensuring
that dropping the window joins its GUI thread rather than returning early on a
disconnected request channel.

The old dispatch-lifetime patch is superseded by upstream's owned window data.
Reentrant Vizia dispatch is handled by the GUI backend's ordered pending queue,
not by duplicating the previous window procedure patch.

Validation:

- `cargo test --manifest-path vendor/baseview/Cargo.toml --features opengl,tracing --lib`
  exercises button bookkeeping and native Windows capture/boundary events.
- `tools/linux_gui_smoke.py` loads the real CLAP binary through its C ABI, parents
  and shows the X11 child, checks rendered pixels, operates the zoom menu, hides
  and shows it, and destroys/recreates the editor. The original X11 code fails
  this check with a one-color window; the patched code passes.
- The Linux package job runs the native test under Xvfb/software OpenGL.

This is not a claim that all upstream Windows/macOS host combinations have been
validated. Keep these changes until equivalent behavior is demonstrated upstream.
