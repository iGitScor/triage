#!/usr/bin/env bash
# Builds the marketing pages, the social cards and the docs into site/public, then publishes them to Cloudflare.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 site/build.py
site/og/render.sh
[ -d docs/node_modules ] || npm --prefix docs ci
npm --prefix docs run build
python3 site/csp.py
# The wrangler of site/package-lock.json, exactly, never "the latest 4.x" with Cloudflare credentials.
npm --prefix site ci --ignore-scripts
# Wrangler sends usage telemetry to Cloudflare by default: not from here.
export WRANGLER_SEND_METRICS=false
cd site && npx --no-install wrangler deploy
