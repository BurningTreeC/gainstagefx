# Local nice-plug-au2 changes

Upstream: fazibear/nice-plug-addons `155c4167ba` (crate 0.1.1), the Rust
rewrite of the AUv2 adapter. The following are carried:

- **The Cocoa view factory is linked.** `NiceAu2CocoaViewFactory` lives in
  `src/bridge/shim.m`, which `build.rs` compiles into a static archive. Nothing
  in Rust referenced that object, so the linker left it out of the plugin, and
  every host's `classNamed:` for the name `kAudioUnitProperty_CocoaUI` reports
  came back nil. In process that is no editor; out of process (AUHostingService,
  as Logic Pro and GarageBand load it) it is a 1x1 remote view in a tiny window.
  `nice_au2_cocoa_view_class_name()` now returns the name from the class itself
  and `properties.rs` reports that, which is the reference that links it.
- **The view's size is the editor's.** `EditorView` keeps the size the editor
  last asked for and gives it to any `setFrameSize:`, instead of calling into
  the editor (and its lock) from there. Once spawned, it takes the size the
  editor opened at: before then the editor cannot know the window's backing
  scale, and an editor sized in physical pixels is half that on Retina.
- **The editor can resize its view.** The frontend passes `HostMethods` whose
  `request_resize` sets the view's frame, which hosts follow through
  `NSViewFrameDidChangeNotification`. Zoom and GainStageFx's collapsible
  sections otherwise resized only baseview's own child view.
- **A closed view is freed.** The display link retains the view; it now stops
  when the view leaves its window, so a host that releases the view frees it and
  the editor closes, rather than rendering on unseen for the instance's life.

Validation: `tools/au_editor_smoke.m`, run by the macOS package job after
`auval`, opens the editor in process and out of process with audio rendering,
checks its size and pixels, reopens it, and samples the main threads if one
stops. The job also checks the class is in each slice's Objective-C class list.
