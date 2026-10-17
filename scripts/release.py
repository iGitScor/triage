#!/usr/bin/env python3
"""Releases Remora in one commit: `make release V=x.y.z`.

Checks first, so a red check stops before any tag exists: on `main`, nothing uncommitted, level with `origin/main`,
every CI run on this commit green, no known vulnerability or licence problem (`make deps-check`), and changes listed
under "## Unreleased" in CHANGELOG.md. Then, in a single commit "chore: version x.y.z": the version everywhere
(scripts/version.py), the changelog's heading dated, and the website's pages built again (they show the version).
Then the signed tag vx.y.z, and both pushed together. The release workflow builds, publishes, then deploys the website.

    python3 scripts/release.py 0.11.0            # release
    python3 scripts/release.py 0.11.0 --dry-run  # the checks only
"""

import datetime
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"


def run(*command: str, capture: bool = True) -> str:
    done = subprocess.run(command, cwd=ROOT, capture_output=capture, text=True, check=False)
    if done.returncode != 0:
        sys.exit(f"`{' '.join(command)}` failed:\n{(done.stderr or done.stdout or '').strip()}")
    return (done.stdout or "").strip()


def refuse(reason: str) -> None:
    sys.exit(f"Not released: {reason}")


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    dry = "--dry-run" in sys.argv
    if len(args) != 1 or not re.fullmatch(r"\d+\.\d+\.\d+", args[0]):
        sys.exit(__doc__)
    version, tag = args[0], f"v{args[0]}"

    if run("git", "branch", "--show-current") != "main":
        refuse("release from main.")
    if run("git", "status", "--porcelain", "--untracked-files=no"):
        refuse("commit or stash the changes first.")
    run("git", "fetch", "--quiet", "--tags", "origin")
    head, remote = run("git", "rev-parse", "HEAD"), run("git", "rev-parse", "origin/main")
    if head != remote:
        refuse("main and origin/main differ: push or pull first, then wait for CI.")
    if run("git", "tag", "--list", tag):
        refuse(f"the tag {tag} already exists.")

    runs = json.loads(run("gh", "run", "list", "--commit", head, "--json", "workflowName,status,conclusion"))
    if not runs:
        refuse("no CI run on this commit yet.")
    waiting = [r["workflowName"] for r in runs if r["status"] != "completed"]
    if waiting:
        refuse(f"CI is still running ({', '.join(waiting)}); try again when it's done.")
    failed = [r["workflowName"] for r in runs if r["conclusion"] not in ("success", "skipped")]
    if failed:
        refuse(f"CI failed on this commit ({', '.join(failed)}).")
    print("✓ main, clean, pushed, CI green")

    run("make", "deps-check", capture=False)
    print("✓ no known vulnerability or licence problem")

    changelog = CHANGELOG.read_text()
    unreleased = re.search(r"^## Unreleased\n(.*?)(?=^## )", changelog, re.DOTALL | re.MULTILINE)
    if not unreleased or not unreleased.group(1).strip():
        refuse('nothing under "## Unreleased" in CHANGELOG.md.')
    print("✓ changelog ready")
    if dry:
        print(f"Dry run: {tag} would be released.")
        return

    today = datetime.date.today().isoformat()
    CHANGELOG.write_text(changelog.replace("## Unreleased\n", f"## {version} ({today})\n", 1))
    run("python3", "scripts/version.py", version)
    run("python3", "site/build.py")
    run("git", "add", "-u")
    run("git", "add", "site/public")
    run("git", "commit", "--quiet", "-m", f"chore: version {version}")
    run("git", "tag", "-m", f"Remora {version}", tag)
    run("git", "push", "--quiet", "--atomic", "origin", "main", tag)
    print(f"✓ {tag} pushed in one commit: the release workflow builds, publishes, then deploys the website.")
    print(f"  Follow it: gh run list --branch {tag}")


if __name__ == "__main__":
    main()
