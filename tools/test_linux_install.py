#!/usr/bin/env python3
"""Installer integration checks in temporary directories; never installs a plugin."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

PROJECT = Path(__file__).resolve().parents[1]


class LinuxInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="gainstagefx installer ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / "custom cargo target"
        self.source = self.target / "bundled"
        self.source.mkdir(parents=True)
        (self.source / "GainStageFx.clap").write_bytes(b"new plugin")
        binary = self.source / "GainStageFx.vst3/Contents/x86_64-linux/GainStageFx.so"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"new plugin")
        for name in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
            (self.source / name).write_text("test notice")
        self.clap = self.root / "clap plugins/BurningTreeC"
        self.vst3 = self.root / "vst3 plugins/BurningTreeC"
        self.env = dict(os.environ, CARGO_TARGET_DIR=str(self.target),
                        CLAP_PATH=str(self.clap.parent), VST3_PATH=str(self.vst3.parent))

    def run_installer(self, packaged=False, args=None):
        if packaged:
            script = self.source / "install.sh"
            shutil.copy2(PROJECT / ".github/install-Linux.sh", script)
            args = [] if args is None else args
        else:
            script = PROJECT / "install.sh"
            args = ["--no-build"] if args is None else args
        return subprocess.run(["bash", str(script), *args], env=self.env,
                              capture_output=True, text=True, check=False)

    def seed_existing(self):
        self.clap.mkdir(parents=True)
        (self.clap / "GainStageFx.clap").write_bytes(b"old plugin")
        old = self.vst3 / "GainStageFx.vst3/Contents/x86_64-linux/GainStageFx.so"
        old.parent.mkdir(parents=True)
        old.write_bytes(b"old plugin")

    def test_source_installer_uses_cargo_target_and_copies_both_formats(self):
        self.seed_existing()
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.clap / "GainStageFx.clap").read_bytes(), b"new plugin")
        self.assertEqual((self.vst3 / "GainStageFx.vst3/Contents/x86_64-linux/GainStageFx.so").read_bytes(), b"new plugin")
        self.assertTrue((self.clap / "LICENSE").is_file())
        self.assertTrue((self.vst3 / "THIRD-PARTY-NOTICES.md").is_file())
        self.assertIn("Restart REAPER", result.stdout)

    def test_missing_vst3_leaves_existing_clap_intact(self):
        self.seed_existing()
        shutil.rmtree(self.source / "GainStageFx.vst3")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.clap / "GainStageFx.clap").read_bytes(), b"old plugin")

    def test_copy_failure_leaves_both_installed_formats_intact(self):
        self.seed_existing()
        commands = self.root / "commands"
        commands.mkdir()
        copy = commands / "cp"
        copy.write_text('#!/bin/bash\nfor arg in "$@"; do\n  case "$arg" in *GainStageFx.vst3) exit 42;; esac\ndone\nexec /usr/bin/cp "$@"\n')
        copy.chmod(0o755)
        self.env["PATH"] = str(commands) + os.pathsep + self.env["PATH"]
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.clap / "GainStageFx.clap").read_bytes(), b"old plugin")
        self.assertEqual((self.vst3 / "GainStageFx.vst3/Contents/x86_64-linux/GainStageFx.so").read_bytes(), b"old plugin")
        self.assertFalse(list(self.clap.glob(".gainstagefx.*")))
        self.assertFalse(list(self.vst3.glob(".gainstagefx.*")))

    def test_search_path_list_is_rejected_before_install(self):
        self.env["CLAP_PATH"] += ":/another/path"
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("search-path list", result.stderr)
        self.assertFalse(self.clap.exists())

    def test_packaged_installer_copies_plugins_and_notices(self):
        result = self.run_installer(packaged=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.clap / "GainStageFx.clap").read_bytes(), b"new plugin")
        self.assertEqual((self.vst3 / "THIRD-PARTY-NOTICES.md").read_text(), "test notice")

    def test_system_installer_rejects_extra_arguments_before_install(self):
        result = self.run_installer(packaged=True, args=["--system", "typo"])
        self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
