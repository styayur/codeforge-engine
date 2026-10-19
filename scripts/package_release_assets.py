#!/usr/bin/env python3
"""Create normalized CLI archives and installer assets for a release."""

from __future__ import annotations

import argparse
import sys
import os
import re
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"release packaging error: {message}")


def normalized(name: str) -> str:
    result = re.sub(r"\s+", ".", name.strip())
    result = re.sub(r"[^A-Za-z0-9._-]", ".", result)
    return result.strip(".")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", required=True, choices=("windows", "linux"))
    parser.add_argument("--tag", required=True)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--bundle-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    tag = args.tag.strip()
    if not tag.startswith("v"):
        fail(f"tag must start with v: {tag!r}")
    cli = args.cli.resolve()
    bundle = args.bundle_dir.resolve()
    output = args.output.resolve()
    if not cli.is_file():
        fail(f"CLI binary does not exist: {cli}")
    if not bundle.is_dir():
        fail(f"bundle directory does not exist: {bundle}")
    if output.name != "artifacts" or not output.is_relative_to(root):
        fail(f"output must be an artifacts directory inside the repository: {output}")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)

    release_notes = root / "docs" / "releases" / f"{tag}.md"
    if not release_notes.is_file():
        fail(f"release notes do not exist: {release_notes}")
    for required in (root / "README.md", root / "LICENSE"):
        if not required.is_file():
            fail(f"required distribution file does not exist: {required}")

    if args.platform == "windows":
        suffix = ".exe"
        expected_installers = {".msi", ".exe"}
        target = "x86_64-pc-windows-msvc"
        archive_name = f"codeforge-cli-{tag}-{target}.zip"
        archive_root = f"codeforge-{tag}-{target}"
    else:
        suffix = ""
        expected_installers = {".appimage", ".deb"}
        target = "x86_64-unknown-linux-gnu"
        archive_name = f"codeforge-cli-{tag}-{target}.tar.gz"
        archive_root = f"codeforge-{tag}-{target}"

    if cli.suffix.lower() != suffix:
        fail(f"CLI binary suffix {cli.suffix!r} does not match platform {args.platform!r}")

    with tempfile.TemporaryDirectory(prefix="codeforge-release-") as temporary:
        staging = Path(temporary) / archive_root
        staging.mkdir()
        shutil.copy2(cli, staging / cli.name)
        shutil.copy2(root / "README.md", staging / "README.md")
        shutil.copy2(root / "LICENSE", staging / "LICENSE")
        shutil.copy2(release_notes, staging / "RELEASE_NOTES.md")

        archive = output / archive_name
        if args.platform == "windows":
            with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle_file:
                for path in sorted(staging.iterdir()):
                    bundle_file.write(path, arcname=f"{archive_root}/{path.name}")
        else:
            os.chmod(staging / cli.name, 0o755)
            with tarfile.open(archive, "w:gz") as bundle_file:
                for path in sorted(staging.iterdir()):
                    info = bundle_file.gettarinfo(str(path), arcname=f"{archive_root}/{path.name}")
                    info.mode = 0o755 if path.name == cli.name else 0o644
                    with path.open("rb") as source:
                        bundle_file.addfile(info, source)

    found = [path for path in bundle.rglob("*") if path.is_file()]
    installers = [path for path in found if path.suffix.lower() in expected_installers]
    if not installers:
        fail(f"no {args.platform} installer assets found under {bundle}")
    seen: set[str] = set()
    for installer in installers:
        destination_name = normalized(installer.name)
        if not destination_name or destination_name in seen:
            fail(f"installer names are empty or collide after normalization: {installer.name!r}")
        seen.add(destination_name)
        shutil.copy2(installer, output / destination_name)

    for path in sorted(output.iterdir()):
        sys.stdout.write(f"{path.name}\t{path.stat().st_size} bytes\n")


if __name__ == "__main__":
    main()
