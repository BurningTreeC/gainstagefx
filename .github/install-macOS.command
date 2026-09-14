#!/bin/bash
# Run only for an archive you intentionally trust. No root or Apple account.
set -euo pipefail
cd "$(dirname "$0")"
[ "$(uname -s)" = Darwin ] || { echo "This installer is macOS-only." >&2; exit 1; }
readonly VENDOR=BurningTreeC
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
installed=0
install_bundle() {
    local source="$1" directory="$2" name
    name="$(basename "$source")"
    codesign --verify --deep --strict --verbose=4 "$source"
    mkdir -p "$directory"
    rm -rf "${directory:?}/$name"
    ditto "$source" "$directory/$name"
    # Trust only these explicitly selected bundles, never change global security.
    xattr -dr com.apple.quarantine "$directory/$name" 2>/dev/null || true
    codesign --verify --deep --strict --verbose=4 "$directory/$name"
    echo "Installed $name to $directory"
    installed=1
}
for bundle in *.clap *.vst3 *.component; do
    [ -d "$bundle" ] || continue
    case "$bundle" in
        *.clap) destination="$HOME/Library/Audio/Plug-Ins/CLAP/$VENDOR" ;;
        *.vst3) destination="$HOME/Library/Audio/Plug-Ins/VST3/$VENDOR" ;;
        *.component) destination="$HOME/Library/Audio/Plug-Ins/Components" ;;
    esac
    install_bundle "$bundle" "$destination"
done
for app in auv3/*.app; do
    [ -d "$app" ] || continue
    install_bundle "$app" "$HOME/Applications"
    target="$HOME/Applications/$(basename "$app")"
    log_dir="$HOME/Library/Logs/$VENDOR"
    mkdir -p "$log_dir"
    log="$log_dir/$(basename "$app").registration.log"
    {
        "$lsregister" -f "$target"
        for extension in "$target"/Contents/PlugIns/*.appex; do
            pluginkit -a "$extension"
            identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$extension/Contents/Info.plist")
            pluginkit -m -v -i "$identifier"
        done
    } >"$log" 2>&1 || true
    cat "$log"
    echo "AUv3 registration output: $log"
    echo "App installation alone does not confirm AUv3 loading. Open the app and test in your host."
done
[ "$installed" = 1 ] || { echo "No plugin bundles found beside this installer." >&2; exit 1; }
echo "Restart your audio host to rescan. See MACOS_AUDIO_UNITS.md for validation and troubleshooting."
