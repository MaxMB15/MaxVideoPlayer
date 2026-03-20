#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAURI_CONF="$REPO_ROOT/apps/desktop/src-tauri/tauri.conf.json"
CARGO_TOML="$REPO_ROOT/apps/desktop/src-tauri/Cargo.toml"
SETTINGS_TSX="$REPO_ROOT/apps/desktop/src/components/settings/Settings.tsx"

# ── Git-flow branch guard ─────────────────────────────────────────────────────
cd "$REPO_ROOT"
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$CURRENT_BRANCH" != "main" ]]; then
  echo ""
  echo "  ERROR: release.sh must be run from the 'main' branch."
  echo "  You are on: $CURRENT_BRANCH"
  echo ""
  echo "  Git-flow release process:"
  echo "    1. Merge dev → main via PR"
  echo "    2. git checkout main && git pull origin main"
  echo "    3. Run this script"
  echo ""
  exit 1
fi

# Ensure main is up-to-date with origin
echo ""
echo "  Fetching origin/main…"
git fetch origin main --quiet
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse origin/main)
if [[ "$LOCAL" != "$REMOTE" ]]; then
  echo ""
  echo "  ERROR: local main is not up-to-date with origin/main."
  echo "  Run: git pull origin main"
  echo ""
  exit 1
fi

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
git add "$TAURI_CONF" "$CARGO_TOML" "$SETTINGS_TSX" Cargo.lock
git commit -m "chore: bump version to $NEW"
echo "  Committed version bump"

# ── Tag ──────────────────────────────────────────────────────────────────────
git tag "v$NEW"
echo "  Created tag v$NEW"

# ── Push main + tag (triggers release workflow) ───────────────────────────────
echo ""
read -rp "  Push main and tag to origin now? [y/N]: " PUSH
case "$PUSH" in
  [yY])
    git push origin main
    git push origin "v$NEW"
    echo ""
    echo "  ✓ Release workflow triggered for v$NEW."
    echo "  Watch progress at: https://github.com/MaxMB15/MaxVideoPlayer/actions"
    echo "  Draft release will appear at: https://github.com/MaxMB15/MaxVideoPlayer/releases"
    ;;
  *)
    echo ""
    echo "  Not pushed. When ready, run:"
    echo "    git push origin main && git push origin v$NEW"
    echo ""
    exit 0
    ;;
esac

# ── Merge version bump back to dev (git-flow requirement) ────────────────────
echo ""
echo "  Git-flow requires the version bump to be reflected in dev."
read -rp "  Merge main → dev now? [y/N]: " MERGE_DEV
case "$MERGE_DEV" in
  [yY])
    git checkout dev
    git pull origin dev --quiet
    git merge --no-ff main -m "chore: merge main into dev after v$NEW release"
    git push origin dev
    git checkout main
    echo ""
    echo "  ✓ Version bump merged back to dev."
    ;;
  *)
    echo ""
    echo "  Skipped. Remember to merge main → dev manually:"
    echo "    git checkout dev && git pull origin dev"
    echo "    git merge --no-ff main -m 'chore: merge main into dev after v$NEW release'"
    echo "    git push origin dev"
    ;;
esac

echo ""
