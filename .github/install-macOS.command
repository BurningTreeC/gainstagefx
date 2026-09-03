#!/usr/bin/env bash
#
# Installs GainStageFx for the current user and clears the macOS quarantine flag,
# which is otherwise what stops an ad-hoc signed plugin from loading.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

# A folder named after the vendor, matching the Linux and Windows installers.
readonly VENDOR="BurningTreeC"
clap_dir="$HOME/Library/Audio/Plug-Ins/CLAP/$VENDOR"
vst3_dir="$HOME/Library/Audio/Plug-Ins/VST3/$VENDOR"

install_bundle() {
    local bundle="$1" dest="$2"
    [ -e "$bundle" ] || { echo "missing $bundle, is this archive complete?"; exit 1; }
    mkdir -p "$dest"
    rm -rf "${dest:?}/$bundle"
    cp -R "$bundle" "$dest/"
    # Signed ad-hoc rather than notarized, so the quarantine flag has to go.
    xattr -dr com.apple.quarantine "$dest/$bundle" 2>/dev/null || true
    echo "Installed $bundle to $dest"
}

install_bundle "GainStageFx.clap" "$clap_dir"
install_bundle "GainStageFx.vst3" "$vst3_dir"

echo
echo "Done. Rescan plugins in your DAW."
