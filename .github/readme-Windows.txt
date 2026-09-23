GainStageFx
===========

The easy way
------------
Run install.exe from this folder. It installs into
Common Files\CLAP and Common Files\VST3 when run as administrator, and into
your own AppData folder otherwise. Both are searched by every host.

It puts the plugins in a folder named after the vendor, so after installing
as administrator you will find them at

  C:\Program Files\Common Files\CLAP\BurningTreeC\GainStageFx.clap
  C:\Program Files\Common Files\VST3\BurningTreeC\GainStageFx.vst3

and without administrator rights at

  %LOCALAPPDATA%\Programs\Common\CLAP\BurningTreeC\GainStageFx.clap
  %LOCALAPPDATA%\Programs\Common\VST3\BurningTreeC\GainStageFx.vst3

Those are the folders the CLAP and VST3 specifications name, and hosts search
them including their subfolders. install.exe prints where it put each one.

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

Create the folder first if it does not exist. A BurningTreeC subfolder works
too, and is what install.exe uses.


Where your saved presets go
---------------------------
%APPDATA%\GainStageFx\Presets, one .json file per preset. The folder is
created the first time a preset is saved successfully, so it will not be there
before then.

If your DAW does not list CLAP plugins, see
https://github.com/free-audio/clap#hosts

This program comes with ABSOLUTELY NO WARRANTY. It is free software under the
GNU General Public License version 3 or later; see LICENSE. The licences of
the libraries it uses are in THIRD-PARTY-NOTICES.md.
Source: https://github.com/BurningTreeC/gainstagefx
