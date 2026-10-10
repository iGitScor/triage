#!/usr/bin/env python3
"""Writes the docs' Content-Security-Policy in public/_headers with the hash of each inline script VitePress emits,
instead of 'unsafe-inline'. One of them lists the pages' bundle hashes, so it changes with the docs: run after
every docs build (make docs-build and deploy.sh do). `--check` (CI) fails when _headers is out of date."""
import base64, hashlib, pathlib, re, sys

site = pathlib.Path(__file__).resolve().parent
docs, headers = site / "public/docs", site / "public/_headers"

hashes = set()
for page in sorted(docs.rglob("*.html")):
    for attrs, body in re.findall(r"<script(?![^>]*\bsrc=)([^>]*)>(.*?)</script>", page.read_text(encoding="utf-8"), re.S):
        if "json" in attrs:  # data blocks, which CSP doesn't run
            continue
        hashes.add("'sha256-" + base64.b64encode(hashlib.sha256(body.encode()).digest()).decode() + "'")
if not hashes:
    sys.exit("No inline script found in public/docs: build the docs first (make docs-build).")

policy = (f"default-src 'self'; script-src 'self' {' '.join(sorted(hashes))}; style-src 'self' 'unsafe-inline'; "
          "img-src 'self' data:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; object-src 'none'; "
          "base-uri 'self'; form-action 'none'")
text = headers.read_text(encoding="utf-8")
updated = re.sub(r"(/docs/\*\n  Content-Security-Policy: ).*", lambda m: m.group(1) + policy, text)
if "--check" in sys.argv:
    if updated != text:
        sys.exit("✗ public/_headers: the docs' script hashes are out of date: run python3 site/csp.py")
    print(f"✓ public/_headers matches the docs' {len(hashes)} inline scripts")
else:
    headers.write_text(updated, encoding="utf-8")
    print(f"✓ public/_headers: {len(hashes)} inline script hashes for the docs")
