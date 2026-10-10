#!/usr/bin/env python3
"""Remora's version, kept the same in every place it is written: the macOS Info.plist, the Rust workspace (and the
crates' dependencies on each other, and Cargo.lock), and the Windows interface's package files.

    python3 scripts/version.py              # show it; fails if the places disagree
    python3 scripts/version.py 0.3.2        # set it everywhere, then commit and tag v0.3.2
    python3 scripts/version.py --check 0.3.2  # fails unless every place says 0.3.2 (the release workflow)

The build number (CFBundleVersion) is not here: the release workflow stamps it from the run number.
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
PLIST = ROOT / "macos/Resources/Info.plist"
WORKSPACE = ROOT / "windows/Cargo.toml"
CRATES = sorted((ROOT / "windows/crates").glob("*/Cargo.toml"))
LOCK = ROOT / "windows/Cargo.lock"
PACKAGE = ROOT / "windows/app/package.json"
PACKAGE_LOCK = ROOT / "windows/app/package-lock.json"
WORKSPACE_PACKAGES = ("remora", "remora_core", "remora_plugins", "remora_app")

PLIST_RE = re.compile(r"(<key>CFBundleShortVersionString</key>\s*<string>)([^<]*)(</string>)")
WORKSPACE_RE = re.compile(r'(\[workspace\.package\]\s*\nversion = ")([^"]*)(")')
DEPENDENCY_RE = re.compile(r'(remora_\w+ = \{ version = ")([^"]*)(", path)')
LOCK_RE = re.compile(r'(\[\[package\]\]\nname = "(?:%s)"\nversion = ")([^"]*)(")' % "|".join(WORKSPACE_PACKAGES))


def read() -> dict[str, list[str]]:
    """Every place and the version it holds."""
    found = {
        "macos/Resources/Info.plist": [m.group(2) for m in PLIST_RE.finditer(PLIST.read_text())],
        "windows/Cargo.toml": [m.group(2) for m in WORKSPACE_RE.finditer(WORKSPACE.read_text())],
        "windows/Cargo.lock": [m.group(2) for m in LOCK_RE.finditer(LOCK.read_text())],
    }
    for crate in CRATES:
        versions = [m.group(2) for m in DEPENDENCY_RE.finditer(crate.read_text())]
        if versions:
            found[str(crate.relative_to(ROOT))] = versions
    package_lock = json.loads(PACKAGE_LOCK.read_text())
    found["windows/app/package.json"] = [json.loads(PACKAGE.read_text())["version"]]
    found["windows/app/package-lock.json"] = [package_lock["version"], package_lock["packages"][""]["version"]]
    return found


def write(version: str) -> None:
    def replace(path: pathlib.Path, pattern: re.Pattern, expected: int) -> None:
        text, count = pattern.subn(lambda m: m.group(1) + version + m.group(3), path.read_text())
        if count < expected:
            sys.exit(f"{path.relative_to(ROOT)}: expected {expected} version field(s), found {count}")
        path.write_text(text)

    replace(PLIST, PLIST_RE, 1)
    replace(WORKSPACE, WORKSPACE_RE, 1)
    replace(LOCK, LOCK_RE, len(WORKSPACE_PACKAGES))
    for crate in CRATES:
        if DEPENDENCY_RE.search(crate.read_text()):
            replace(crate, DEPENDENCY_RE, 1)
    package = json.loads(PACKAGE.read_text())
    package["version"] = version
    PACKAGE.write_text(json.dumps(package, indent=2, ensure_ascii=False) + "\n")
    package_lock = json.loads(PACKAGE_LOCK.read_text())
    package_lock["version"] = version
    package_lock["packages"][""]["version"] = version
    PACKAGE_LOCK.write_text(json.dumps(package_lock, indent=2, ensure_ascii=False) + "\n")


def report(found: dict[str, list[str]]) -> set[str]:
    for place, versions in found.items():
        print(f"  {', '.join(sorted(set(versions))):<10} {place}")
    return {v for versions in found.values() for v in versions}


def main() -> None:
    args = sys.argv[1:]
    if args[:1] == ["--check"] and len(args) == 2:
        expected = args[1].removeprefix("v")
        versions = report(read())
        if versions != {expected}:
            sys.exit(f"The tag says {expected}, the source says {', '.join(sorted(versions))}. "
                     f"Run `make version V={expected}`, commit, then tag again.")
        print(f"Every place says {expected}.")
    elif len(args) == 1 and SEMVER.match(args[0].removeprefix("v")):
        version = args[0].removeprefix("v")
        write(version)
        report(read())
        print(f"Set to {version}. Commit it, then tag v{version}.")
    elif not args:
        versions = report(read())
        if len(versions) != 1:
            sys.exit("The places disagree: set one version with `make version V=x.y.z`.")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
