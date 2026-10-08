#!/usr/bin/env bash
# Builds the marketing pages, the social cards and the docs into site/public, then publishes them to Cloudflare.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 site/build.py
site/og/render.sh
[ -d docs/node_modules ] || npm --prefix docs ci
npm --prefix docs run build
cd site && npx --yes wrangler@4 deploy
