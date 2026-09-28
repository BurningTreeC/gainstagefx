# nice-plug and editor migration

Updated 2026-09-27. The migration is implemented in the working tree.

## Dependency choices

| Component | Selected source | Local work |
| --- | --- | --- |
| [nice-plug](https://codeberg.org/RustAudio/nice-plug) | `e60b5db09d8606c3dc3e93f982847a00bce17325`, 0.4.2 | Framework used without source patches |
| [Vizia](https://github.com/vizia/vizia) | `426d2e7b8533d485e78ab7f07f9f81d8324c256d`, 0.4.0 | Port baseview integration to 0.3.4; ordered reentrant dispatch and geometry/text-input events |
| [baseview](https://github.com/RustAudio/baseview) | `0fdebac0821370c2604005b8a81d6d96d714018d`, 0.3.4 | Required OpenGL compilation, Windows input/resize, and Linux initial-frame fixes |
| [vizia-plug](https://github.com/vizia/vizia-plug) | `34812c0ba14c5df39621bab956c74e660220636b`, 0.1.0 | Port to nice-plug-core 0.4.2 and current Vizia; GUI-thread parameter updates |

These were the inspected upstream heads when the port began. They are pinned
for reproducibility. Current Vizia and baseview cannot simply be used together
without an adapter port: upstream Vizia still declared baseview 0.2.2, while the
upstream plugin adapter declared nice-plug-core 0.1.4.

The previous NIH-plug, NIH Vizia fork, and old baseview dependency are removed.
There is one shared baseview implementation. Detailed patch inventories:
[baseview](vendor/baseview/PATCHES.md), [Vizia](vendor/vizia/PATCHES.md),
[adapter](vendor/vizia_plug/PATCHES.md).

## Preserved behavior

- CLAP/VST3 identity, parameter IDs and saved-state serialization are retained.
- The DSP implementation, oversampling, 66-sample latency, and frozen audio
  fixtures are unchanged by this migration. Denormal flushing remains enabled.
- The editor keeps its 780 x 968 layout, controls, original knob artwork and
  Roboto fonts. Old lens bindings/custom femtovg drawing are ported to reactive
  signals and Skia. Preset menus, save dialogs, text entry and 75/90/100/125/150/175/200%
  zoom remain available. The menu defaults to 100%, with a separate base rendering
  scale of 1.5 (175% maps to 2.625; 200% to 3.0), in physical pixels per design unit. Host DPI suggestions do not multiply this
  explicit base scale or change the menu percentage.
- Parameter notifications set an atomic flag; reactive updates happen on the
  GUI thread. This avoids putting a signal-registry mutex on the audio path.

## Linux blank editor diagnosis

The latest baseview X11 code marked an embedded window visible during `Show`,
before its `MapNotify` event arrived. `MapNotify` then saw no visibility change,
so it never subscribed to Present notifications or started its fallback timer.
Window creation succeeded, but no frame was rendered. The fix derives frame startup from the resulting visibility state, including
reparenting. The embedded FX chain also exposed a stale hidden state on an
intermediate parent: ancestry now refreshes the moved window itself and listens
to its own structure notifications.

The native test reproduces the original failure as a one-color child window.
It verifies rendering, actual menu-driven zoom, hide/show, and editor recreation
through the packaged CLAP C ABI. It runs in the Linux package CI job under Xvfb.
The rebuilt release CLAP was also loaded in an isolated native REAPER instance: its
editor displayed rendered controls both on opening and after closing/reopening,
in both floating and embedded FX-chain views.

## Validation and remaining limits

The port has been checked with the frozen DSP/state/regression suite, the real
reactive panel rendered through Skia, all-target/all-feature Clippy, native Linux
CLAP window tests, Linux release bundles/standalone compilation, and the Windows
backend's four regression tests under Wine. The Windows backend also cross-compiles
with OpenGL enabled. Native Windows and macOS DAW testing remains necessary.

Cabinet/speaker/microphone physics changes are separate from this framework port.
See [the acoustic review](CABINET_PHYSICS_REVIEW.md); its proposed redesign is not
implemented by the editor migration.
