#!/usr/bin/env python3
"""Writes THIRD-PARTY-NOTICES.txt for the Windows and Linux app (DEP-07): what Remora bundles (the Outfit font, the
Simple Icons logos), the npm packages the interface ships, and every Rust crate's licence through `cargo about`
(desktop/about.toml, desktop/about.hbs). CI and the release run it before building; a local build without it ships
a placeholder (build.rs). Needs cargo-about on PATH, or its path in CARGO_ABOUT, and `npm ci` done in desktop/app."""

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "desktop" / "app"
OUT = APP / "src-tauri" / "notices" / "THIRD-PARTY-NOTICES.txt"
# What the built interface contains: its dependencies, and the parts of Svelte that run in it.
NPM = ["@tauri-apps/api", "@fontsource-variable/outfit", "svelte", "clsx", "esm-env"]
LICENCE_FILES = ["LICENSE", "LICENSE.md", "LICENSE.txt", "LICENCE", "license", "LICENSE-MIT", "OFL.txt"]
RULE = "=" * 78


def npm_notices() -> str:
    parts = []
    for name in NPM:
        folder = APP / "node_modules" / name
        meta = json.loads((folder / "package.json").read_text(encoding="utf-8"))
        text = next(
            ((folder / f).read_text(encoding="utf-8") for f in LICENCE_FILES if (folder / f).is_file()),
            f"Licence: {meta.get('license', 'see the package')}",
        )
        parts.append(f"{RULE}\n{name} {meta['version']} ({meta.get('license', '?')})\n{'-' * 78}\n{text.strip()}\n")
    return "\n".join(parts)


def rust_notices() -> str:
    about = os.environ.get("CARGO_ABOUT", "cargo-about")
    result = subprocess.run(
        [about, "generate", "--config", "about.toml", "about.hbs"],
        cwd=ROOT / "desktop",
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        sys.exit(f"cargo about failed:\n{result.stderr}")
    return result.stdout


def main() -> None:
    bundled = (ROOT / "macos" / "Resources" / "LICENCES.txt").read_text(encoding="utf-8").strip()
    bundled = bundled.replace("(Outfit-OFL.txt)", "(below)").replace("(see LICENSE.txt)", "(see LICENSE.txt)")
    outfit = (ROOT / "macos" / "Resources" / "Outfit-OFL.txt").read_text(encoding="utf-8").strip()
    text = "\n\n".join(
        [
            bundled,
            f"{RULE}\nOutfit: SIL Open Font License 1.1\n{'-' * 78}\n{outfit}",
            "npm packages in the interface, with their licences.\n\n" + npm_notices(),
            rust_notices().strip(),
        ]
    )
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text + "\n", encoding="utf-8")
    print(f"{OUT.relative_to(ROOT)}: {len(text.splitlines())} lines")


if __name__ == "__main__":
    main()
