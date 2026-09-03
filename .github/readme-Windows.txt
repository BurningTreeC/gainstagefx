GainStageFx
===========

The easy way
------------
Run install.exe from this folder. It installs into
Common Files\CLAP and Common Files\VST3 when run as administrator, and into
your own AppData folder otherwise. Both are searched by every host.

install.exe is not code signed, so Windows SmartScreen will say the publisher
is unknown. Signing certificates cost money every year, which this project
does not have. Click "More info" and then "Run anyway", or install by hand
using the steps below if you would rather not.

By hand
-------
CLAP   copy GainStageFx.clap to
         C:\Program Files\Common Files\CLAP
VST3   copy the GainStageFx.vst3 folder to
         C:\Program Files\Common Files\VST3

Or, without administrator rights, into
  %LOCALAPPDATA%\Programs\Common\CLAP
  %LOCALAPPDATA%\Programs\Common\VST3

Create the folder first if it does not exist.

If your DAW does not list CLAP plugins, see
https://github.com/free-audio/clap#hosts

This program comes with ABSOLUTELY NO WARRANTY. It is free software under the
GNU General Public License version 3 or later; see LICENSE. The licences of
the libraries it uses are in THIRD-PARTY-NOTICES.md.
Source: https://github.com/BurningTreeC/gainstagefx
