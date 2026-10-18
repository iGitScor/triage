#!/usr/bin/env python3
"""Lists the tests `swift test` skipped, with why, so a green run doesn't hide what it didn't check.

Reads the test output (a file, or stdin) and prints a Markdown table, written to the job summary on GitHub
($GITHUB_STEP_SUMMARY). A test skipped for want of Apple's on-device assets also gets a notice on the run: those are
the ones CI can't run, but `make check` does on a Mac that has them.

    swift test 2>&1 | tee test.log; python3 scripts/skipped-tests.py test.log
"""

import os
import pathlib
import re
import sys

text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8") if len(sys.argv) > 1 else sys.stdin.read()
skipped = sorted(set(re.findall(r'➜ Test (\S+?)\(\S*\) skipped(?:: "(.*?)")?\n', text)))

lines = ["### Skipped tests", ""]
if skipped:
    lines += ["| Test | Why |", "|---|---|"] + [f"| `{name}` | {why or 'no reason given'} |" for name, why in skipped]
else:
    lines.append("None: every test ran.")
summary = "\n".join(lines) + "\n"

if path := os.environ.get("GITHUB_STEP_SUMMARY"):
    with open(path, "a", encoding="utf-8") as out:
        out.write(summary)
else:
    print(summary)
for name, why in skipped:
    if "Apple" in why:
        print(f"::notice title=Skipped on CI::{name}: {why}. It runs with `make check` on a Mac that has them.")
