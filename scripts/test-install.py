#!/usr/bin/env python3
"""Exercise the real POSIX installer against disposable command fixtures."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

INSTALLER = Path(__file__).resolve().with_name("install.sh")


class InstallerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="pinst-install-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = dict(os.environ, HOME=str(self.root),
                        PATH=f"{self.bin}:/usr/bin:/bin",
                        PINST_VERSION="v1.0.0", PINST_REPO="https://example.invalid/repo",
                        PINST_SRC_DIR=str(self.root / "source"),
                        PINST_INSTALL_DIR=str(self.root / "installed"),
                        CARGO_TARGET_DIR=str(self.root / "target"),
                        CALLS=str(self.root / "calls"))
        self.tool("curl", 'exit 22')
        self.tool("git", '''printf '%s\\n' "$*" >> "$CALLS"
if [ "$1" = clone ]; then
  for last; do :; done
  mkdir -p "$last/.git"
fi''')
        self.tool("cargo", '''mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\\nexit 0\\n' > "$CARGO_TARGET_DIR/release/pinst"''')

    def tool(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
        path.chmod(0o755)

    def run_install(self):
        return subprocess.run(["sh", str(INSTALLER)], cwd=self.root,
                              env=self.env, text=True, capture_output=True)

    def test_failed_download_fallback_clones_requested_tag(self):
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("clone --depth=1 --branch v1.0.0", (self.root / "calls").read_text())
        self.assertTrue((self.root / "installed/pinst").is_file())

    def test_existing_source_fetches_requested_tag(self):
        (self.root / "source/.git").mkdir(parents=True)
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = (self.root / "calls").read_text()
        self.assertIn("fetch --depth=1 origin refs/tags/v1.0.0", calls)
        self.assertIn("checkout --detach FETCH_HEAD", calls)
        self.assertNotIn("pull", calls)

    def test_bad_checksum_never_builds_or_replaces_existing_binary(self):
        self.tool("curl", '''while [ $# -gt 0 ]; do
if [ "$1" = -o ]; then printf 'corrupt\\n' > "$2"; exit; fi
shift
done
exit 2''')
        destination = self.root / "installed/pinst"
        destination.parent.mkdir()
        destination.write_text("old binary")
        result = self.run_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(destination.read_text(), "old binary")
        self.assertFalse((self.root / "calls").exists())

    def test_nix_preview_does_not_download_or_create_state(self):
        script = INSTALLER.with_name("bootstrap-nix.sh")
        result = subprocess.run(["sh", str(script), "--dry-run"],
                                cwd=self.root, env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Build pinst", result.stdout)
        self.assertFalse((self.root / ".local/state").exists())

    def test_intel_nix_installer_rejects_modified_script(self):
        self.tool("uname", 'case "$1" in -s) echo Darwin;; -m) echo x86_64;; esac')
        self.tool("sudo", 'exit 0')
        self.tool("curl", '''while [ $# -gt 0 ]; do
if [ "$1" = -o ]; then printf 'exit 0\\n' > "$2"; exit; fi
shift
done
exit 2''')
        result = subprocess.run(["sh", str(INSTALLER.with_name("bootstrap-nix.sh")), "--yes"],
                                cwd=self.root, env=self.env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 mismatch", result.stderr)
        self.assertFalse((self.root / ".local/state").exists())


if __name__ == "__main__":
    unittest.main()
