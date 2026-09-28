#!/usr/bin/env bash
#
# Builds the plugin and installs the CLAP and VST3 into the user's plugin
# folders.
#
#   ./install.sh              build, then install
#   ./install.sh --no-build   install the existing bundles without rebuilding
#
# Both go into a BurningTreeC subfolder. Set CLAP_PATH or VST3_PATH to install
# somewhere other than ~/.clap and ~/.vst3.

set -euo pipefail

readonly VENDOR="BurningTreeC"
readonly PACKAGE="gainstagefx"

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
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

# shellcheck source=.github/install-Linux.sh
source "$project_dir/.github/install-Linux.sh"
gainstagefx_validate_destination "$clap_dir"
gainstagefx_validate_destination "$vst3_dir"

# Ask Cargo for its actual target directory, including CARGO_TARGET_DIR and
# .cargo/config.toml overrides. xtask uses the same metadata when bundling.
if ! command -v jq >/dev/null 2>&1; then
    echo "install.sh: jq is required to read Cargo's target directory." >&2
    exit 1
fi
target_dir=$(cd "$project_dir" && cargo metadata --offline --no-deps --format-version 1 | jq -er '.target_directory')
bundled="$target_dir/bundled"

if [ "$build" = true ]; then
    echo "Building $PACKAGE with release-lto..."
    (cd "$project_dir" && cargo xtask bundle "$PACKAGE" --profile release-lto)
fi

gainstagefx_install "$bundled" "$project_dir" "$clap_dir" "$vst3_dir"
