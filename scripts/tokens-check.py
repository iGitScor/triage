#!/usr/bin/env python3
"""Checks that Remora uses the Estuary design system's tokens (@iscor/design, pinned in package.json).

- The website's site/public/site/tokens.css is the package's dist/tokens.css, as is (the site is static: no
  bundler to import it). `--write` copies it after an update of the package.
- The Mac's colours (macos/Sources/Remora/Presentation/Theme.swift, `Myna`) equal the package's, in light and in dark,
  with and without Increase Contrast, opacity included.

The Windows app and the docs import the package's CSS themselves.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PACKAGE = ROOT / "node_modules/@iscor/design/dist"
SITE = ROOT / "site/public/site/tokens.css"
THEME = ROOT / "macos/Sources/Remora/Presentation/Theme.swift"

if not PACKAGE.exists():
    sys.exit("@iscor/design isn't installed: run `npm ci` at the repository's root.")

problems = []
package_css = (PACKAGE / "tokens.css").read_text()
if "--write" in sys.argv:
    SITE.write_text(package_css)
elif SITE.read_text() != package_css:
    problems.append(
        f"{SITE.relative_to(ROOT)} isn't the package's tokens.css: run `python3 scripts/tokens-check.py --write`"
    )

tokens = json.loads((PACKAGE / "tokens.json").read_text())


def camel(name: str) -> str:
    return re.sub(r"-(\w)", lambda m: m.group(1).upper(), name)


def rgba(value: str) -> tuple[str, float]:
    """#rrggbb or #rrggbbaa → (RRGGBB, alpha rounded to 2 places)."""
    return value[1:7].upper(), round(int(value[7:9], 16) / 255, 2) if len(value) == 9 else 1.0


swift = THEME.read_text()


def color_arguments(name: str) -> str | None:
    """What is between the parentheses of `static let <name> = Color(…)`, nested ones included."""
    start = swift.find(f"static let {name} = Color(")
    if start < 0:
        return None
    depth, opened = 0, swift.index("(", start)
    for index in range(opened, len(swift)):
        depth += {"(": 1, ")": -1}.get(swift[index], 0)
        if depth == 0:
            return swift[opened + 1 : index]
    return None


def resolved(arguments: str) -> dict[str, tuple[str, float]]:
    """The Mac's colour in each of the four appearances, from its Color(…) arguments."""
    hexes = dict(re.findall(r"\b(light|dark): 0x([0-9A-F]{6})", arguments))
    both = re.search(r"(?<![\w])opacity: ([\d.]+)", arguments)
    opacity = {
        scheme: float(m[1])
        if (m := re.search(rf"{scheme}Opacity: ([\d.]+)", arguments))
        else float(both[1])
        if both
        else 1.0
        for scheme in ("light", "dark")
    }
    contrast = re.search(r"contrast: \(0x([0-9A-F]{6}), 0x([0-9A-F]{6})\)", arguments)
    contrast_hex = {"light": contrast[1], "dark": contrast[2]} if contrast else hexes
    pair = re.search(r"contrastOpacity: \(([\d.]+), ([\d.]+)\)", arguments)
    single = re.search(r"contrastOpacity: ([\d.]+)", arguments)
    contrast_opacity = (
        {"light": float(pair[1]), "dark": float(pair[2])}
        if pair
        else {"light": float(single[1]), "dark": float(single[1])}
        if single
        else opacity
    )
    return {
        "light": (hexes["light"], round(opacity["light"], 2)),
        "dark": (hexes["dark"], round(opacity["dark"], 2)),
        "contrast light": (contrast_hex["light"], round(contrast_opacity["light"], 2)),
        "contrast dark": (contrast_hex["dark"], round(contrast_opacity["dark"], 2)),
    }


checked = 0
for name in tokens["light"]:
    arguments = color_arguments(camel(name))
    if arguments is None:
        continue  # a token the Mac doesn't use
    checked += 1
    mac = resolved(arguments)
    expected = {
        "light": rgba(tokens["light"][name]),
        "dark": rgba(tokens["dark"][name]),
        "contrast light": rgba(tokens["contrast"]["light"][name]),
        "contrast dark": rgba(tokens["contrast"]["dark"][name]),
    }
    for appearance, value in expected.items():
        if mac[appearance] != value:
            problems.append(
                f"Myna.{camel(name)} ({appearance}) is 0x{mac[appearance][0]} at {mac[appearance][1]}, "
                f"@iscor/design says 0x{value[0]} at {value[1]}"
            )
disabled = re.search(r"static let disabledOpacity = ([\d.]+)", swift)
if not disabled or float(disabled.group(1)) != tokens["base"]["disabled"]:
    problems.append(f"Myna.disabledOpacity isn't the package's {tokens['base']['disabled']}")

if problems:
    sys.exit("Remora differs from @iscor/design:\n  " + "\n  ".join(problems))
print(f"✓ the site's tokens.css and the Mac's {checked} colours match @iscor/design")
