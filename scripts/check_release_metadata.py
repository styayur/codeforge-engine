#!/usr/bin/env python3
"""Validate release version and companion metadata before packaging."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path


def fail(message: str) -> None:
    sys.stderr.write(f"release metadata error: {message}\n")
    raise SystemExit(1)


def local_workspace_packages(root: Path) -> dict[str, str]:
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    packages: dict[str, str] = {}
    for package in lock.get("package", []):
        name = package.get("name", "")
        if name.startswith("codeforge-") and "source" not in package:
            packages[name] = package.get("version", "")
    return packages


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", help="Optional Git tag to compare with the workspace version")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    version = workspace["workspace"]["package"]["version"]

    desktop = json.loads((root / "apps/desktop/package.json").read_text(encoding="utf-8"))
    tauri = json.loads(
        (root / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8")
    )

    errors: list[str] = []
    if desktop.get("version") != version:
        errors.append(
            f"apps/desktop/package.json is {desktop.get('version')!r}, expected {version!r}"
        )
    if tauri.get("version") != version:
        errors.append(
            f"apps/desktop/src-tauri/tauri.conf.json is {tauri.get('version')!r}, expected {version!r}"
        )
    for name, package_version in sorted(local_workspace_packages(root).items()):
        if package_version != version:
            errors.append(f"Cargo.lock package {name} is {package_version!r}, expected {version!r}")

    release_notes = root / "docs" / "releases" / f"v{version}.md"
    if not release_notes.is_file():
        errors.append(f"missing release notes: {release_notes.relative_to(root)}")
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    if not re.search(rf"^## v{re.escape(version)}(?:\s|$)", changelog, re.MULTILINE):
        errors.append(f"CHANGELOG.md has no v{version} section")

    if args.tag:
        expected_tag = f"v{version}"
        if args.tag != expected_tag:
            errors.append(f"Git tag is {args.tag!r}, expected {expected_tag!r}")

    if errors:
        for error in errors:
            sys.stderr.write(f"release metadata error: {error}\n")
        raise SystemExit(1)

    sys.stdout.write(f"release metadata consistent: v{version}\n")
    if args.tag:
        sys.stdout.write(f"Git tag validated: {args.tag}\n")


if __name__ == "__main__":
    main()
