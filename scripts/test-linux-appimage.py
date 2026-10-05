"""Regression checks for the Linux AppImage release guard."""

from contextlib import redirect_stderr, redirect_stdout
import importlib.util
import io
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("appimage_check", Path(__file__).with_name("check-linux-appimage.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class AppImageChecks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "AppDir"
        self.application = self.write("usr/bin/application", b"application")
        self.write("AppRun", b"entry point")

    def write(self, name, content=b"library"):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        path.chmod(0o755)
        return path

    def test_webkit_and_other_wayland_libraries_are_allowed(self):
        for name in ("libwebkit2gtk-4.1.so.0", "libwayland-server.so.0", "libwayland-egl.so.1"):
            self.write("usr/lib/" + name)
        self.assertGreater(check.validate_appdir(self.root), 3)

    def test_regular_and_versioned_clients_at_any_depth_are_rejected(self):
        for name in ("libwayland-client.so", "usr/lib/libwayland-client.so.0.23.1", "nested/lib/libwayland-client.so.0"):
            with self.subTest(name=name):
                path = self.write(name)
                with self.assertRaisesRegex(check.ValidationError, "host Mesa"):
                    check.validate_appdir(self.root)
                path.unlink()

    def test_broken_client_symlink_is_rejected(self):
        link = self.root / "usr/lib/nested/libwayland-client.so.0"
        link.parent.mkdir(parents=True)
        try:
            link.symlink_to("missing-library")
        except OSError as error:
            self.skipTest(f"Symlink creation unavailable on this host: {error}")
        with self.assertRaisesRegex(check.ValidationError, "libwayland-client.so.0"):
            check.validate_appdir(self.root)

    def test_missing_entry_point_or_application_is_rejected(self):
        (self.root / "AppRun").unlink()
        with self.assertRaisesRegex(check.ValidationError, "AppRun"):
            check.validate_appdir(self.root)
        self.write("AppRun")
        self.application.unlink()
        with self.assertRaisesRegex(check.ValidationError, "usr/bin"):
            check.validate_appdir(self.root)

    def test_no_input_and_conflicting_inputs_are_rejected(self):
        for argv in ([], ["image.AppImage", "--appdir", str(self.root)]):
            with self.subTest(argv=argv), redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as result:
                    check.main(argv)
                self.assertEqual(result.exception.code, 2)

    def test_missing_image_does_not_start_extraction(self):
        with patch.object(check.subprocess, "run") as run:
            with self.assertRaisesRegex(check.ValidationError, "does not exist"):
                check.validate_appimage(self.root / "missing.AppImage")
            run.assert_not_called()

    def test_extraction_failure_cleans_temporary_directory(self):
        image = self.write("fixture.AppImage")
        directories = []
        def fail(command, **kwargs):
            directories.append(Path(kwargs["cwd"]))
            (directories[-1] / "partial-output").write_text("partial")
            self.assertEqual(command, [str(image.resolve()), "--appimage-extract"])
            return subprocess.CompletedProcess(command, 1, stderr="broken image")
        with patch.object(check.subprocess, "run", side_effect=fail):
            with self.assertRaisesRegex(check.ValidationError, "broken image"):
                check.validate_appimage(image)
        self.assertFalse(directories[0].exists())

    def test_successful_extraction_checks_contents_and_cleans_up(self):
        image = self.write("fixture.AppImage")
        directories = []
        def extract(command, **kwargs):
            directories.append(Path(kwargs["cwd"]))
            shutil.copytree(self.root, directories[-1] / "squashfs-root")
            return subprocess.CompletedProcess(command, 0, stderr="")
        with patch.object(check.subprocess, "run", side_effect=extract):
            self.assertGreater(check.validate_appimage(image), 0)
        self.assertFalse(directories[0].exists())

    def test_appdir_command_succeeds(self):
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(check.main(["--appdir", str(self.root)]), 0)
        self.assertIn("no bundled libwayland-client", output.getvalue())


if __name__ == "__main__":
    unittest.main()
