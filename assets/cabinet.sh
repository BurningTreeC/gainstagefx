#!/usr/bin/env bash
# Turns a photograph of a speaker cabinet into assets/cabinet.png.
#
# If the source already has a real alpha channel it is used as it stands. If
# it does not -- an image exported with the transparency *painted in* as a
# checkerboard, which is what an image without a real alpha channel usually is
# -- the checkerboard is keyed out by brightness: a cabinet is dark tolex and
# a mid grille, and both are well below the light squares.
#
# Usage: assets/cabinet.sh <source image>
set -euo pipefail

src=${1:?usage: cabinet.sh <source image>}
out=$(dirname "$0")/cabinet.png
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# ImageMagick answers True, False or Blend, and Blend is a real alpha channel
# too -- checking only for True sends an image that has one down the keying
# path, which throws away the soft edges it already had.
case "$(magick "$src" -format '%A' info:)" in
True|Blend)
    magick "$src" -trim +repage -resize x300 "$out"
    ;;
*)
    magick "$src" -colorspace Gray -threshold 80% -negate \
        -morphology Close Disk:5 -morphology Open Disk:3 \
        -blur 0x0.8 -level 40%,60% "$work/mask.png"
    magick "$src" "$work/mask.png" -alpha off -compose CopyOpacity -composite \
        -trim +repage -resize x300 "$out"
    ;;
esac

magick "$out" -format 'wrote %f, %wx%h\n' info:
