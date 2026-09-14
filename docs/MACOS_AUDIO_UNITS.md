# macOS Audio Units — gainstagefx

An Apple Developer account is NOT part of this project's build requirements.

This repository packages only its own plugins. Its xtask, metadata, CMake,
validation tools, CI and distribution do not need any sibling repository.
`--all` means all supported packages **in this Cargo workspace**.

## Architecture and existing behavior

Rust/NIH-plug remains authoritative for DSP, CLAP parameters, state, presets and
the Vizia/baseview GUI. The existing native `nih_export_clap!` and
`nih_export_vst3!` exports are unchanged. Only the CLAP is wrapped:

```text
NIH-plug -> CLAP (independently distributable)
         -> VST3 (native NIH export)
CLAP -> clap-wrapper -> AUv2 .component
                    -> AUv3 .appex inside an AUv3 containing .app
```

No alternate DSP, parameter model, AU backend or Cocoa plugin GUI is implemented.
`probe.mm` is a test host, never compiled into distributable plugins. The
containing application and extension GUI integration come from upstream.
Each AU contains its own CLAP at `Contents/PlugIns/<name>.clap`; it does not
need anything installed in the user/system CLAP directories. `clap-wrapper` is
added as a CMake subproject with only AU targets requested, avoiding upstream's
top-level default VST3 target. The separate AudioUnitSDK is fetched for AUv2
only; AUv3 uses system frameworks.

Normal `cargo xtask bundle ...` and `bundle-universal ...` still delegate to
NIH-plug unchanged. Linux/Windows never configure CMake or download Apple SDKs;
`bundle-au` rejects those operating systems before launching external tools.

## Pinned dependencies

