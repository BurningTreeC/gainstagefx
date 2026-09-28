# nice-plug-au2

Audio Unit version 2 support for plugins built with [nice-plug](https://crates.io/crates/nice-plug).

On macOS, implement `Au2Plugin` and export the plugin with `nice_export_au2!`:

```rust
use nice_plug_au2::{Au2Category, Au2Plugin, nice_export_au2};

impl Au2Plugin for MyPlugin {
    const AU2_CATEGORY: Au2Category = Au2Category::Effect;
    const AU2_MANUFACTURER: [u8; 4] = *b"Acme";
    const AU2_SUBTYPE: [u8; 4] = *b"Demo";
}

nice_export_au2!(MyPlugin);
```

On Linux and Windows, the public marker API remains available and `nice_export_au2!` expands to nothing.

Use `nice-plug-au2-xtask` to create a `.component` bundle on macOS.

License: ISC.
