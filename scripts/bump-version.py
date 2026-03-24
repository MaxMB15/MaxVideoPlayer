#!/usr/bin/env python3
"""
Bump the desktop app version in:
  - apps/desktop/src-tauri/tauri.conf.json
  - apps/desktop/src-tauri/Cargo.toml (first [package] version line)
  - apps/desktop/src/components/settings/Settings.tsx (About line)

Usage:
  python3 scripts/bump-version.py NEW_VERSION

Example:
  python3 scripts/bump-version.py 0.4.0
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TAURI_CONF = ROOT / "apps/desktop/src-tauri/tauri.conf.json"
CARGO_TOML = ROOT / "apps/desktop/src-tauri/Cargo.toml"
SETTINGS_TSX = ROOT / "apps/desktop/src/components/settings/Settings.tsx"

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+$")


def bump(new: str) -> None:
    if not SEMVER_RE.match(new):
        print("ERROR: version must be semver X.Y.Z (e.g. 0.4.0)", file=sys.stderr)
        sys.exit(1)

    raw = TAURI_CONF.read_text(encoding="utf-8")
    updated = re.sub(
        r'("version"\s*:\s*)"[^"]+"',
        r'\1"%s"' % new,
        raw,
        count=1,
    )
    TAURI_CONF.write_text(updated, encoding="utf-8")
    print(f"  Updated {TAURI_CONF.relative_to(ROOT)}")

    raw = CARGO_TOML.read_text(encoding="utf-8")
    updated = re.sub(
        r'^(version\s*=\s*)"[^"]+"',
        r'\1"%s"' % new,
        raw,
        count=1,
        flags=re.MULTILINE,
    )
    CARGO_TOML.write_text(updated, encoding="utf-8")
    print(f"  Updated {CARGO_TOML.relative_to(ROOT)}")

    raw = SETTINGS_TSX.read_text(encoding="utf-8")
    updated = re.sub(
        r"MaxVideoPlayer v\d+\.\d+\.\d+",
        f"MaxVideoPlayer v{new}",
        raw,
        count=1,
    )
    SETTINGS_TSX.write_text(updated, encoding="utf-8")
    print(f"  Updated {SETTINGS_TSX.relative_to(ROOT)}")


def main() -> None:
    if len(sys.argv) != 2:
        print("Usage: bump-version.py NEW_VERSION", file=sys.stderr)
        sys.exit(1)
    bump(sys.argv[1])


if __name__ == "__main__":
    main()
