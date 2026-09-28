#!/usr/bin/env bash

set -euo pipefail

PLUGIN="${1:-GainStageFx}"

readonly MANUFACTURER="BrTC"
readonly SUBTYPE="GSfx"
readonly COMPONENT_TYPE="aufx"

clap_bundle="target/bundled/${PLUGIN}.clap"
clap_binary="${clap_bundle}/Contents/MacOS/${PLUGIN}"

component="target/bundled/${PLUGIN}.component"
contents="${component}/Contents"
macos="${contents}/MacOS"
resources="${contents}/Resources"
component_binary="${macos}/${PLUGIN}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: AUv2 bundles can only be packaged on macOS" >&2
    exit 1
fi

if [[ ! -f "${clap_binary}" ]]; then
    echo "error: universal CLAP binary does not exist:" >&2
    echo "  ${clap_binary}" >&2
    echo "Run 'cargo xtask bundle-universal gainstagefx --release' first." >&2
    exit 1
fi

echo "Checking universal plugin binary..."

archs="$(lipo -archs "${clap_binary}")"
echo "Architectures: ${archs}"

if [[ "${archs}" != *"x86_64"* || "${archs}" != *"arm64"* ]]; then
    echo "error: ${clap_binary} is not a universal x86_64 + arm64 binary" >&2
    exit 1
fi

# The AU adapter must actually have been linked into the plugin.
if ! nm -gU "${clap_binary}" | grep -q 'NiceAu2Factory'; then
    echo "error: NiceAu2Factory is missing from the plugin binary" >&2
    echo "The nice-plug-au2 adapter was not linked into GainStageFx." >&2
    exit 1
fi

if ! nm -gU "${clap_binary}" | grep -q 'nice_au2_register_plugin_entry'; then
    echo "error: nice_au2_register_plugin_entry is missing" >&2
    exit 1
fi

version="$(
    awk '
        /^\[package\]/ { in_package=1; next }
        /^\[/ && in_package { exit }
        in_package && /^version[[:space:]]*=/ {
            gsub(/^[^"]*"/, "")
            gsub(/".*$/, "")
            print
            exit
        }
    ' Cargo.toml
)"

if [[ -z "${version}" ]]; then
    echo "error: could not determine package version from Cargo.toml" >&2
    exit 1
fi

echo "Creating ${component}..."

rm -rf "${component}"
mkdir -p "${macos}" "${resources}"

# The CLAP, VST3 and AU entry points all live in the same Rust cdylib.
# bundle-universal has already lipo'd the two architecture slices, so reuse
# that exact universal Mach-O image rather than building/lipo'ing it again.
cp "${clap_binary}" "${component_binary}"
chmod +x "${component_binary}"

cat > "${contents}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>

    <key>CFBundleExecutable</key>
    <string>${PLUGIN}</string>

    <key>CFBundleIdentifier</key>
    <string>com.burningtreec.gainstagefx.au</string>

    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>

    <key>CFBundleName</key>
    <string>${PLUGIN}</string>

    <key>CFBundleDisplayName</key>
    <string>${PLUGIN}</string>

    <key>CFBundlePackageType</key>
    <string>BNDL</string>

    <key>CFBundleShortVersionString</key>
    <string>${version}</string>

    <key>CFBundleVersion</key>
    <string>${version}</string>

    <key>NSHumanReadableCopyright</key>
    <string>Copyright BurningTreeC</string>

    <key>AudioComponents</key>
    <array>
        <dict>
            <key>type</key>
            <string>${COMPONENT_TYPE}</string>

            <key>subtype</key>
            <string>${SUBTYPE}</string>

            <key>manufacturer</key>
            <string>${MANUFACTURER}</string>

            <key>name</key>
            <string>BurningTreeC: ${PLUGIN}</string>

            <key>description</key>
            <string>Circuit modelled gain stages</string>

            <key>factoryFunction</key>
            <string>NiceAu2Factory</string>

            <key>version</key>
            <integer>1</integer>

            <key>sandboxSafe</key>
            <true/>
        </dict>
    </array>
</dict>
</plist>
EOF

plutil -lint "${contents}/Info.plist"

echo "Ad-hoc signing ${component}..."
codesign \
    --force \
    --deep \
    --sign - \
    "${component}"

echo "Verifying signature..."
codesign \
    --verify \
    --deep \
    --strict \
    --verbose=2 \
    "${component}"

echo "Verifying packaged architecture..."
lipo -archs "${component_binary}"

echo
echo "Created AUv2:"
echo "  ${component}"
echo
echo "Component:"
echo "  type=${COMPONENT_TYPE}"
echo "  subtype=${SUBTYPE}"
echo "  manufacturer=${MANUFACTURER}"