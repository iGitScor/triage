#!/usr/bin/env python3
"""Checks that Remora uses the Estuary design system's tokens (@iscor/design, pinned in package.json).

- The website's site/public/site/tokens.css is the package's dist/tokens.css, as is (the site is static: no
  bundler to import it). `--write` copies it after an update of the package.
- The Mac's colours (macos/Sources/Remora/Presentation/Theme.swift, `Myna`) equal the package's, in light and in dark,
  opacity included. The Mac keeps its own file for what the package doesn't have yet: the Increase Contrast variants.

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


checked = 0
for name in tokens["light"]:
    arguments = color_arguments(camel(name))
    if arguments is None:
        continue  # a token the Mac doesn't use
    checked += 1
    found = dict(re.findall(r"(light|dark): 0x([0-9A-F]{6})", arguments))
    opacity = re.search(r"(?<![\w])opacity: ([\d.]+)", arguments)
    opacities = {
        "light": float((re.search(r"lightOpacity: ([\d.]+)", arguments) or opacity or [None, 1])[1]),
        "dark": float((re.search(r"darkOpacity: ([\d.]+)", arguments) or opacity or [None, 1])[1]),
    }
    for scheme in ("light", "dark"):
        hex_value, alpha = rgba(tokens[scheme][name])
        if found.get(scheme) != hex_value or round(opacities[scheme], 2) != alpha:
            problems.append(
                f"Myna.{camel(name)} ({scheme}) is 0x{found.get(scheme)} at {opacities[scheme]}, "
                f"@iscor/design says 0x{hex_value} at {alpha}"
            )
disabled = re.search(r"static let disabledOpacity = ([\d.]+)", swift)
if not disabled or float(disabled.group(1)) != tokens["base"]["disabled"]:
    problems.append(f"Myna.disabledOpacity isn't the package's {tokens['base']['disabled']}")

if problems:
    sys.exit("Remora differs from @iscor/design:\n  " + "\n  ".join(problems))
print(f"✓ the site's tokens.css and the Mac's {checked} colours match @iscor/design")
