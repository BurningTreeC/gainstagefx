GainStageFx
===========

The easy way
------------
Double-click Install.command. It copies both plugins into place and clears the
macOS quarantine flag described below.

The first time, macOS will refuse to run it because it came from the internet.
Right-click Install.command, choose Open, then confirm in the dialog. After
that it runs normally.

By hand
-------
Copy the bundles (they are folders, copy the whole thing) to:

  CLAP   ~/Library/Audio/Plug-Ins/CLAP/GainStageFx.clap
  VST3   ~/Library/Audio/Plug-Ins/VST3/GainStageFx.vst3

(Install.command puts them in a BurningTreeC subfolder of each, which hosts
search just the same.)

Then open Terminal and run:

  xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/GainStageFx.clap
  xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/GainStageFx.vst3

Why that last step is needed
----------------------------
The plugins are signed, but only ad-hoc: signed by nobody in particular. That
is enough for macOS to load them, and on Apple Silicon it is required, but it
is not the same as being notarized by Apple. Notarizing needs a paid Apple
Developer account, which this project does not have.

macOS tags anything downloaded from the internet with a quarantine flag, and
refuses to load quarantined code that is not notarized. Removing the flag with
the command above tells macOS you trust these files. Nothing else about the
plugins changes.

Note that Logic and GarageBand only load Audio Units, and this plugin is CLAP
and VST3, so use a host that supports those.

This program comes with ABSOLUTELY NO WARRANTY. It is free software under the
GNU General Public License version 3 or later; see LICENSE. The licences of
the libraries it uses are in THIRD-PARTY-NOTICES.md.
Source: https://github.com/BurningTreeC/gainstagefx

Audio Units
-----------
This archive also includes self-contained AUv2 .component bundles and AUv3
containing apps in auv3/. Install.command installs AUv2 in your user's
Components folder and AUv3 apps in ~/Applications, then records registration
output. Do not install an .appex by itself in Components.

An Apple Developer account is NOT part of this project's build requirements.
These builds are ad-hoc signed, not Developer ID signed, and not notarized.
Downloaded/transferred builds may trigger Gatekeeper/quarantine warnings.
The installer clears quarantine only for these bundles you chose to install.
AUv3 registration/loading remains dependent on the local macOS host: consult
MACOS_AUDIO_UNITS.md and the per-plugin CI validation results. A built app is
not proof that an AUv3 extension registered or loaded.

The AU distribution minimum is macOS 11.0. Binaries include arm64 and x86_64.
