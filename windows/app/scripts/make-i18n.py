#!/usr/bin/env python3
"""Writes src/i18n/fr.json from macos/scripts/translations_fr.py (one dictionary for both apps), and lists the
strings the Windows app shows that have no French yet: t("…") calls in src/, and the English the Rust crates
produce for the interface (bundle titles, badges, presets, manifests, notices, refusals, error messages, with
`{name}` read as %@ as the interface's t() does). `--check` (CI) fails when the file is out of date or a string has
no French."""
import json
import pathlib
import re
import sys

# The Windows console's code page can't print ✓ or French accents.
sys.stdout.reconfigure(encoding="utf-8")
root = pathlib.Path(__file__).resolve().parent.parent
repo = root.parent.parent
sys.path.insert(0, str(repo / "macos" / "scripts"))
from translations_fr import FR  # noqa: E402

out = root / "src" / "i18n" / "fr.json"
check = "--check" in sys.argv
if check:
    # CI: the committed file must match the dictionary (compared as data, so line endings don't matter).
    current = json.loads(out.read_text(encoding="utf-8")) if out.exists() else {}
    if current != FR:
        sys.exit(f"✗ {out.relative_to(repo)} is out of date: run npm run i18n")
    print(f"✓ {out.relative_to(repo)} matches macos/scripts/translations_fr.py")
else:
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(dict(sorted(FR.items())), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

ui = re.compile(r"""\bt\(\s*(['"])((?:\\.|(?!\1).)+)\1""")
# t(done ? 'Move to inbox' : 'Done'): every literal in a one-line call; and objects of messages marked `// t-keys`.
ui_call = re.compile(r"\bt\(([^()\n]*)\)")
ui_literal = re.compile(r"""(['"])((?:\\.|(?!\1).)+)\1""")
ui_keys = re.compile(r"// t-keys\n(.*?)\n\s*\}", re.S)
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
    # Error messages: Display impls, Err("…") and ok_or("…"), and the texts they pick from.
    re.compile(r'write!\(f, "([^"]+)"'),
    re.compile(r'Err\("([^"]+)"'),
    re.compile(r'ok_or(?:_else)?\((?:\|\| )?"([^"]+)"'),
    re.compile(r'=> "([A-Z][^"]+[.:!?])",'),
    # Messages kept in constants, an item's context, and fallbacks.
    re.compile(r'const [A-Z_]+: &str = "((?:[A-Z]|%@)[^"]*[.!?])";'),
    re.compile(r'context: format!\("([^"]+)"'),
    re.compile(r'unwrap_or_else\(\|\| "([^"]+)"\.into\(\)\)'),
]
steps = re.compile(r'setup_steps:\s*vec!\[(.*?)\]', re.S)

used = set()
for file in (root / "src").rglob("*"):
    if file.suffix in {".svelte", ".ts"} and not file.name.endswith(".test.ts"):
        text = file.read_text(encoding="utf-8")
        used |= {m.group(2) for m in ui.finditer(text)}
        for call in ui_call.finditer(text):
            used |= {m.group(2) for m in ui_literal.finditer(call.group(1))}
        for block in ui_keys.finditer(text):
            used |= set(re.findall(r":\s*'([^']+)'", block.group(1)))
for file in list((repo / "windows" / "crates").rglob("*.rs")) + list((root / "src-tauri" / "src").rglob("*.rs")):
    if file.name == "fixtures.rs":  # test data only
        continue
    text = file.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    for pattern in rust:
        for match in pattern.finditer(text):
            used |= {g for g in match.groups() if g}
    for block in steps.finditer(text):
        used |= set(re.findall(r'"((?:[^"\\]|\\.)+)"\.into\(\)', block.group(1)))

# Rust's `{name}` is what t() reads as %@.
used = {re.sub(r"\{[a-z_.]*\}", "%@", key) for key in used}
missing = sorted(k for k in used if k not in FR and not re.fullmatch(r"[\d\s%+−·#@.:/()!?-]*", k))
if not check:
    print(f"✓ {out.relative_to(repo)}: {len(FR)} strings")
if missing:
    print(f"{len(missing)} without French (add them to macos/scripts/translations_fr.py):")
    for key in missing:
        print(f"    {json.dumps(key, ensure_ascii=False)}: \"\",")
    if check:
        sys.exit(1)
elif check:
    print("✓ every string the Windows app shows has French")
