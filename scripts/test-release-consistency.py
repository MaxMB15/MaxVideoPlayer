#!/usr/bin/env python3
"""
test-release-consistency.py — Static analysis of release.yml to verify that
the asset filenames referenced in latest.json match the filenames actually
uploaded to the GitHub release.

This catches the class of bug where latest.json references a filename like
MaxVideoPlayer_0.3.7_aarch64.app.tar.gz but the upload step pushes
MaxVideoPlayer.app.tar.gz (using gh's # label syntax which only changes
the display name, not the download URL).

Run: python3 scripts/test-release-consistency.py
"""

import re
import sys
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml"
PLACEHOLDER_VERSION = "0.0.0"


def extract_latest_json_urls(content: str) -> dict[str, str]:
    """Extract URL patterns from the Python block that generates latest.json."""
    urls: dict[str, str] = {}

    # Find all f-string URL patterns in the latest.json generation block
    # e.g. f"https://github.com/.../MaxVideoPlayer_{version}_aarch64.app.tar.gz"
    for match in re.finditer(
        r'platforms\["([^"]+)"\]\s*=\s*\{[^}]*"url":\s*f"([^"]+)"',
        content,
        re.DOTALL,
    ):
        platform = match.group(1)
        url_template = match.group(2)
        # Replace {version} with placeholder to get the filename pattern
        url = url_template.replace("{version}", PLACEHOLDER_VERSION)
        # Extract just the filename from the URL
        filename = url.rsplit("/", 1)[-1]
        urls[platform] = filename

    return urls


def extract_uploaded_filenames(content: str) -> list[str]:
    """Extract the actual filenames uploaded via gh release upload.

    Handles both direct paths and renamed files (cp then upload).
    Does NOT count gh's '#label' syntax as a rename since that only
    changes the display name, not the download URL.
    """
    filenames: list[str] = []

    # Find all gh release upload commands
    for block_match in re.finditer(
        r"gh release upload[^\n]*\n((?:[ \t]+.*\n)*)", content
    ):
        upload_block = block_match.group(0)
        # Each line in the upload block that contains a file path
        for line in upload_block.split("\n"):
            line = line.strip().rstrip("\\").strip()
            if not line or line.startswith("--") or line.startswith("gh "):
                continue
            # Strip gh's display label syntax: "file#label" → "file"
            # The actual uploaded filename is the base of the path before #
            path = line.split("#")[0].strip().strip('"')
            if path:
                # Resolve variable references to get the pattern
                filename = path.rsplit("/", 1)[-1]
                # Replace version variable patterns with placeholder
                filename = re.sub(
                    r"\$\{?VERSION\}?", PLACEHOLDER_VERSION, filename
                )
                filenames.append(filename)

    return filenames


def test_macos_updater_filename_matches():
    """The macOS updater filename in latest.json must match what's actually uploaded."""
    content = WORKFLOW.read_text()

    latest_urls = extract_latest_json_urls(content)
    uploaded = extract_uploaded_filenames(content)

    assert "darwin-aarch64" in latest_urls, (
        "latest.json does not contain a darwin-aarch64 platform entry"
    )

    expected_macos = latest_urls["darwin-aarch64"]
    assert expected_macos in uploaded, (
        f"latest.json references '{expected_macos}' but the upload step "
        f"only uploads: {uploaded}\n"
        f"The updater will 404 because the filename doesn't match.\n"
        f"Fix: ensure the upload step copies/renames the file to '{expected_macos}' "
        f"before uploading."
    )
    print(f"  PASS: macOS updater '{expected_macos}' found in uploads")


def test_macos_updater_sig_matches():
    """The macOS .sig file must also be uploaded with the matching name."""
    content = WORKFLOW.read_text()

    latest_urls = extract_latest_json_urls(content)
    uploaded = extract_uploaded_filenames(content)

    expected_sig = latest_urls["darwin-aarch64"] + ".sig"
    assert expected_sig in uploaded, (
        f"latest.json implies sig file '{expected_sig}' but the upload step "
        f"only uploads: {uploaded}\n"
        f"Fix: upload the .sig file with matching name."
    )
    print(f"  PASS: macOS signature '{expected_sig}' found in uploads")


def test_linux_updater_filename_matches():
    """The Linux updater filename in latest.json must match what's actually uploaded."""
    content = WORKFLOW.read_text()

    latest_urls = extract_latest_json_urls(content)
    uploaded = extract_uploaded_filenames(content)

    assert "linux-x86_64" in latest_urls, (
        "latest.json does not contain a linux-x86_64 platform entry"
    )

    expected_linux = latest_urls["linux-x86_64"]
    assert expected_linux in uploaded, (
        f"latest.json references '{expected_linux}' but the upload step "
        f"only uploads: {uploaded}\n"
        f"The updater will 404 because the filename doesn't match."
    )
    print(f"  PASS: Linux updater '{expected_linux}' found in uploads")


def test_all_platforms_have_signatures():
    """Every platform in latest.json must have a non-empty signature field."""
    content = WORKFLOW.read_text()

    # Check that every platform block includes "signature": <var> (not empty string)
    for match in re.finditer(
        r'platforms\["([^"]+)"\]\s*=\s*\{[^}]*"signature":\s*(\w+)',
        content,
        re.DOTALL,
    ):
        platform = match.group(1)
        sig_var = match.group(2)
        # The variable should be guarded by an `if <var>:` check
        guard_pattern = rf"if {sig_var}:"
        assert re.search(guard_pattern, content), (
            f"Platform '{platform}' uses signature variable '{sig_var}' "
            f"but there's no `if {sig_var}:` guard — empty signatures "
            f"could be published."
        )
        print(f"  PASS: {platform} signature variable '{sig_var}' is guarded")


def main():
    if not WORKFLOW.exists():
        print(f"SKIP: {WORKFLOW} not found")
        sys.exit(0)

    print(f"Checking {WORKFLOW}\n")

    tests = [
        test_macos_updater_filename_matches,
        test_macos_updater_sig_matches,
        test_linux_updater_filename_matches,
        test_all_platforms_have_signatures,
    ]

    failed = 0
    for test in tests:
        name = test.__name__
        try:
            test()
        except AssertionError as e:
            print(f"  FAIL: {name}")
            print(f"    {e}")
            failed += 1
        except Exception as e:
            print(f"  ERROR: {name} — {e}")
            failed += 1

    print()
    if failed:
        print(f"{failed} test(s) FAILED")
        sys.exit(1)
    else:
        print("All release consistency checks passed")
        sys.exit(0)


if __name__ == "__main__":
    main()
