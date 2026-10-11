#!/usr/bin/env python3
"""Keeps the test counts in the READMEs and the Windows docs equal to the tests in the code.

Counts test declarations (`@Test`, `#[test]`, vitest `test(`/`it(`), which is what the runners report: no test is
parameterized. `--check` fails instead of writing (CI); otherwise it prints the files it changed (the pre-commit hook
stages them).
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def count(folder: str, glob: str, pattern: str) -> int:
    regex = re.compile(pattern, re.MULTILINE)
    return sum(len(regex.findall(path.read_text())) for path in (ROOT / folder).rglob(glob))


RUST = r"^\s*#\[(?:tokio::)?test\]"
counts = {
    "swift": count("macos/Tests", "*.swift", r"^\s*@Test\b"),
    "crates/remora_core": count("windows/crates/remora_core", "*.rs", RUST),
    "crates/remora_plugins": count("windows/crates/remora_plugins", "*.rs", RUST),
    "crates/remora_app": count("windows/crates/remora_app", "*.rs", RUST),
    "app/src-tauri": count("windows/app/src-tauri/src", "*.rs", RUST),
    "app/src": count("windows/app/src", "*.test.ts", r"^\s*(?:test|it)\("),
}
cargo = sum(n for key, n in counts.items() if key.startswith(("crates/", "app/src-tauri")))


def last_cell(text: str, part: str, value: str) -> str:
    """The Tests column of a `| `part` | … |` row."""
    return re.sub(rf"^(\| `{re.escape(part)}` \|.*\|) *[^|]*?\|$", rf"\g<1> {value} |", text, flags=re.MULTILINE)


def parts_table(text: str, typed: str) -> str:
    for part in ("crates/remora_core", "crates/remora_plugins", "crates/remora_app", "app/src-tauri"):
        text = last_cell(text, part, str(counts[part]))
    return last_cell(text, "app/src", f"{typed}, {counts['app/src']}")


def readme(text: str) -> str:
    text = re.sub(r"^(\| macOS \(Swift Testing\) \| )\d+", rf"\g<1>{counts['swift']}", text, flags=re.MULTILINE)
    return re.sub(r"^(\| Windows \(`cargo test`\) \| )\d+", rf"\g<1>{cargo}", text, flags=re.MULTILINE)


FILES = {
    "README.md": readme,
    "windows/README.md": lambda text: parts_table(text, "type-checked"),
    "docs/develop/windows.md": lambda text: parts_table(text, "type-checked"),
    "docs/fr/developper/windows.md": lambda text: parts_table(text, "types vérifiés"),
}

stale = []
for name, update in FILES.items():
    path = ROOT / name
    before = path.read_text()
    after = update(before)
    if after != before:
        stale.append(name)
        if "--check" not in sys.argv:
            path.write_text(after)

if "--check" in sys.argv and stale:
    sys.exit(f"Stale test counts in {', '.join(stale)}: run `make test-counts`.")
print("\n".join(stale))
