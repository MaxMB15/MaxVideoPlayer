#!/usr/bin/env bash
# validate-release.sh — Verify that a release's latest.json has working download URLs.
#
# Usage:
#   ./scripts/validate-release.sh              # checks the latest release
#   ./scripts/validate-release.sh v0.3.7       # checks a specific tag
#
# Exits 0 if all platform URLs resolve (HTTP 200), non-zero otherwise.

set -euo pipefail

REPO="MaxMB15/MaxVideoPlayer"
TAG="${1:-latest}"

echo "=== Validating release: $TAG ==="

# Fetch latest.json
if [[ "$TAG" == "latest" ]]; then
  URL="https://github.com/$REPO/releases/latest/download/latest.json"
else
  URL="https://github.com/$REPO/releases/download/$TAG/latest.json"
fi

echo "Fetching $URL"
LATEST_JSON=$(curl -sL -w "\n%{http_code}" "$URL")
HTTP_CODE=$(echo "$LATEST_JSON" | tail -1)
BODY=$(echo "$LATEST_JSON" | sed '$d')

if [[ "$HTTP_CODE" != "200" ]]; then
  echo "FAIL: latest.json returned HTTP $HTTP_CODE"
  exit 1
fi

echo "latest.json fetched successfully"

# Parse version
VERSION=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['version'])")
echo "Version: $VERSION"

# Extract all platform URLs and signatures
PLATFORMS=$(echo "$BODY" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for platform, info in data.get('platforms', {}).items():
    print(f\"{platform}|{info['url']}|{info.get('signature', '')}\")
")

if [[ -z "$PLATFORMS" ]]; then
  echo "FAIL: No platforms found in latest.json"
  exit 1
fi

FAILED=0

while IFS='|' read -r platform url signature; do
  echo ""
  echo "--- Platform: $platform ---"
  echo "  URL: $url"

  # Check the download URL resolves (HEAD request, follow redirects)
  STATUS=$(curl -sI -o /dev/null -w "%{http_code}" -L "$url")
  if [[ "$STATUS" == "200" ]]; then
    echo "  Download: OK (HTTP $STATUS)"
  else
    echo "  Download: FAIL (HTTP $STATUS)"
    FAILED=1
  fi

  # Check the .sig URL resolves too
  SIG_URL="${url}.sig"
  SIG_STATUS=$(curl -sI -o /dev/null -w "%{http_code}" -L "$SIG_URL")
  if [[ "$SIG_STATUS" == "200" ]]; then
    echo "  Signature: OK (HTTP $SIG_STATUS)"
  else
    echo "  Signature: FAIL (HTTP $SIG_STATUS) — $SIG_URL"
    FAILED=1
  fi

  # Verify the signature in latest.json is non-empty
  if [[ -z "$signature" ]]; then
    echo "  Signature field: FAIL (empty in latest.json)"
    FAILED=1
  else
    echo "  Signature field: OK (present)"
  fi

done <<< "$PLATFORMS"

echo ""
if [[ "$FAILED" -eq 0 ]]; then
  echo "=== ALL CHECKS PASSED ==="
  exit 0
else
  echo "=== SOME CHECKS FAILED ==="
  exit 1
fi
