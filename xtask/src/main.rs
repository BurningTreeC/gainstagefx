fn main() -> nih_plug_xtask::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("bundle-au") {
        return bundle_au();
    }
    nih_plug_xtask::main()
}

#[cfg(target_os = "macos")]
fn bundle_au() -> nih_plug_xtask::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member");
    let status = std::process::Command::new("python3")
        .arg(root.join("packaging/macos/au.py"))
        .arg("build")
        .args(std::env::args().skip(2))
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other("Audio Unit packaging failed; see tool output").into());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn bundle_au() -> nih_plug_xtask::Result<()> {
    Err(
        std::io::Error::other("bundle-au is macOS-only; use the existing bundle command here")
            .into(),
    )
}
