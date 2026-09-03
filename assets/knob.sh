#!/usr/bin/env bash
# Turns a photograph of a knob into assets/knob.png.
#
# The panel draws the knob without ever rotating it -- rotating a photograph
# rotates the light baked into it -- so the source must have no printed
# indicator on it. The pointer is drawn on top by the widget instead.
#
# Usage: assets/knob.sh <source image>
set -euo pipefail

src=${1:?usage: knob.sh <source image>}
out=$(dirname "$0")/knob.png
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The knob is lit metal on a black ground, so the alpha is the ground itself:
# threshold it, close the pinholes the knurling punches in it, and feather the
# edge by a pixel so it does not alias against the panel.
magick "$src" -colorspace Gray -threshold 12% \
    -morphology Close Disk:3 -morphology Open Disk:2 \
    -blur 0x1 -level 40%,60% "$work/alpha.png"

magick "$src" "$work/alpha.png" -alpha off -compose CopyOpacity -composite \
    -trim +repage -resize 512x512 "$out"

magick "$out" -format 'wrote %f, %wx%h\n' info:
