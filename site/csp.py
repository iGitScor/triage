#!/usr/bin/env python3
"""Writes the docs' Content-Security-Policy in public/_headers with the hash of each inline script VitePress emits,
instead of 'unsafe-inline'. With `metaChunk` (docs/.vitepress/config.ts) the pages' bundle hashes live in a JS file,
so only scripts that never change are inline and the hashes stay the same from one docs edit to the next. Run after
a docs build (make docs-build and deploy.sh do); `--check` (the Website workflow) fails when an inline script was
added or changed, e.g. by a VitePress update: then run this and commit public/_headers."""

import base64
import hashlib
import pathlib
import re
import sys

site = pathlib.Path(__file__).resolve().parent
docs, headers = site / "public/docs", site / "public/_headers"

hashes = set()
for page in sorted(docs.rglob("*.html")):
    for attrs, body in re.findall(
        r"<script(?![^>]*\bsrc=)([^>]*)>(.*?)</script>", page.read_text(encoding="utf-8"), re.DOTALL
    ):
        if "json" in attrs:  # data blocks, which CSP doesn't run
            continue
        hashes.add("'sha256-" + base64.b64encode(hashlib.sha256(body.encode()).digest()).decode() + "'")
if not hashes:
    sys.exit("No inline script found in public/docs: build the docs first (make docs-build).")

policy = (
    f"default-src 'self'; script-src 'self' {' '.join(sorted(hashes))}; style-src 'self' 'unsafe-inline'; "
    "img-src 'self' data:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; object-src 'none'; "
    "base-uri 'self'; form-action 'none'"
)
text = headers.read_text(encoding="utf-8")
updated = re.sub(r"(/docs/\*\n  Content-Security-Policy: ).*", lambda m: m.group(1) + policy, text)
if "--check" in sys.argv:
    if updated != text:
        sys.exit("✗ public/_headers: the docs' script hashes are out of date: run python3 site/csp.py")
    print(f"✓ public/_headers matches the docs' {len(hashes)} inline scripts")
else:
    headers.write_text(updated, encoding="utf-8")
    print(f"✓ public/_headers: {len(hashes)} inline script hashes for the docs")
