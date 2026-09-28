#!/usr/bin/env bash
#
# Installs GainStageFx for the current user and clears the macOS quarantine
# flag, which is otherwise what stops an ad-hoc signed plugin from loading.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

readonly VENDOR="BurningTreeC"

clap_dir="$HOME/Library/Audio/Plug-Ins/CLAP/$VENDOR"
vst3_dir="$HOME/Library/Audio/Plug-Ins/VST3/$VENDOR"
au_dir="$HOME/Library/Audio/Plug-Ins/Components"

install_bundle() {
    local bundle="$1"
    local dest="$2"

    [ -e "$bundle" ] || {
        echo "missing $bundle, is this archive complete?"
        exit 1
    }

    mkdir -p "$dest"
    rm -rf "${dest:?}/$bundle"
    cp -R "$bundle" "$dest/"

    # The release is ad-hoc signed rather than Developer-ID signed/notarized.
    xattr -dr com.apple.quarantine "$dest/$bundle" 2>/dev/null || true

    echo "Installed $bundle to $dest"
}

install_bundle "GainStageFx.clap" "$clap_dir"
install_bundle "GainStageFx.vst3" "$vst3_dir"
install_bundle "GainStageFx.component" "$au_dir"

# Force macOS to discard cached Audio Unit information. The service will be
# restarted automatically when an AU host next needs it.
killall -9 AudioComponentRegistrar 2>/dev/null || true

echo
echo "Done."
echo
echo "Installed formats:"
echo "  CLAP : $clap_dir/GainStageFx.clap"
echo "  VST3 : $vst3_dir/GainStageFx.vst3"
echo "  AUv2 : $au_dir/GainStageFx.component"
echo
echo "Rescan plugins in your DAW."