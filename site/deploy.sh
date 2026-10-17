#!/usr/bin/env bash
# Publishes the website and docs to Cloudflare from this computer. The release workflow does it after each release
# (its site job); this is the fallback. It writes nothing that is committed: the pages come from the release commit
# (`make release`), the social cards from `make og`.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 site/build.py
if ! git diff --quiet -- site/public; then
  echo "site/public differs from what is committed: commit the pages first (or release with make release)." >&2
  exit 1
fi
[ -d docs/node_modules ] || npm --prefix docs ci
npm --prefix docs run build
python3 site/csp.py
npm --prefix site ci --ignore-scripts
export WRANGLER_SEND_METRICS=false
cd site && npx --no-install wrangler deploy
