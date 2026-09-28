fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    println!("cargo:rerun-if-changed=src/bridge/shim.m");

    // Compile the Objective-C Cocoa UI factory into the AU binary.
    //
    // AUv2 hosts discover the editor through Objective-C class metadata.
    // Merely implementing the editor in Rust is not sufficient: the
    // NiceAu2CocoaViewFactory class must exist in the final Mach-O image.
    cc::Build::new()
        .file("src/bridge/shim.m")
        .flag("-fobjc-arc")
        .compile("nice_plug_au2_cocoa_shim");

    println!("cargo:rustc-link-lib=framework=AudioUnit");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
}