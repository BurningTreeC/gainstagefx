#!/usr/bin/env bash
#
# Builds the plugin and installs the CLAP and VST3 into the user's plugin
# folders.
#
#   ./install.sh              build, then install
#   ./install.sh --no-build   install whatever is already in target/bundled
#
# Both go into a BurningTreeC subfolder. Set CLAP_PATH or VST3_PATH to install
# somewhere other than ~/.clap and ~/.vst3.

set -euo pipefail

readonly VENDOR="BurningTreeC"
readonly CLAP="GainStageFx.clap"
readonly VST3="GainStageFx.vst3"
readonly PACKAGE="gainstagefx"

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
bundled="$project_dir/target/bundled"
clap_dir="${CLAP_PATH:-$HOME/.clap}/$VENDOR"
vst3_dir="${VST3_PATH:-$HOME/.vst3}/$VENDOR"

build=true
for arg in "$@"; do
    case "$arg" in
        --no-build) build=false ;;
        -h | --help)
            sed -n '3,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "install.sh: unknown option '$arg'" >&2
            exit 2
            ;;
    esac
done

if [ "$build" = true ]; then
    echo "Building $PACKAGE..."
    (cd "$project_dir" && cargo xtask bundle "$PACKAGE" --release)
fi

# Copies one bundle into place. Deletes the destination first and copies with
# -R so this works whether the bundle is a single shared library, as the CLAP
# is on Linux, or a directory, as the VST3 always is.
install_bundle() {
    local name="$1" dest="$2"

    if [ ! -e "$bundled/$name" ]; then
        echo "install.sh: '$bundled/$name' does not exist; run without --no-build first." >&2
        exit 1
    fi

    mkdir -p "$dest"
    rm -rf "${dest:?}/$name"
    cp -R "$bundled/$name" "$dest/"
    echo "Installed $name to $dest"
}

install_bundle "$CLAP" "$clap_dir"
install_bundle "$VST3" "$vst3_dir"

# The plugin is under the GPL, so the licence and the dependency notices travel
# with it rather than only living in the source tree.
for dir in "$clap_dir" "$vst3_dir"; do
    for doc in LICENSE THIRD-PARTY-NOTICES.md; do
        [ -e "$project_dir/$doc" ] && cp "$project_dir/$doc" "$dir/"
    done
done