| Dependency | Tag | Commit |
| --- | --- | --- |
| [clap-wrapper](https://github.com/free-audio/clap-wrapper/tree/1cca996e96f29ab2be7ae9f8cfe532bbc92e1dd6) | v0.16.0 | `1cca996e96f29ab2be7ae9f8cfe532bbc92e1dd6` |
| [AudioUnitSDK](https://github.com/apple/AudioUnitSDK/tree/53a9a2008aae7fb1b0a9f093dd523b9b12f6c0d9) | AudioUnitSDK-1.1.0 | `53a9a2008aae7fb1b0a9f093dd523b9b12f6c0d9` |
| [CLAP SDK](https://github.com/free-audio/clap/tree/69a69252fdd6ac1d06e246d9a04c0a89d9607a17) | 1.2.6 | `69a69252fdd6ac1d06e246d9a04c0a89d9607a17` |

`packaging/macos/dependencies.json` is the pin source. FetchContent uses full
commit hashes and disables upstream's implicit dependency downloads. All are
public source downloads requiring no Apple login. Wrapper/SDK licenses and
fmt's notice are carried alongside the existing Rust third-party notices.
NIH-plug/NIH-plug Vizia/NIH-plug xtask remain at
`f36931f7af4646065488a9845d8f8c2f95252c23`.
The upstream README, CMake functions, embedded-CLAP example, wiki and both AU
implementations were inspected; the older wiki is incomplete for AUv3. This
integration uses the pinned functions in `cmake/wrap_auv2.cmake`,
`wrap_auv3.cmake`, and `wrap_auv3_standalone.cmake`.

## Stable identifiers and versions

All are effects (`aufx`) from BurningTreeC (`BTrC`). These new subtype codes are
permanent compatibility identifiers; never regenerate them on a build.
Existing names/capitalization are preserved, including Comp76Fx revision names
where applicable. There is no synthetic combined Comp76Fx CLAP to wrap.

| Rust package | Display/artifact stem | Bundle ID base / existing CLAP ID | AU subtype |
| --- | --- | --- | --- |
| gainstagefx | GainStageFx | `com.burningtreec.gainstagefx` | `GsFx` |

For each base `B`, AUv2 uses `B.auv2`, the AUv3 containing app uses `B.auv3`,
and its extension uses `B.auv3.extension`. The extension is a child of the app's
identifier. AUv2 and AUv3 intentionally share the component tuple; some hosts
choose one implementation instead of displaying both. The validation host
filters `kAudioComponentFlag_IsV3AudioUnit` and verifies the selected format.

Cargo metadata supplies the package version (currently 0.7.0), including
workspace-inherited versions. Only three-part numeric versions with each part
<=255 are accepted, to prevent upstream's packed AudioComponent version from
silently aliasing another release. Unsupported prerelease/build suffixes fail
with an explanation. AU component versions use `(major<<16)+(minor<<8)+patch`.
The generated manifest captures the actual version used in each build.

## Requirements and minimum macOS

Use a Mac with full Xcode selected by `xcode-select`, its macOS SDK/tools,
CMake >=3.27, Python >=3.9, Git, and Rust. Xcode's command-line-tools package
alone cannot supply the Xcode CMake generator. First-run local Xcode license
acceptance/components may be needed; account login and automatic signing are
not required. Public dependencies require network access on the first build.

The AU distribution minimum is **macOS 11.0**, centralized in
`packaging/macos/plugins.json`. `bundle-au` applies this same target to Rust
CLAP/VST3 and CMake AU wrappers/apps. This explicitly raises the older macOS CI
10.13 floor for this new four-format distribution and supports native Apple
Silicon. It also avoids a legacy filesystem compatibility dependency. Older
native-only bundle commands remain available with their previous policy.
Actual operation on the minimum OS still requires a machine running it.

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
# This workspace only, Universal 2, all four formats:
cargo xtask bundle-au --all --release
# One specific package:
cargo xtask bundle-au gainstagefx --release
# AUv2 independently, even if AUv3 fails on your machine:
cargo xtask bundle-au --all --release --format auv2
# AUv3 app/extension only, plus native CLAP and VST3:
cargo xtask bundle-au --all --release --format auv3
# Apple Silicon only:
cargo xtask bundle-au --all --release --arch arm64
```

For Comp76Fx, `bundle-au comp76fx --release` is also a family alias inside its
own workspace; `--all` selects revisions A, D and F. Each revision remains a
separate artifact with its own subtype and identifier.

Builds are sequential: native NIH CLAP/VST3, AUv2, then AUv3. Each completed AU
format is staged, signed and checked before proceeding. Build directories are
`<Cargo target dir>/macos-au/{auv2,auv3}/{universal,arm64}-{release,debug}`.
Both use `-G Xcode`. The CMake layer calls the upstream embedded-CLAP helpers;
no generated Xcode files enter the source tree. A content-hashed helper-only translation unit forces descriptor regeneration
when the CLAP or manifest changes, including under Xcode, which does not
honor the LINK_DEPENDS target property.

The separate distribution is:

```text
dist/macos/
  manifest.json
  licenses/
  gainstagefx/
    GainStageFx.clap
    GainStageFx.vst3
    GainStageFx.component
    auv3/
      GainStageFx AUv3.app/
        Contents/PlugIns/GainStageFx.appex/
          Contents/PlugIns/GainStageFx.clap/
```

There is one package directory per selected local plugin. The .appex is never
renamed to .component. Native NIH products also remain in `target/bundled`.
A build replaces the selected package's distribution directory and writes a
manifest for the selected formats/packages; use `--all` for a complete release.
Do not manually combine manifests from unrelated builds.

## Ad-hoc signing

Default and only supported AU signing mode: **AD-HOC**. No keychain import,
Team ID, signing certificate, provisioning profile, account secret, Developer
ID, notarization, timestamp or hardened-runtime release step is used.
Xcode receives `CODE_SIGN_IDENTITY=-`, `CODE_SIGN_STYLE=Manual`, and empty
team/profile settings. The AUv3 extension has app-sandbox and user-selected-file
entitlements; the app requires no account entitlement.

After staging, the helper signs each nested CLAP, then extension, then outer
app. It preserves the AUv3 sandbox entitlements explicitly. `--deep` is used
for verification, never as a shortcut for signing. The AUv2 Xcode identifier
is aligned with upstream's generated plist; the containing-app plist uses the
Cargo version instead of upstream's literal 1.0/1. The staged minimum OS is
normalized before signing, without changing Xcode's generated plist inputs.

```bash
python3 packaging/macos/au.py sign "dist/macos/gainstagefx/GainStageFx.component"
python3 packaging/macos/au.py sign "dist/macos/gainstagefx/auv3/GainStageFx AUv3.app"
codesign --verify --deep --strict --verbose=4 "dist/macos/gainstagefx/GainStageFx.component"
codesign -dv --verbose=4 "dist/macos/gainstagefx/auv3/GainStageFx AUv3.app"
```

Signature inspection must show `Signature=adhoc`. Re-signing an .appex with a
bare codesign command can remove its sandbox entitlements; use this helper.

These builds are not Developer ID signed and are not notarized.
They may trigger macOS Gatekeeper/quarantine warnings when transferred
between machines or downloaded from the internet.

That is acceptable for this project's local/manual distribution target. When
you intentionally trust a downloaded artifact, `Install.command` clears only
its quarantine attributes as part of local installation. For a specific
trusted bundle you can also use `xattr -dr com.apple.quarantine <bundle>`.
Never disable global Gatekeeper settings or SIP. Ad-hoc signatures establish
bundle integrity; they do not provide a verified developer identity.

## Installation and registration

From a source checkout after building:

```bash
python3 packaging/macos/au.py install --format auv2
python3 packaging/macos/au.py install --format auv3
python3 packaging/macos/au.py register
```

AUv2 goes to `~/Library/Audio/Plug-Ins/Components/`, with no root access. AUv3
containing apps go to `~/Applications/`; the embedded extension stays inside
its app. Registration calls LaunchServices for the app and PluginKit for its
embedded extension, then retries an exact bundle-ID lookup for up to 10 seconds:

```bash
app="$HOME/Applications/GainStageFx AUv3.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"
pluginkit -a "$app/Contents/PlugIns/GainStageFx.appex"
pluginkit -m -v -i com.burningtreec.gainstagefx.auv3.extension
open "$app"
```

Opening the containing app is the normal interactive discovery/host workflow;
it can request microphone permission for its standalone audio input. A
successful app copy or `pluginkit -a` exit code alone is not reported as proof
of registration. `registration.json` and individual command logs record the
actual result. Even a positive registry match is not proof of successful
loading: use the AUv3 probe or a host next.

Downloaded CI archives have `Install.command` beside the plugin bundles. This
installs CLAP/VST3 too, and logs AUv3 registration under
`~/Library/Logs/BurningTreeC/`. Source helpers install only the requested AUs,
so the self-containment test does not install a separate CLAP accidentally.
Restart the audio host after installation. If registration appears stale,
quit hosts and optionally run `killall AudioComponentRegistrar` for your own
user, then reopen the host. No helper deletes unrelated Audio Unit caches.

## Automated validation and architecture inspection

```bash
python3 -m unittest discover -s packaging/macos -p test_packaging.py
python3 packaging/macos/au.py verify
python3 packaging/macos/au.py validate
python3 packaging/macos/au.py probe --format auv2
python3 packaging/macos/au.py probe --format auv3
lipo -archs "dist/macos/gainstagefx/GainStageFx.component/Contents/MacOS/GainStageFx"
file "dist/macos/gainstagefx/GainStageFx.component/Contents/PlugIns/GainStageFx.clap/Contents/MacOS/GainStageFx"
```

The verifier checks all native and wrapper executables and all embedded CLAP
executables, requiring exact arm64+x86_64 (or exact arm64 for that mode), valid
ad-hoc signatures, AUv3 sandbox entitlements, bundle identifiers/types/names,
versions and component codes, a real nested extension, and byte-identical
embedded/reference CLAP binaries. `otool -L` rejects unbundled non-system
runtime dependencies. A wrapper-only universal binary does not pass.

The test host selects AUv2/AUv3 explicitly. It compares CLAP/AU parameter counts,
numeric IDs, names, ranges, default values and writable flags, changes values,
serializes AU fullState, destroys/recreates the AU and verifies restored
parameter values. AUv3's upstream omission of CLAP-hidden parameters is
accounted for. This does not yet automate UI gestures or every live automation
path. NIH normalized parameter/state implementations remain authoritative.

Source inspection found stereo->stereo and mono->mono for every plugin in
this workspace, without mono->stereo or auxiliary buses. The probe tests both
layouts at 44.1/48/96 kHz and 32/128/512 frames, rendering eight blocks per
combination and rejecting non-finite samples. It does not add unsupported
layouts and is not a sample-exact DSP comparison. Actual results are emitted
as separate JSON and text files for every plugin/format.

The `validate` command invokes the real component tuples, for example:

```bash
auval -v aufx GsFx BTrC
```

With both versions registered, auval may select either implementation of the
shared tuple. CI runs auval with AUv2 installed **before AUv3 registration**;
the subsequent probe independently proves which AU version it instantiated.
Only the runner's native slice is exercised at runtime; both binary slices
are inspected. No Intel-host/Logic success is inferred from lipo output.

## CI and release artifacts

`.github/workflows/build.yml` uses this repository's `bundle-au --all --release`
on `macos-15` (arm64), with both Rust macOS targets installed. It verifies all
bundles, tests AUv2 without installing CLAP, runs auval and the parameter/state/
render probe, then installs/registers AUv3 and runs its probe. Compilation,
structure, signature, architecture and registered-but-unloadable AUv3 failures
are strict failures. Only a missing AUv3 registration may be explicitly
reported and tolerated with `--allow-unregistered-auv3`; this produces an
UNTESTED result, never a successful load claim. No Apple secrets are needed.
Exact Xcode/macOS/tool versions and errors are uploaded as the separate
`*-macos-au-diagnostics` artifact, including after a failed build.

Mac bundles are tarred before artifact upload, then extracted with permissions
and symlinks intact for release ZIP creation. Linux/Windows packaging commands,
formats and installers are preserved. Release jobs exclude diagnostics from
plugin archives. No workflow has been run remotely merely by editing it here.

## Host acceptance checklist and current limitations

Implementation was performed on Linux. **No macOS binary, codesign result,
auval pass, AUv3 registration/load, or Logic result has been observed here.**
No exact macOS/Xcode error can honestly be recorded until a Mac executes this
workflow. The configured ad-hoc approach follows upstream's macOS AUv3 code;
its build/registration viability in the target host remains unverified.
AUv2 can be built and used independently via `--format auv2`.

For every local plugin and both AU formats, record OS/Xcode/host version,
architecture and these checks in a release validation log:

- Logic Pro first, then GarageBand and REAPER macOS if installed: discovery,
  GUI open/close/reopen, supported resize/scaling, Retina, mouse and keyboard.
- Compare CLAP and AU parameter text, normalization, gestures, host->plugin
  and plugin->host automation feedback, including audio-running automation.
- Change settings/preset, save host project, close/reopen host/project and
  verify complete state and GUI state. The automated fullState roundtrip
  complements this check but does not replace a host-project test.
- Test both advertised layouts, rates and buffer sizes; check latency,
  oversampling changes, offline bounce and representative audio for parity.
- Move/remove the separately installed CLAP, then repeat AU loading. CI never
  installs a standalone CLAP during AU testing, but local machines may have one.

The existing GUIs save JSON presets under a user config directory, while
factory presets and persisted parameter/editor state are supplied by NIH-plug.
AUv3 sandboxing can give the same unchanged preset code a different home/config
location or deny access to pre-existing external presets. Shared external
preset-folder access is not yet verified; no broad filesystem or account
entitlements were added to conceal this limitation.

No new runtime glue was introduced. Packaging scans, copies, signs and launches
processes only outside the audio process. Upstream processing code was reviewed:
AUv2 pre-reserves event/gesture storage, AUv3 prepares audio storage outside
render and uses queues for audio/UI communication. This is a source review,
not a measured guarantee that every upstream path is allocation/lock-free;
realtime instrumentation and stress tests remain a Mac acceptance check.

If AUv3 cannot register with ad-hoc signing on a particular OS/runner, retain
the app/extension and exact `registration.json`, command logs and probe errors.
Do not substitute credentials or claim that copied .appex files are installed.
An Apple account, Developer ID and notarization are not future requirements.

## Troubleshooting and clean rebuild

- Missing Xcode generator: select full Xcode and confirm `xcodebuild -version`.
- Architecture check fails: use the same `--arch` for the complete chain;
  install both Rust targets for Universal 2 and rebuild.
- AUv3 signature/registration fails: inspect both nested and outer signatures
  and the sandbox entitlement, keep exact tool logs, then try opening the
  containing app. A host may reject ad-hoc software; no account fallback exists.
- Version/plist mismatch: rerun `bundle-au` so metadata regenerates from Cargo.
- For a clean rebuild, remove only this repository's generated
  `target/macos-au` (or the equivalent custom Cargo target directory) and
  `dist/macos`, then rerun the build. Do not delete system caches.
- Restore the previous distribution from your own backup before downgrading;
  the installer replaces only matching product names in your user directories.

## Implementation validation record (Linux, 2026-09-14)

- Existing `cargo test --release --offline`: 255 passed, 0 failed,
  5 ignored across unit/integration/doc-test targets.
- Packaging unittest suite: 10 passed.
- `cargo clippy -p xtask --offline -- -D warnings`, xtask rustfmt, installer
  Bash syntax, workflow YAML/shell syntax and `git diff --check`: passed.
- Existing native NIH CLAP/VST3 release bundling: passed for this workspace.
- Compiled xtask Linux `bundle-au` rejection before Python/CMake: passed.
- Release archive script fixture retained executable modes and a symlink
  through tar upload transport, release ZIP creation and extraction: passed.
- Windows execution: not run on this Linux machine. Existing Windows job
  commands and format exports are retained.
- macOS compilation, signatures, architecture inspection, auval, AUv3
  registration/loading and host/GUI testing: not run; require macOS CI or
  a local Mac. No remote CI run was triggered by this implementation.

AUv3 without an Apple account: infrastructure implemented with ad-hoc
signing, but no real macOS build/registration result is available from this
Linux session. The absence of an Apple account is intentional.

An additional Cargo-alias rejection check waited on an existing release build
directory lock and was stopped. The compiled xtask rejection check passed;
no other build process was interrupted.
