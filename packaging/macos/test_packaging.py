"""Packaging checks, deliberately outside the plugin's src/ and tests/."""
import copy
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import au


class PackagingTests(unittest.TestCase):
    def test_discovery_waits_for_every_exact_component(self):
        config = au.catalog()
        plugins = [dict(p, type="aufx", manufacturer=config["manufacturer"]) for p in config["plugins"]]
        manifest = {"plugins": plugins}
        calls = []
        def listing(args, **kwargs):
            calls.append(args)
            entries = plugins if len(calls) > 1 else []
            kwargs["log"].write_text("\n".join(
                f'{p["type"]} {p["subtype"]} {p["manufacturer"]} - {p["name"]}' for p in entries))
            return subprocess.CompletedProcess(args, 0)
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(au, "run", side_effect=listing), patch.object(au.time, "sleep"):
                au.wait_for_components(manifest, Path(directory))
            self.assertEqual(len(calls), 2)
            def missing(args, **kwargs):
                kwargs["log"].write_text("aufx XXXX YYYY - unrelated plugin\n")
                return subprocess.CompletedProcess(args, 0)
            with patch.object(au, "run", side_effect=missing), patch.object(au.time, "sleep"):
                with self.assertRaisesRegex(RuntimeError, "not discovered"):
                    au.wait_for_components(manifest, Path(directory))

    def test_metadata_matches_this_workspace(self):
        manifest = au.plan([], True)
        self.assertEqual({p["package"] for p in manifest["plugins"]},
                         {p["package"] for p in au.catalog()["plugins"]})
        bundler = (au.ROOT / "bundler.toml").read_text()
        for p in manifest["plugins"]:
            self.assertIn(f'[{p["package"]}]\nname = "{p["name"]}"', bundler)
            self.assertEqual(p["type"], "aufx")
            self.assertEqual(p["bundle_id"], p["clap_id"])

    def test_au_version_does_not_silently_alias_releases(self):
        self.assertEqual(au.apple_version("0.7.0"), "0.7.0")
        self.assertEqual(au.apple_version("0.6.0"), "0.6.0")
        for invalid in ("1", "1.2", "1.2.3-beta", "1.2.3+local", "1.256.0"):
            with self.assertRaises(RuntimeError):
                au.apple_version(invalid)

    def test_unknown_package_fails(self):
        with self.assertRaisesRegex(RuntimeError, "Unknown package"):
            au.plan(["not-a-plugin"], False)

    def test_pins_are_full_commits(self):
        pins = json.loads((au.HERE / "dependencies.json").read_text())
        for value in pins.values():
            self.assertRegex(value["commit"], r"^[0-9a-f]{40}$")
            self.assertNotIn(value["version"], ("main", "master"))

    def test_nonmac_build_fails_before_tools_run(self):
        with patch.object(sys, "argv", ["au.py", "build", "--all"]), \
             patch.object(au.platform, "system", return_value="Windows"), \
             patch.object(au, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "require macOS"):
                au.main()
            run.assert_not_called()

    def test_rejects_single_slice_when_universal_requested(self):
        with patch.object(au, "run", return_value=subprocess.CompletedProcess([], 0, "arm64\n")):
            with self.assertRaisesRegex(RuntimeError, "Architecture mismatch"):
                au.verify_architecture(Path("binary"), ["arm64", "x86_64"])

    def test_rejects_non_adhoc_signature(self):
        with patch.object(au, "run", return_value=subprocess.CompletedProcess([], 0, "Signature size=123\n")):
            with self.assertRaisesRegex(RuntimeError, "ad-hoc"):
                au.verify_signature(Path("plugin.component"))

    def test_macho_identity_is_not_an_external_dependency(self):
        commands = """Load command 1
          cmd LC_ID_DYLIB
          name /build/libplugin.dylib (offset 24)
        Load command 2
          cmd LC_LOAD_DYLIB
          name /usr/lib/libSystem.B.dylib (offset 24)
        """
        au.verify_load_commands(commands, "plugin")
        with self.assertRaisesRegex(RuntimeError, "Non-system dynamic dependency"):
            au.verify_load_commands(commands.replace("/usr/lib/libSystem.B.dylib", "@rpath/unbundled.dylib"), "plugin")

    @unittest.skipIf(sys.platform == "darwin", "This checks non-macOS rejection")
    def test_cmake_rejects_before_configuring_compilers_or_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(["cmake", "-S", str(au.HERE), "-B", directory],
                                    text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertRegex(result.stderr, "macOS-only|CMake 3.27 or higher is required")
            self.assertFalse((Path(directory) / "_deps").exists())

    def test_cmake_output_paths_and_auv2_plist_input(self):
        # Exercise the actual consumer CMake module using lightweight upstream
        # target stubs. Ninja Multi-Config exposes accidental Release/Release
        # suffixes without needing Xcode or compiling any plugin source.
        with tempfile.TemporaryDirectory(prefix="au cmake ") as directory:
            root = Path(directory)
            plugins = []
            for p in au.catalog()["plugins"]:
                clap = root / (p["name"] + ".clap")
                (clap / "Contents/MacOS").mkdir(parents=True)
                (clap / "Contents/MacOS" / p["name"]).write_bytes(b"CLAP test input")
                plugins.append(dict(p, clap_path=str(clap), bundle_id=p["clap_id"],
                                    version="0.8.0", manufacturer="BTrC", vendor="BurningTreeC"))
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps(dict(plugins=plugins)))
            (root / "stub.cpp").write_text("int main() { return 0; }\n")
            fixture = r'''
cmake_minimum_required(VERSION 3.18)
project(AUContract LANGUAGES CXX)
function(target_add_auv2_wrapper)
    cmake_parse_arguments(W "" "TARGET" "" ${ARGN})
    target_sources(${W_TARGET} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/stub.cpp")
    add_executable(${W_TARGET}-build-helper "${CMAKE_CURRENT_SOURCE_DIR}/stub.cpp")
    set_target_properties(${W_TARGET} PROPERTIES BUNDLE TRUE MACOSX_BUNDLE TRUE)
endfunction()
function(target_add_auv3_wrapper)
    cmake_parse_arguments(W "" "TARGET" "" ${ARGN})
    target_sources(${W_TARGET} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/stub.cpp")
    add_executable(${W_TARGET}-auv3-build-helper "${CMAKE_CURRENT_SOURCE_DIR}/stub.cpp")
endfunction()
function(target_add_auv3_standalone_wrapper)
    cmake_parse_arguments(W "" "TARGET" "" ${ARGN})
    target_sources(${W_TARGET} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/stub.cpp")
endfunction()
include("${AU_MODULE}")
file(READ "${AU_MANIFEST}" data)
string(JSON count LENGTH "${data}" plugins)
math(EXPR last "${count} - 1")
foreach(index RANGE ${last})
    string(JSON plugin GET "${data}" plugins ${index})
    string(JSON package GET "${plugin}" package)
    add_nih_clap_audio_unit("${plugin}")
    set(target "${package}_${AU_FORMAT}")
    file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/${package}-$<CONFIG>.txt"
        CONTENT "$<TARGET_PROPERTY:${target},LIBRARY_OUTPUT_DIRECTORY>\n$<TARGET_FILE_DIR:${target}>\n")
    if(AU_FORMAT STREQUAL "auv2")
        get_target_property(plist ${target} XCODE_ATTRIBUTE_INFOPLIST_FILE)
        get_target_property(app ${target} MACOSX_BUNDLE)
        get_target_property(bundle ${target} BUNDLE)
        get_target_property(product ${target} XCODE_PRODUCT_TYPE)
        if(app OR NOT bundle OR NOT product STREQUAL "com.apple.product-type.bundle")
            message(FATAL_ERROR "AUv2 must be a CFBundle, not an application")
        endif()
        if(NOT plist STREQUAL "${CMAKE_CURRENT_BINARY_DIR}/${target}-build-helper-output/auv2_Info.plist")
            message(FATAL_ERROR "Xcode must process the CLAP-generated AUv2 plist")
        endif()
    else()
        file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/${package}-app-$<CONFIG>.txt"
            CONTENT "$<TARGET_FILE_DIR:${package}_app>")
    endif()
endforeach()
'''
            (root / "CMakeLists.txt").write_text(fixture)
            for kind in ("auv2", "auv3"):
                for config in ("Debug", "Release"):
                    with self.subTest(format=kind, config=config):
                        build = root / (kind + config)
                        result = subprocess.run([
                            "cmake", "-S", str(root), "-B", str(build), "-G", "Ninja Multi-Config",
                            "-DAU_MODULE=" + str(au.HERE / "cmake/AddNihAudioUnit.cmake"),
                            "-DAU_MANIFEST=" + str(manifest), "-DAU_FORMAT=" + kind,
                            "-DAU_BUILD_CONFIG=" + config,
                        ], capture_output=True, text=True)
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                        expected = str(build / "products" / config)
                        for p in plugins:
                            paths = (build / (p["package"] + "-" + config + ".txt")).read_text().splitlines()
                            self.assertEqual(paths, [expected, expected])
                            if kind == "auv3":
                                self.assertEqual((build / (p["package"] + "-app-" + config + ".txt")).read_text(), expected)

    def test_bundle_metadata_and_embedded_clap(self):
        p = dict(au.catalog()["plugins"][0], version="0.7.0", type="aufx",
                 manufacturer="BTrC", vendor="BurningTreeC")
        p["bundle_id"] = p["clap_id"]
        with tempfile.TemporaryDirectory() as folder:
            dist = Path(folder)
            manifest = dict(plugins=[p], formats=["auv2", "auv3"], architectures=["arm64"])

            def bundle(path, suffix, package_type, app=False):
                (path / "Contents/MacOS").mkdir(parents=True)
                name = p["name"] + (" AUv3" if app else "")
                info = dict(CFBundleExecutable=name, CFBundleName=name,
                            CFBundleIdentifier=p["bundle_id"] + suffix,
                            CFBundleVersion=p["version"], CFBundleShortVersionString=p["version"],
                            CFBundlePackageType=package_type)
                if package_type in ("BNDL", "XPC!"):
                    component = {key: p[key] for key in ("type", "subtype", "manufacturer")}
                    component.update(name=p["vendor"] + ": " + p["name"], version=7 << 8)
                    if package_type == "BNDL":
                        info["AudioComponents"] = [component]
                    else:
                        info["NSExtension"] = dict(NSExtensionPointIdentifier="com.apple.AudioUnit-UI",
                                                   NSExtensionAttributes=dict(AudioComponents=[component]))
                (path / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
                (path / "Contents/MacOS" / name).write_bytes(b"same original CLAP bytes")
                return path

            clap, vst3, component, app = au.bundle_paths(dist, p, manifest["formats"])
            bundle(clap, ".clap", "BNDL")
            bundle(vst3, ".vst3", "BNDL")
            bundle(component, ".auv2", "BNDL")
            bundle(app, ".auv3", "APPL", True)
            extension = bundle(app / "Contents/PlugIns" / (p["name"] + ".appex"), ".auv3.extension", "XPC!")
            for parent in (component, extension):
                bundle(parent / "Contents/PlugIns" / clap.name, ".clap", "BNDL")
            with patch.object(au, "verify_signature"), patch.object(au, "verify_architecture"):
                au.verify(dist, manifest)
                embedded = extension / "Contents/PlugIns" / clap.name
                au.executable(embedded).write_bytes(b"wrong plugin")
                with self.assertRaisesRegex(RuntimeError, "Embedded CLAP differs"):
                    au.verify(dist, manifest)
            bad = copy.deepcopy(p)
            bad["subtype"] = "Oops"
            with self.assertRaisesRegex(RuntimeError, "Wrong AU subtype"):
                au.verify_metadata(component, bad, "auv2")


if __name__ == "__main__":
    unittest.main()
