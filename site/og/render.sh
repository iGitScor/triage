#!/usr/bin/env bash
# Renders public/site/og.{en,fr}.png (1200×630) and public/site/apple-touch-icon.png from card.html,
# with the site's own styles and the real screenshots. Needs Node (npx downloads Playwright once).
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p public/_og && cp og/card.html og/card.css og/card.js public/_og/
python3 -m http.server 8788 --bind 127.0.0.1 --directory public >/dev/null 2>&1 &
server=$!
trap 'kill $server; rm -rf public/_og' EXIT
sleep 1

shot() { npx --yes playwright@1.63.0 screenshot --color-scheme light --wait-for-timeout 500 --viewport-size "$1" "http://127.0.0.1:8788/_og/card.html?$2" "public/site/$3"; }
shot "1200,630" "lang=en" og.en.png
shot "1200,630" "lang=fr" og.fr.png
shot "180,180" "icon" apple-touch-icon.png
