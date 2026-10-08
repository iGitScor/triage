#!/usr/bin/env python3
"""Writes src/i18n/fr.json from macos/scripts/translations_fr.py (one dictionary for both apps), and lists the
strings the Windows app shows that have no French yet: t("…") calls in src/, and the English the Rust crates
produce for the interface (bundle titles, badges, presets, manifests, notices, refusals)."""
import json
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
repo = root.parent.parent
sys.path.insert(0, str(repo / "macos" / "scripts"))
from translations_fr import FR  # noqa: E402

out = root / "src" / "i18n" / "fr.json"
if "--check" in sys.argv:
    # CI: the committed file must match the dictionary (compared as data, so line endings don't matter).
    current = json.loads(out.read_text(encoding="utf-8")) if out.exists() else {}
    if current != FR:
        sys.exit(f"✗ {out.relative_to(repo)} is out of date: run npm run i18n")
    print(f"✓ {out.relative_to(repo)} matches macos/scripts/translations_fr.py")
    sys.exit(0)
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(dict(sorted(FR.items())), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

ui = re.compile(r"""\bt\(\s*(['"])((?:\\.|(?!\1).)+)\1""")
rust = [
    re.compile(r'Badge::new\("[^"]*",\s*"([^"]+)"'),
    re.compile(r'\.notifying\("([^"]+)"\)'),
    re.compile(r'Self::of\("[^"]*",\s*"([^"]+)"'),
    re.compile(r'label:\s*"([^"]+)"'),
    re.compile(r'Some\("((?:Not allowed|External AI)[^"]+)"\)'),
    re.compile(r'(?:summary|setup_label|description):\s*"([^"]+)"\.into\(\)'),
    re.compile(r'ConfigField::(?:token|new)\(\s*"([^"]+)",\s*"([^"]+)"'),
    re.compile(r'title: if reminder \{ "([^"]+)" \} else \{ "([^"]+)" \}'),
    re.compile(r'\bt\("([^"]+)"\)'),
]
steps = re.compile(r'setup_steps:\s*vec!\[(.*?)\]', re.S)

used = set()
for file in (root / "src").rglob("*"):
    if file.suffix in {".svelte", ".ts"}:
        used |= {m.group(2) for m in ui.finditer(file.read_text(encoding="utf-8"))}
for file in list((repo / "windows" / "crates").rglob("*.rs")) + list((root / "src-tauri" / "src").rglob("*.rs")):
    text = file.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    for pattern in rust:
        for match in pattern.finditer(text):
            used |= {g for g in match.groups() if g}
    for block in steps.finditer(text):
        used |= set(re.findall(r'"((?:[^"\\]|\\.)+)"\.into\(\)', block.group(1)))

missing = sorted(k for k in used if k not in FR and not re.fullmatch(r"[\d\s%+−·#@.:/-]*", k))
print(f"✓ {out.relative_to(repo)}: {len(FR)} strings")
if missing:
    print(f"{len(missing)} without French (add them to macos/scripts/translations_fr.py):")
    for key in missing:
        print(f"    {json.dumps(key, ensure_ascii=False)}: \"\",")
