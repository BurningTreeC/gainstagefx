GainStageFx
===========

The easy way
------------
Run install.sh from this folder:

  ./install.sh              install for you
  ./install.sh --system     install for all users (run with sudo)

It copies both plugins into ~/.clap/BurningTreeC and ~/.vst3/BurningTreeC.

By hand
-------
CLAP   copy GainStageFx.clap to            ~/.clap
VST3   copy the GainStageFx.vst3 folder to ~/.vst3

For all users, use /usr/lib/clap and /usr/lib/vst3 instead. Create the
directory first if it does not exist. A per-vendor subfolder such as
~/.clap/BurningTreeC works too, and is what install.sh uses.

If your DAW does not list CLAP plugins, see
https://github.com/free-audio/clap#hosts

This program comes with ABSOLUTELY NO WARRANTY. It is free software under the
MIT License or the Apache License 2.0, at your option; see LICENSE-MIT and
LICENSE-APACHE. The licences of the libraries it uses are in
THIRD-PARTY-NOTICES.md.
Source: https://github.com/BurningTreeC/gainstagefx
