#!/usr/bin/env python3
"""Generate or verify SHA-256 checksums for release assets."""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def parse_checksums(path: Path) -> dict[str, str]:
    expected: dict[str, str] = {}
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split("  ", 1)
        if len(parts) != 2 or len(parts[0]) != 64:
            raise SystemExit(f"invalid checksum line {line_number}: {line!r}")
        expected[parts[1]] = parts[0]
    return expected


def generate(directory: Path) -> None:
    files = sorted(path for path in directory.iterdir() if path.is_file())
    if not files:
        raise SystemExit(f"no release assets found in {directory}")
    names = [path.name for path in files]
    if len(names) != len(set(names)):
        raise SystemExit("release asset names are not unique")
    for name in names:
        if any(character.isspace() for character in name):
            raise SystemExit(f"release asset name contains whitespace: {name!r}")
    lines = [f"{digest(path)}  {path.name}" for path in files]
    (directory / "checksums.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    sys.stdout.write(f"wrote {directory / 'checksums.txt'} with {len(lines)} assets\n")


def verify(checksums: Path, directory: Path) -> None:
    expected = parse_checksums(checksums)
    actual_names = {
        path.name
        for path in directory.iterdir()
        if path.is_file() and path.name != "checksums.txt"
    }
    expected_names = set(expected)
    if actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        raise SystemExit(f"published asset set mismatch; missing={missing}, extra={extra}")
    for name, expected_hash in sorted(expected.items()):
        actual_hash = digest(directory / name)
        if actual_hash != expected_hash:
            sys.stderr.write(
                f"checksum mismatch for {name}: expected {expected_hash}, got {actual_hash}\n"
            )
            raise SystemExit(1)
    sys.stdout.write(f"verified {len(expected)} published assets and checksums\n")


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate_parser = subparsers.add_parser("generate")
    generate_parser.add_argument("directory", type=Path)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("checksums", type=Path)
    verify_parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "generate":
        generate(args.directory)
    else:
        verify(args.checksums, args.directory)


if __name__ == "__main__":
    main()
