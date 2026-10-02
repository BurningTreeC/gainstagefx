#!/usr/bin/env bash
# Install the CLAP and VST3 bundles next to this script.
# Usage: ./install.sh [--system]
set -euo pipefail

gainstagefx_validate_destination() {
    case "$1" in
        *:*)
            echo "install.sh: use one destination directory, not a search-path list: $1" >&2
            return 2
            ;;
    esac
}

# Also used by the source-tree installer. Stage both formats before touching
# either installed copy, so a missing bundle or failed copy leaves them intact.
gainstagefx_install() (
    local source_dir="$1" docs_dir="$2" clap_dir="$3" vst3_dir="$4"
    local clap_stage="" vst3_stage="" name
    gainstagefx_validate_destination "$clap_dir"
    gainstagefx_validate_destination "$vst3_dir"
    if [[ ! -f "$source_dir/GainStageFx.clap" || ! -d "$source_dir/GainStageFx.vst3" ]]; then
        echo "install.sh: both GainStageFx.clap and GainStageFx.vst3 are required in $source_dir" >&2
        exit 1
    fi
    for name in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
        if [[ ! -f "$docs_dir/$name" ]]; then
            echo "install.sh: missing $docs_dir/$name" >&2
            exit 1
        fi
    done

    trap '[[ -z "$clap_stage" ]] || rm -rf -- "$clap_stage"; [[ -z "$vst3_stage" ]] || rm -rf -- "$vst3_stage"' EXIT
    mkdir -p -- "$clap_dir" "$vst3_dir"
    clap_stage=$(mktemp -d "$clap_dir/.gainstagefx.XXXXXX")
    vst3_stage=$(mktemp -d "$vst3_dir/.gainstagefx.XXXXXX")
    cp -R -- "$source_dir/GainStageFx.clap" "$clap_stage/"
    cp -R -- "$source_dir/GainStageFx.vst3" "$vst3_stage/"
    for name in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
        cp -- "$docs_dir/$name" "$clap_stage/"
        cp -- "$docs_dir/$name" "$vst3_stage/"
    done

    # A Linux CLAP is one file: rename replaces it atomically without writing
    # over a library a running host may still have mapped.
    mv -fT -- "$clap_stage/GainStageFx.clap" "$clap_dir/GainStageFx.clap"

    # A nonempty directory cannot be atomically replaced with ordinary rename.
    # Keep the previous VST3 until the fully copied replacement is in place.
    if [[ -e "$vst3_dir/GainStageFx.vst3" || -L "$vst3_dir/GainStageFx.vst3" ]]; then
        mv -T -- "$vst3_dir/GainStageFx.vst3" "$vst3_stage/previous"
    fi
    if ! mv -T -- "$vst3_stage/GainStageFx.vst3" "$vst3_dir/GainStageFx.vst3"; then
        if [[ -e "$vst3_stage/previous" || -L "$vst3_stage/previous" ]]; then
            if ! mv -T -- "$vst3_stage/previous" "$vst3_dir/GainStageFx.vst3"; then
                # Preserve the backup if rollback itself fails.
                trap - EXIT
                echo "install.sh: restore the previous VST3 from $vst3_stage/previous" >&2
            fi
        fi
        exit 1
    fi
    for name in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
        mv -fT -- "$clap_stage/$name" "$clap_dir/$name"
        mv -fT -- "$vst3_stage/$name" "$vst3_dir/$name"
    done
    echo "Installed CLAP: $clap_dir/GainStageFx.clap"
    echo "Installed VST3: $vst3_dir/GainStageFx.vst3"
    echo "Restart REAPER (or your DAW) to unload the previous plugin binary, then rescan if needed."
)

gainstagefx_installer_main() {
    local source_dir clap_dir vst3_dir
    source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
    case "${1:-}" in
        --system)
            [[ $# -eq 1 ]] || {
                echo "usage: $0 [--system]" >&2
                return 2
            }
            clap_dir="/usr/lib/clap/BurningTreeC"
            vst3_dir="/usr/lib/vst3/BurningTreeC"
            ;;
        "")
            [[ $# -eq 0 ]] || {
                echo "usage: $0 [--system]" >&2
                return 2
            }
            clap_dir="${CLAP_PATH:-$HOME/.clap}/BurningTreeC"
            vst3_dir="${VST3_PATH:-$HOME/.vst3}/BurningTreeC"
            ;;
        *)
            echo "usage: $0 [--system]" >&2
            return 2
            ;;
    esac
    gainstagefx_install "$source_dir" "$source_dir" "$clap_dir" "$vst3_dir"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    gainstagefx_installer_main "$@"
fi
