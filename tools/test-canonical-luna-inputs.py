#!/usr/bin/env python3
"""Negative checks for the canonical input snapshot verifier."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

TOOL = Path(__file__).with_name("canonical-luna-inputs.py")
FONT = "fonts/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf"


class InputSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="luna-input-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for directory in ("niri-apks", "provider-apks", "provider-root/usr/lib",
                          "graphics/mesa/out/usr/local/lib", "graphics/noctalia-runtime-apks"):
            (self.root / directory).mkdir(parents=True)
        for name in ("alpine-runtime-rootfs.tar.gz", "kernel/bzImage", FONT,
                     "graphics/noctalia/out/usr/local/bin/noctalia"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"test fixture")
        self.binary = self.root / "graphics/noctalia/out/usr/local/bin/noctalia"
        self.binary.chmod(0o755)
        self.link = self.root / "provider-root/usr/lib/libexample.so"
        self.link.symlink_to("libexample.so.1")
        self.assertEqual(self.invoke("record").returncode, 0)

    def invoke(self, action="verify"):
        return subprocess.run([sys.executable, str(TOOL), action, str(self.root)],
                              capture_output=True, text=True)

    def test_unchanged_snapshot(self):
        self.assertEqual(self.invoke().returncode, 0)

    def test_missing_font(self):
        (self.root / FONT).unlink()
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_changed_binary(self):
        self.binary.write_bytes(b"different binary")
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_changed_executable_mode(self):
        self.binary.chmod(0o644)
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_changed_link(self):
        self.link.unlink()
        self.link.symlink_to("unrelated.so")
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_extra_input(self):
        (self.root / "provider-apks/unexpected.apk").write_bytes(b"unexpected")
        self.assertNotEqual(self.invoke().returncode, 0)


if __name__ == "__main__":
    unittest.main()
