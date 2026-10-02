# Local Vizia integration changes

Upstream: vizia/vizia `426d2e7b8533d485e78ab7f07f9f81d8324c256d`
(manifest 0.4.0), inspected 2026-09-26.

The baseview backend is ported from baseview 0.2.2 to 0.3.4. It exposes the new
window creation/host/lifecycle API, preserves GL context ownership, and uses the
new native resize callbacks. Vsync remains disabled in the GL configuration;
baseview schedules frames.

Native events and resize callbacks are queued during reentry and drained in
order; nested drawing is skipped. Text-field focus controls the scoped Windows
text-input hook. User zoom is requested through native resize and committed
from the resulting size notification, including asynchronous Linux callbacks
and host rollback. `UserScaleChanged` and `WindowScaleChanged` notify the adapter
and GainStageFx of the resulting geometry.

Synthetic events the context sends itself through the proxy (the mouse move a
relayout sends to refresh hover, with the cursor position at layout time) are
moved into the event queue before each native input event rather than only at
the next frame. With slow frames the stale move otherwise overtook a real one
and the next press landed where the pointer had been: found by PultEQFx's CLAP
editor smoke test under Xvfb, where clicking through the size menu missed.

`NamedKey` is re-exported from the input module for keyboard-types 0.8 callers.
The rendering/reactivity/layout engines are otherwise unchanged. GainStageFx's
own controls use upstream reactive signals and Skia drawing.
