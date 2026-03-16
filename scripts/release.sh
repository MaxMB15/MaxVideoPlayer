#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAURI_CONF="$REPO_ROOT/apps/desktop/src-tauri/tauri.conf.json"
CARGO_TOML="$REPO_ROOT/apps/desktop/src-tauri/Cargo.toml"
SETTINGS_TSX="$REPO_ROOT/apps/desktop/src/components/settings/Settings.tsx"

# ── Read current version from tauri.conf.json ────────────────────────────────
CURRENT=$(python3 -c "import json,sys; print(json.load(open('$TAURI_CONF'))['version'])")
MAJOR=$(echo "$CURRENT" | cut -d. -f1)
MINOR=$(echo "$CURRENT" | cut -d. -f2)
PATCH=$(echo "$CURRENT" | cut -d. -f3)

echo ""
echo "  MaxVideoPlayer Release Script"
echo "  ─────────────────────────────"
echo "  Current version: $CURRENT"
echo ""
echo "  Select release type:"
echo "    1) Patch  ($MAJOR.$MINOR.$((PATCH + 1)))"
echo "    2) Minor  ($MAJOR.$((MINOR + 1)).0)"
echo "    3) Major  ($((MAJOR + 1)).0.0)"
echo ""
read -rp "  Choice [1/2/3]: " CHOICE

case "$CHOICE" in
  1) NEW="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  2) NEW="$MAJOR.$((MINOR + 1)).0" ;;
  3) NEW="$((MAJOR + 1)).0.0" ;;
  *) echo "Invalid choice. Aborting."; exit 1 ;;
esac

echo ""
echo "  Bumping $CURRENT → $NEW"
echo ""
read -rp "  Confirm? [y/N]: " CONFIRM
case "$CONFIRM" in
  [yY]) ;;
  *) echo "Aborted."; exit 0 ;;
esac

# ── Update tauri.conf.json ────────────────────────────────────────────────────
python3 - <<PYEOF
import json, re

path = "$TAURI_CONF"
with open(path) as f:
    raw = f.read()

# Preserve formatting: targeted replacement of the version field
updated = re.sub(
    r'("version"\s*:\s*)"[^"]+"',
    r'\g<1>"$NEW"',
    raw,
    count=1
)

with open(path, "w") as f:
    f.write(updated)

print("  Updated tauri.conf.json")
PYEOF

# ── Update Cargo.toml ────────────────────────────────────────────────────────
python3 - <<PYEOF
import re

path = "$CARGO_TOML"
with open(path) as f:
    raw = f.read()

# Only replace the first [package] version = "..." line
updated = re.sub(
    r'^(version\s*=\s*)"[^"]+"',
    r'\g<1>"$NEW"',
    raw,
    count=1,
    flags=re.MULTILINE
)

with open(path, "w") as f:
    f.write(updated)

print("  Updated Cargo.toml")
PYEOF

# ── Update Settings.tsx version string ───────────────────────────────────────
sed -i.bak \
  "s/MaxVideoPlayer v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*/MaxVideoPlayer v$NEW/" \
  "$SETTINGS_TSX"
rm -f "${SETTINGS_TSX}.bak"
echo "  Updated Settings.tsx"

# ── Commit ───────────────────────────────────────────────────────────────────
cd "$REPO_ROOT"
git add "$TAURI_CONF" "$CARGO_TOML" "$SETTINGS_TSX"
git commit -m "chore: bump version to $NEW"
echo "  Committed version bump"

# ── Tag ──────────────────────────────────────────────────────────────────────
git tag "v$NEW"
echo "  Created tag v$NEW"

# ── Push ─────────────────────────────────────────────────────────────────────
echo ""
read -rp "  Push commit and tag to origin now? [y/N]: " PUSH
case "$PUSH" in
  [yY])
    git push origin HEAD
    git push origin "v$NEW"
    echo ""
    echo "  Done! Release workflow triggered for v$NEW."
    echo "  Watch progress at: https://github.com/MaxMB15/MaxVideoPlayer/actions"
    echo "  Draft release will appear at: https://github.com/MaxMB15/MaxVideoPlayer/releases"
    ;;
  *)
    echo ""
    echo "  Not pushed. When ready, run:"
    echo "    git push origin HEAD && git push origin v$NEW"
    ;;
esac

echo ""
