#!/usr/bin/env bash
#
# Installs the plugin bundles sitting next to this script.
#
#   ./install.sh              install for the current user
#   ./install.sh --system     install for all users (needs root)

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

# Plugins go in a folder named after the vendor, which keeps the plugin
# directories tidy. Every host reads subfolders.
readonly VENDOR="BurningTreeC"

if [ "${1:-}" = "--system" ]; then
    clap_dir="/usr/lib/clap/$VENDOR"
    vst3_dir="/usr/lib/vst3/$VENDOR"
elif [ -n "${1:-}" ]; then
    echo "usage: $0 [--system]" >&2
    exit 2
else
    clap_dir="${CLAP_PATH:-$HOME/.clap}/$VENDOR"
    vst3_dir="${VST3_PATH:-$HOME/.vst3}/$VENDOR"
fi

installed=0
install_bundle() {
    local bundle="$1" dest="$2" kind="$3"
    if [ ! -e "$bundle" ]; then
        echo "$bundle is not in this folder, skipping it"
        return
    fi
    mkdir -p "$dest"
    rm -rf "${dest:?}/$bundle"
    cp -R "$bundle" "$dest/"
    echo "$kind  $bundle  ->  $dest"
    installed=$((installed + 1))
}

install_bundle "GainStageFx.clap" "$clap_dir" "CLAP"
install_bundle "GainStageFx.vst3" "$vst3_dir" "VST3"

if [ "$installed" -eq 0 ]; then
    echo "No plugin bundles were found next to this script." >&2
    exit 1
fi

echo
echo "Done. Rescan plugins in your DAW."
