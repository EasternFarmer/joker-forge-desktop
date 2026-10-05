"""Check extracted Linux releases without requiring FUSE.

Usage: python3 scripts/check-linux-appimage.py path/to/*.AppImage
       python3 scripts/check-linux-appimage.py --appdir path/to/AppDir

Bundled libwayland-client can conflict with the host's Mesa drivers. Tauri's
bundler must exclude it: https://github.com/tauri-apps/tauri/issues/15976
Other runtime libraries, including WebKit, are deliberately allowed.
"""

import argparse
from pathlib import Path
import os
import subprocess
import tempfile


class ValidationError(RuntimeError):
    pass


def is_executable(path):
    # Windows fixture runs cannot represent Linux executable permissions.
    return path.is_file() and (os.name != "posix" or bool(path.stat().st_mode & 0o111))


def validate_appdir(appdir):
    appdir = Path(appdir)
    if not appdir.is_dir():
        raise ValidationError(f"AppDir does not exist: {appdir}")

    # rglob includes broken symlinks and entries below nested directories.
    entries = list(appdir.rglob("*"))
    bundled_clients = sorted(
        str(path.relative_to(appdir))
        for path in entries
        if path.name.startswith("libwayland-client.so")
    )
    if bundled_clients:
        raise ValidationError(
            "AppImage bundles libwayland-client, which can conflict with host Mesa. "
            "Rebuild with the fixed Tauri bundler. Found: " + ", ".join(bundled_clients)
        )

    if not is_executable(appdir / "AppRun"):
        raise ValidationError("AppDir must contain an executable AppRun entry point")
    application_dir = appdir / "usr" / "bin"
    if not application_dir.is_dir() or not any(
        is_executable(path) for path in application_dir.iterdir()
    ):
        raise ValidationError("AppDir must contain an application executable in usr/bin")
    return len(entries)


def validate_appimage(appimage):
    appimage = Path(appimage).resolve()
    if not appimage.is_file():
        raise ValidationError(f"AppImage does not exist: {appimage}")
    with tempfile.TemporaryDirectory(prefix="joker-forge-appimage-") as directory:
        try:
            result = subprocess.run(
                [str(appimage), "--appimage-extract"],
                cwd=directory,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                timeout=120,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ValidationError(f"Could not extract {appimage.name}: {error}") from error
        if result.returncode:
            detail = (result.stderr or "").strip()[-1000:]
            raise ValidationError(
                f"Could not extract {appimage.name} (exit {result.returncode}): {detail}"
            )
        return validate_appdir(Path(directory) / "squashfs-root")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("appimages", nargs="*", type=Path)
    parser.add_argument("--appdir", type=Path, help="check an already extracted AppDir")
    args = parser.parse_args(argv)
    if bool(args.appimages) == bool(args.appdir):
        parser.error("provide AppImage paths or --appdir, exclusively")
    try:
        if args.appdir:
            count = validate_appdir(args.appdir)
            print(f"Verified {args.appdir}: {count} entries; no bundled libwayland-client")
        else:
            for appimage in args.appimages:
                count = validate_appimage(appimage)
                print(f"Verified {appimage.name}: {count} entries; no bundled libwayland-client")
    except (ValidationError, OSError) as error:
        parser.exit(1, f"AppImage check failed: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
