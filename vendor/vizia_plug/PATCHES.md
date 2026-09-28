# Local nice-plug adapter changes

Upstream: vizia/vizia-plug `34812c0ba14c5df39621bab956c74e660220636b`
(manifest 0.1.0), inspected 2026-09-26.

The adapter is ported from nice-plug-core 0.1.4 to 0.4.2 and from its older Vizia
pin to the vendored Vizia 0.4.0 revision. It implements the new concrete editor
and editor-handle APIs, including deferred parenting, show/hide, native size,
DPI, host resize callbacks and host main-thread scheduling.

Parameter callbacks only set an atomic dirty flag. The GUI idle callback reads
parameter values and updates reactive signals; no registry mutex or signal
allocation runs on the audio thread. Cached signals are cleared on editor reopen.

The adapter persists the existing user zoom state, tracks native DPI separately,
and observes confirmed window geometry. Programmatic zoom is enabled in baseview
while the framework's resize hint keeps arbitrary host resizing disabled.
Keyboard forwarding uses the current NamedKey API and remains limited to focused
textboxes.

User zoom and the application's base rendering scale are separate. GainStageFx
uses a base of 1.5 while starting the menu at 100%. Only user zoom is persisted;
the base remains an application setting. With this fixed-base constructor, host
DPI suggestions do not multiply the rendering scale or alter the menu percentage.
