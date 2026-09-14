#!/usr/bin/env python3
"""Build-time packaging only. No code here runs inside a plugin or audio callback."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shlex
import shutil
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
LSREGISTER = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(args, *, cwd=ROOT, env=None, log=None, check=True, timeout=None, quiet=False):
    args = [str(a) for a in args]
    print("+ " + shlex.join(args), flush=True)
    if log:
        log.parent.mkdir(parents=True, exist_ok=True)
        with log.open("w") as out:
            out.write("+ " + shlex.join(args) + "\n")
            out.flush()
            try:
                result = subprocess.run(args, cwd=cwd, env=env, text=True,
                                        stdout=out, stderr=subprocess.STDOUT, timeout=timeout)
            except subprocess.TimeoutExpired:
                out.write(f"\nTIMEOUT after {timeout} seconds\n")
                result = subprocess.CompletedProcess(args, 124)
        print(log.read_text(), flush=True)
    else:
        result = subprocess.run(args, cwd=cwd, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        if not quiet:
            print(result.stdout, end="", flush=True)
    if check:
        require(result.returncode == 0, f"Command failed ({result.returncode}): {shlex.join(args)}")
    return result


def apple_version(version):
    # Do not silently clamp bytes (upstream AU component versions use 8-bit parts),
    # or collapse prereleases into releases with identical compatibility versions.
    require(re.fullmatch(r"\d+\.\d+\.\d+", version), f"Release version needs three numeric parts: {version}")
    require(all(int(n) <= 255 for n in version.split(".")), f"AU version part exceeds 255: {version}")
    return version


def catalog():
    data = json.loads((HERE / "plugins.json").read_text())
    require(re.fullmatch(r"[A-Za-z0-9]{4}", data["manufacturer"]), "Invalid manufacturer FourCC")
    for key in ("package", "name", "clap_id", "subtype"):
        values = [p[key] for p in data["plugins"]]
        require(len(set(values)) == len(values), f"Duplicate {key}")
    for p in data["plugins"]:
        require(re.fullmatch(r"[A-Za-z0-9]{4}", p["subtype"]), "Invalid subtype FourCC")
        require(p["clap_id"].startswith("com.burningtreec."), "Unexpected vendor namespace")
    return data


def plan(packages, all_plugins, root=ROOT):
    config = catalog()
    # cargo metadata handles workspace-inherited versions, custom target dirs, etc.
    metadata = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version=1", "--offline"], cwd=root, quiet=True).stdout)
    members = {p["name"]: p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    available = [p for p in config["plugins"] if p["package"] in members]
    if packages == ["comp76fx"]:
        packages = [p["package"] for p in available if p["package"].startswith("comp76fx_")]
    require(all_plugins != bool(packages), "Choose a package (or comp76fx family), or --all")
    require(all(p in {q["package"] for q in available} for p in packages), "Unknown package in this workspace")
    selected = available if all_plugins else [p for p in available if p["package"] in packages]
    require(selected, "No supported plugins in this workspace")
    result = dict(deployment_target=config["deployment_target"], plugins=[],
                  target_directory=metadata["target_directory"], formats=[], architectures=[])
    for plugin in selected:
        p = dict(plugin)
        p.update(version=apple_version(members[p["package"]]["version"]), bundle_id=p["clap_id"],
                 manufacturer=config["manufacturer"], vendor=config["vendor"], type="aufx")
        p["clap_path"] = str(Path(metadata["target_directory"]) / "bundled" / (p["name"] + ".clap"))
        result["plugins"].append(p)
    return result


def read_plist(bundle):
    path = bundle / "Contents/Info.plist"
    require(path.is_file(), f"Missing {path}")
    with path.open("rb") as f:
        return plistlib.load(f)


def executable(bundle):
    name = read_plist(bundle)["CFBundleExecutable"]
    require(Path(name).name == name, f"Invalid bundle executable: {name}")
    path = bundle / "Contents/MacOS" / name
    require(path.is_file(), f"Missing executable {path}")
    return path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sign(bundle):
    # Leaf CLAP -> extension -> app. Never use --deep to *sign* code.
    nested = [p for p in bundle.rglob("*") if p.is_dir() and p.suffix in (".clap", ".appex", ".framework", ".app")]
    for item in sorted(nested, key=lambda p: len(p.parts), reverse=True) + [bundle]:
        args = ["codesign", "--force", "--sign", "-"]
        if item.suffix == ".appex":
            args += ["--entitlements", HERE / "auv3.entitlements"]
        run(args + [item])
    verify_signature(bundle)


def verify_signature(bundle):
    run(["codesign", "--verify", "--deep", "--strict", "--verbose=4", bundle])
    details = run(["codesign", "-dv", "--verbose=4", bundle]).stdout
    require("Signature=adhoc" in details, f"Expected ad-hoc signature: {bundle}")
    if bundle.suffix == ".appex":
        ent = run(["codesign", "-d", "--entitlements", ":-", bundle]).stdout
        start = ent.find("<?xml")
        require(start >= 0, f"Missing AUv3 entitlements: {bundle}")
        parsed = plistlib.loads(ent[start:ent.index("</plist>", start) + len("</plist>")].encode())
        require(parsed.get("com.apple.security.app-sandbox") is True, "AUv3 sandbox entitlement missing")
        require(not any("application-identifier" in k or "team-identifier" in k for k in parsed), "Unexpected account entitlement")


def verify_architecture(binary, expected):
    actual = set(run(["lipo", "-archs", binary]).stdout.split())
    require(actual == set(expected), f"Architecture mismatch in {binary}: {actual} != {expected}")
    run(["file", binary])
    run(["otool", "-L", binary])
    # -L also lists LC_ID_DYLIB (the Rust cdylib's own install name). Only load
    # commands are dependencies. Inspect both universal slices, ignoring ID.
    commands = run(["otool", "-l", binary], quiet=True).stdout
    verify_load_commands(commands, binary)


def verify_load_commands(commands, binary):
    for command in commands.split("Load command "):
        if re.search(r"\bcmd LC_(?:LOAD_DYLIB|LOAD_WEAK_DYLIB|REEXPORT_DYLIB|LOAD_UPWARD_DYLIB)\b", command):
            match = re.search(r"\bname (.+) \(offset \d+\)", command)
            require(match is not None, f"Unrecognized Mach-O dependency in {binary}")
            dependency = match.group(1)
            require(dependency.startswith(("/System/Library/", "/usr/lib/")),
                    f"Non-system dynamic dependency needs bundling: {dependency} in {binary}")


def bundle_paths(dist, p, formats):
    base = dist / p["package"]
    result = [base / (p["name"] + suffix) for suffix in (".clap", ".vst3")]
    if "auv2" in formats:
        result.append(base / (p["name"] + ".component"))
    if "auv3" in formats:
        result.append(base / "auv3" / (p["name"] + " AUv3.app"))
    return result


def verify_metadata(bundle, p, format_name):
    info = read_plist(bundle)
    suffix = {"auv2": ".auv2", "auv3": ".auv3.extension", "app": ".auv3"}[format_name]
    require(info["CFBundleIdentifier"] == p["bundle_id"] + suffix, f"Wrong bundle ID in {bundle}")
    name = p["name"] + (" AUv3" if format_name == "app" else "")
    require(info["CFBundleName"] == name, f"Wrong plugin name in {bundle}")
    for key in ("CFBundleVersion", "CFBundleShortVersionString"):
        require(info[key] == p["version"], f"Wrong {key} in {bundle}")
    require(info["CFBundlePackageType"] == {"auv2": "BNDL", "auv3": "XPC!", "app": "APPL"}[format_name], f"Wrong bundle type in {bundle}")
    if format_name != "app":
        if format_name == "auv3":
            ext = info["NSExtension"]
            require(ext["NSExtensionPointIdentifier"] == "com.apple.AudioUnit-UI", "Expected GUI AUv3 extension")
            components = ext["NSExtensionAttributes"]["AudioComponents"]
        else:
            components = info["AudioComponents"]
        require(len(components) == 1, "Expected one CLAP descriptor per AU")
        component = components[0]
        for key in ("type", "subtype", "manufacturer"):
            require(component[key] == p[key], f"Wrong AU {key}: {bundle}")
        major, minor, patch = map(int, p["version"].split("."))
        require(component["version"] == max((major << 16) + (minor << 8) + patch, 1), "Wrong AudioComponent version")
        require(component["name"] == p["vendor"] + ": " + p["name"], "Wrong AudioComponent name")


def verify(dist, manifest):
    for p in manifest["plugins"]:
        canonical = dist / p["package"] / (p["name"] + ".clap")
        for top in bundle_paths(dist, p, manifest["formats"]):
            require(top.is_dir(), f"Missing artifact {top}")
            bundles = [top] + [q for q in top.rglob("*") if q.is_dir() and q.suffix in (".clap", ".appex")]
            for bundle in bundles:
                verify_signature(bundle)
                verify_architecture(executable(bundle), manifest["architectures"])
                if bundle.suffix == ".clap":
                    require(digest(executable(bundle)) == digest(executable(canonical)), f"Embedded CLAP differs: {bundle}")
                elif bundle.suffix in (".component", ".appex"):
                    kind = "auv2" if bundle.suffix == ".component" else "auv3"
                    verify_metadata(bundle, p, kind)
                    embedded = bundle / "Contents/PlugIns" / (p["name"] + ".clap")
                    require(embedded.is_dir() and not embedded.is_symlink(), f"Missing self-contained CLAP: {bundle}")
                elif bundle.suffix == ".app":
                    verify_metadata(bundle, p, "app")
                    extensions = list((bundle / "Contents/PlugIns").glob("*.appex"))
                    require(len(extensions) == 1 and extensions[0].name == p["name"] + ".appex", "Missing embedded AUv3 extension")


def copy_bundle(source, dest):
    require(source.is_dir(), f"Missing source bundle {source}")
    if dest.exists():
        shutil.rmtree(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    run(["ditto", source, dest])


def build(args):
    manifest = plan(args.packages, args.all)
    manifest["architectures"] = ["arm64", "x86_64"] if args.arch == "universal" else ["arm64"]
    manifest["formats"] = ["auv2", "auv3"] if args.format == "all" else [args.format]
    target = Path(manifest["target_directory"])
    work = target / "macos-au"
    logs = work / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    for command in (["sw_vers"], ["xcodebuild", "-version"], ["cmake", "--version"], ["rustc", "-Vv"]):
        run(command, log=logs / (command[0] + ".log"))
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET=manifest["deployment_target"])
    packages = [arg for p in manifest["plugins"] for arg in ("-p", p["package"])]
    cargo = ["cargo", "xtask", "bundle-universal" if args.arch == "universal" else "bundle", *packages]
    if args.arch == "arm64":
        cargo += ["--target", "aarch64-apple-darwin"]
    if args.release:
        cargo += ["--release"]
    run(cargo, env=env, log=logs / "nih-bundle.log")
    for p in manifest["plugins"]:
        verify_architecture(executable(Path(p["clap_path"])), manifest["architectures"])
    manifest_file = work / "build-manifest.json"
    manifest_file.write_text(json.dumps(manifest, indent=2) + "\n")
    dist = args.dist.resolve()
    # A new build gets a new manifest; do not accidentally publish an old AUv3.
    for p in manifest["plugins"]:
        base = dist / p["package"]
        if base.exists():
            shutil.rmtree(base)
        for suffix in (".clap", ".vst3"):
            dest = base / (p["name"] + suffix)
            copy_bundle(target / "bundled" / dest.name, dest)
            sign(dest)
    config = "Release" if args.release else "Debug"
    for kind in manifest["formats"]:
        build_dir = work / kind / (args.arch + "-" + config.lower())
        run(["cmake", "-S", HERE, "-B", build_dir, "-G", "Xcode",
             f"-DAU_MANIFEST={manifest_file}", f"-DAU_FORMAT={kind}",
             "-DCMAKE_OSX_ARCHITECTURES=" + ";".join(manifest["architectures"])],
            env=env, log=logs / (kind + "-configure.log"))
        run(["cmake", "--build", build_dir, "--config", config, "--", "CODE_SIGN_IDENTITY=-",
             "CODE_SIGN_STYLE=Manual", "DEVELOPMENT_TEAM=", "PROVISIONING_PROFILE_SPECIFIER=", "PROVISIONING_PROFILE="],
            env=env, log=logs / (kind + "-build.log"))
        for p in manifest["plugins"]:
            suffix = ".component" if kind == "auv2" else " AUv3.app"
            dest = dist / p["package"]
            if kind == "auv3":
                dest /= "auv3"
            dest /= p["name"] + suffix
            copy_bundle(build_dir / "products" / config / dest.name, dest)
            # Upstream's AUv3 helper has a literal 10.13 minimum. Normalize only
            # the staged bundle, before final signing, never Xcode's plist input.
            for bundle in [dest] + list(dest.rglob("*.appex")):
                info = read_plist(bundle)
                info["LSMinimumSystemVersion"] = manifest["deployment_target"]
                with (bundle / "Contents/Info.plist").open("wb") as f:
                    plistlib.dump(info, f)
            sign(dest)
        partial = dict(manifest, formats=manifest["formats"][:manifest["formats"].index(kind) + 1])
        (dist / "manifest.json").write_text(json.dumps(partial, indent=2) + "\n")
        verify(dist, partial)
    # Carry licenses for everything linked by the wrapper; SDKs are fetched
    # separately for AUv2/AUv3, so each relevant source is available here.
    licenses = dist / "licenses"
    licenses.mkdir(exist_ok=True)
    for name in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
        shutil.copy2(ROOT / name, licenses / name)
    for kind in manifest["formats"]:
        deps = work / kind / (args.arch + "-" + config.lower()) / "_deps"
        for dep in ("clap_wrapper", "clap_sdk", "audiounit_sdk"):
            for license_file in (deps / (dep + "-src")).glob("LICENSE*"):
                shutil.copy2(license_file, licenses / (dep + "-" + license_file.name))
    shutil.copy2(HERE / "dependencies.json", licenses / "dependencies.json")
    shutil.copy2(HERE / "WRAPPER-NOTICES.md", licenses / "WRAPPER-NOTICES.md")
    print(f"Verified distribution: {dist}")


def install(dist, manifest, kind):
    require(kind == "all" or kind in manifest["formats"], f"No {kind} product in this distribution")
    home = Path.home()
    for p in manifest["plugins"]:
        if kind in ("all", "auv2") and "auv2" in manifest["formats"]:
            src = dist / p["package"] / (p["name"] + ".component")
            dest = home / "Library/Audio/Plug-Ins/Components" / src.name
            copy_bundle(src, dest)
            verify_signature(dest)
        if kind in ("all", "auv3") and "auv3" in manifest["formats"]:
            src = dist / p["package"] / "auv3" / (p["name"] + " AUv3.app")
            dest = home / "Applications" / src.name
            copy_bundle(src, dest)
            verify_signature(dest)
            # Registration is tested separately, so build/install alone cannot
            # be mistaken for a successfully loaded AUv3.
            print(f"Containing app installed: {dest}; run the register command next.")


def register(manifest, logs):
    results = {}
    for p in manifest["plugins"]:
        app = Path.home() / "Applications" / (p["name"] + " AUv3.app")
        extension = app / "Contents/PlugIns" / (p["name"] + ".appex")
        verify_signature(app)
        records = []
        for label, command in (("launchservices", [LSREGISTER, "-f", app]),
                               ("pluginkit-add", ["pluginkit", "-a", extension])):
            records.append(run(command, log=logs / (p["package"] + "-" + label + ".log"), check=False).returncode)
        identifier = p["bundle_id"] + ".auv3.extension"
        # LaunchServices discovery can be asynchronous. Bounded retry, no cache deletion.
        found = False
        for _ in range(10):
            result = run(["pluginkit", "-m", "-v", "-i", identifier],
                         log=logs / (p["package"] + "-pluginkit-match.log"), check=False)
            found = result.returncode == 0 and identifier in (logs / (p["package"] + "-pluginkit-match.log")).read_text().split("\n", 1)[1]
            if found:
                break
            time.sleep(1)
        results[p["package"]] = {"commands": records, "registered": found,
                                  "loaded": "not tested by registration"}
    (logs / "registration.json").write_text(json.dumps(results, indent=2) + "\n")
    return all(r["registered"] for r in results.values())


def validate(manifest, logs):
    results = {}
    for p in manifest["plugins"]:
        # With both formats installed auval may choose either for the shared
        # component tuple. The separate probe selects and reports the AU version.
        result = run(["auval", "-v", p["type"], p["subtype"], p["manufacturer"]],
                     log=logs / (p["package"] + "-auval.log"), check=False, timeout=300)
        results[p["package"]] = {"auval_exit": result.returncode}
    (logs / "auval.json").write_text(json.dumps(results, indent=2) + "\n")
    require(all(r["auval_exit"] == 0 for r in results.values()), "auval failed; see per-plugin logs")


def probe(dist, manifest, kind, logs, allow_unregistered):
    require(kind in ("auv2", "auv3"), "Select --format auv2 or --format auv3 for the probe")
    target = Path(manifest["target_directory"]) / "macos-au"
    includes = list((target / kind).glob("*/_deps/clap_sdk-src/include"))
    require(includes, "Build this format before running its probe")
    binary = target / "au-probe"
    run(["xcrun", "clang++", "-std=c++17", "-fobjc-arc", HERE / "probe.mm", "-I", includes[0],
         "-framework", "AVFoundation", "-framework", "AudioToolbox", "-framework", "Foundation", "-o", binary],
        log=logs / "probe-build.log")
    run(["codesign", "--force", "--sign", "-", binary])
    results = {}
    registration = json.loads((logs / "registration.json").read_text()) if kind == "auv3" else {}
    for p in manifest["plugins"]:
        if kind == "auv3" and not registration.get(p["package"], {}).get("registered"):
            results[p["package"]] = {"status": "UNTESTED: AUv3 not registered"}
            continue
        result = run([binary, dist / "manifest.json", p["package"], kind,
                      dist / p["package"] / (p["name"] + ".clap"), logs / (p["package"] + "-" + kind + "-probe.json")],
                     log=logs / (p["package"] + "-" + kind + "-probe.log"), check=False, timeout=300)
        results[p["package"]] = {"exit": result.returncode}
    (logs / (kind + "-probe-summary.json")).write_text(json.dumps(results, indent=2) + "\n")
    require(all(r.get("exit") == 0 or ("status" in r and allow_unregistered) for r in results.values()),
            "AU probe failed or unregistered; see per-plugin results")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "plan", "verify", "sign", "install", "register", "validate", "probe"))
    parser.add_argument("packages", nargs="*")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--arch", choices=("arm64", "universal"), default="universal")
    parser.add_argument("--format", choices=("all", "auv2", "auv3"), default="all")
    parser.add_argument("--dist", type=Path, default=ROOT / "dist/macos")
    parser.add_argument("--logs", type=Path, default=ROOT / "target/macos-au/logs")
    parser.add_argument("--allow-unregistered-auv3", action="store_true")
    args = parser.parse_args()
    if args.command == "plan":
        print(json.dumps(plan(args.packages, args.all), indent=2))
        return
    require(platform.system() == "Darwin", "Audio Unit commands require macOS; ordinary bundle commands remain unchanged")
    if args.command == "build":
        build(args)
        return
    if args.command == "sign":
        require(args.packages, "Pass a bundle path to sign")
        for path in args.packages:
            sign(Path(path).resolve())
        return
    manifest = json.loads((args.dist / "manifest.json").read_text())
    args.logs.mkdir(parents=True, exist_ok=True)
    if args.command == "verify":
        verify(args.dist, manifest)
    elif args.command == "install":
        install(args.dist, manifest, args.format)
    elif args.command == "register":
        require("auv3" in manifest["formats"], "No AUv3 product in this distribution")
        ok = register(manifest, args.logs)
        if not ok and args.allow_unregistered_auv3:
            print("WARNING: AUv3 registration failed. Artifacts built; AUv3 usability UNCONFIRMED. See registration.json and exact tool logs.")
        else:
            require(ok, "AUv3 registration failed; see exact tool logs. No Apple account fallback is used.")
    elif args.command == "validate":
        validate(manifest, args.logs)
    elif args.command == "probe":
        probe(args.dist, manifest, args.format, args.logs, args.allow_unregistered_auv3)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        sys.exit(f"Audio Unit packaging: {error}")
