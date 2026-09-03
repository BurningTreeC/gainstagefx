#!/usr/bin/env bash
# Screenshots the standalone panel. The second argument is the DPI scale, so
#   ./shot.sh docs/panel.png 3
# grabs the panel at three times its own coordinates -- 2520 x 1488 rather than
# the 840 x 496 the window opens at. The panel is drawn rather than pictured, so
# it is sharp at any of them; the photographs are 512 and 300 pixel sources and
# hold up to about four. Targets the window by address and refuses
# to act unless the focus actually landed on it -- a title selector that finds
# nothing falls back to whatever is focused, which is how a terminal ended up
# floated across the screen.
set -euo pipefail
out=${1:?usage: shot.sh <output.png> [dpi-scale]}
dpi=${2:-1}

pkill -x gainstagefx 2>/dev/null || true
sleep 0.5
XDG_CONFIG_HOME="${SHOT_CONFIG:-$HOME/.config}" ./target/release/gainstagefx --backend dummy --dpi-scale "$dpi" >/dev/null 2>&1 &
sleep 3

addr=$(hyprctl clients -j | python3 -c "
import json,sys
for c in json.load(sys.stdin):
    if c['title'] == 'GainStageFx':
        print(c['address']); break
")
[ -n "$addr" ] || { echo 'no GainStageFx window'; exit 1; }

hyprctl repl "return hl.dispatch(hl.dsp.focus({ window = \"address:$addr\" }))" >/dev/null
sleep 0.5
active=$(hyprctl activewindow -j | python3 -c "import json,sys; print(json.load(sys.stdin)['address'])")
[ "$active" = "$addr" ] || { echo 'focus did not land; refusing to dispatch'; exit 1; }

floating=$(hyprctl clients -j | python3 -c "
import json, sys
addr = sys.argv[1]
print(next(c['floating'] for c in json.load(sys.stdin) if c['address'] == addr))
" "$addr")
[ "$floating" = "True" ] || hyprctl repl 'return hl.dispatch(hl.dsp.window.float())' >/dev/null
sleep 0.5
hyprctl repl 'return hl.dispatch(hl.dsp.window.move({ x = 100, y = 100 }))' >/dev/null
# The compositor's window opacity lets whatever is behind show through the
# panel, which is fine to look at and useless in a screenshot.
hyprctl setprop "address:$addr" alpha 1 lock >/dev/null 2>&1 || true
hyprctl setprop "address:$addr" alphainactive 1 lock >/dev/null 2>&1 || true
sleep 1.5

# By address again, not activewindow: moving the window can hand focus back to
# whatever was under the cursor.
read -r x y w h < <(hyprctl clients -j | python3 -c "
import json, sys
addr = sys.argv[1]
for c in json.load(sys.stdin):
    if c['address'] == addr:
        assert c['title'] == 'GainStageFx', c['title']
        print(c['at'][0], c['at'][1], c['size'][0], c['size'][1])
        break
else:
    raise SystemExit('window gone')
" "$addr")
grim -g "$x,$y ${w}x${h}" "$out"
magick "$out" -format 'wrote %f, %wx%h\n' info:
