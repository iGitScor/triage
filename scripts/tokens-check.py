#!/usr/bin/env python3
"""Fails when the docs theme's light or dark colours differ from the shared Myna tokens.

site/public/site/tokens.css is the single source for the site, the docs and the Windows app. The docs follow
VitePress's own light/dark switch, so docs/.vitepress/theme/custom.css sets each scheme's tokens again
(`html:not(.dark)` and `html.dark`): every value there must equal the token of the same scheme.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOKENS = ROOT / "site/public/site/tokens.css"
THEME = ROOT / "docs/.vitepress/theme/custom.css"


def block(css: str, selector: str) -> str:
    """The body of the first rule for `selector`, braces matched."""
    start = css.index(selector)
    depth, opened = 0, css.index("{", start)
    for index in range(opened, len(css)):
        depth += {"{": 1, "}": -1}.get(css[index], 0)
        if depth == 0:
            return css[opened + 1 : index]
    raise ValueError(f"{selector} isn't closed")


def variables(body: str) -> dict[str, str]:
    return {name: value.strip() for name, value in re.findall(r"(--[\w-]+)\s*:\s*([^;]+);", body)}


tokens = TOKENS.read_text()
light = variables(block(tokens, ":root"))
dark = {**light, **variables(block(block(tokens, "@media (prefers-color-scheme: dark)"), ":root"))}
theme = THEME.read_text()

problems = [
    f"{scheme}: {name} is {value}, tokens.css says {expected.get(name)}"
    for scheme, selector, expected in (("light", "html:not(.dark)", light), ("dark", "html.dark", dark))
    for name, value in variables(block(theme, selector)).items()
    if expected.get(name) != value
]
if problems:
    sys.exit(f"{THEME.relative_to(ROOT)} differs from {TOKENS.relative_to(ROOT)}:\n  " + "\n  ".join(problems))
print(f"✓ the docs theme matches {TOKENS.relative_to(ROOT)}")
