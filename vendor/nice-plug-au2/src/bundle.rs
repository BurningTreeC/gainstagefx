use std::fs;
use std::path::Path;

use crate::config::{four_cc, four_cc_string};

pub fn create_au_bundle(
    bundle_name: &str,
    lib_path: &Path,
    config: &super::Au2Config,
) -> std::io::Result<()> {
    let component_name = format!("{}.component", bundle_name);
    let component_dir = Path::new(&component_name);
    let contents_dir = component_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    let lib_dest = macos_dir.join(lib_path.file_name().unwrap());
    fs::copy(lib_path, &lib_dest)?;

    let component_type = config.category.component_type();
    let sub_type = four_cc(config.sub_type);
    let manufacturer = four_cc(config.manufacturer);
    let manufacturer_string = four_cc_string(config.manufacturer);
    let sub_type_string = four_cc_string(config.sub_type);

    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleIdentifier</key>
    <string>com.{}.{}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>AudioUnit</key>
    <dict>
        <key>Manufacturer</key>
        <string>{}</string>
        <key>Name</key>
        <string>{}</string>
        <key>Version</key>
        <integer>1</integer>
    </dict>
    <key>AudioComponents</key>
    <array>
        <dict>
            <key>type</key>
            <string>{}</string>
            <key>subtype</key>
            <string>{}</string>
            <key>manufacturer</key>
            <string>{}</string>
            <key>name</key>
            <string>{}: {}</string>
            <key>version</key>
            <integer>65536</integer>
            <key>factoryFunction</key>
            <string>NiceAu2Factory</string>
        </dict>
    </array>
    <key>Component</key>
    <dict>
        <key>Type</key>
        <integer>{}</integer>
        <key>SubType</key>
        <integer>{}</integer>
        <key>Manufacturer</key>
        <integer>{}</integer>
        <key>Flags</key>
        <integer>0</integer>
    </dict>
</dict>
</plist>
"#,
        lib_path.file_name().unwrap().to_string_lossy(),
        manufacturer_string,
        bundle_name,
        config.name,
        manufacturer_string,
        config.name,
        config.category.component_type_string(),
        sub_type_string,
        manufacturer_string,
        manufacturer_string,
        config.name,
        component_type,
        sub_type,
        manufacturer
    );

    fs::write(contents_dir.join("Info.plist"), info_plist)?;

    println!("Created AU bundle at '{}'", component_dir.display());
    Ok(())
}
