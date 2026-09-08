#!/usr/bin/env bash
# Renders assets/knob.png from assets/knob.glb.
#
# The knob used to be a photograph run through a threshold to pull an alpha
# channel off its black ground. It is now rendered from the model instead, so
# the asset carries the project's own licence and -- more to the point -- so
# that its lighting is the same rig every other rendered control on any of
# these panels uses. A photograph brings its own studio with it, and no two
# photographs agree about where the light is.
#
# The panel draws this without ever rotating it. Rotating a sprite carries the
# light baked into it round with the knob, so the highlight would travel with
# the control instead of staying at the top left where the panel's light is.
# The model therefore has no indicator on it; the widget draws the pointer on
# top at whatever angle the value asks for.
#
# The renderer lives in pulteqfx's assetgen. It is deterministic, so running
# this again without changing the model or the rig reproduces the file byte for
# byte.
#
# Usage: assets/knob.sh [path to the assetgen workspace]
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
gen=${1:-$here/../../pulteqfx}

if [ ! -d "$gen/assetgen" ]; then
    echo "no assetgen at $gen/assetgen -- pass its workspace as the argument" >&2
    exit 1
fi

# --scale 1: the model is already in millimetres, which is what assetgen works
#   in, and its axis is +Z, which is the axis assetgen turns parts about.
# --fallback aluminium: the export states base colour, metalness and roughness
#   per part and glTF has no way to state a *finish*, so the brushing and the
#   micro-texture come from assetgen's own aluminium, which is calibrated
#   against a photograph of a real 1176 cap.
# --margin 1.02: a little air, so the knurl does not touch the frame and
#   alias against it.
cargo run --release --manifest-path "$gen/Cargo.toml" -p assetgen -- \
    --part glb --glb "$here/knob.glb" \
    --scale 1.0 --fallback aluminium \
    --out "$here/knob.png" \
    --size 512 --ss 4 --frames 1 --angle 0 \
    --ao-samples 96 --margin 1.02

magick "$here/knob.png" -format 'wrote %f, %wx%h\n' info:
