# One entry point for the whole repository. Each target calls the folder's own tooling (macos/Makefile, cargo, npm,
# the Python scripts), so nothing here duplicates how a part is built. `make` alone lists the targets.
# See CONTRIBUTING.md. On Windows without make, run the commands from desktop/README.md directly.

.DEFAULT_GOAL := help
.PHONY: help setup test check format format-check version deps-check mac-test mac-run mac-demo desktop-test desktop-check desktop-demo desktop-fixture i18n i18n-check test-counts \
	policy-files docs docs-build docs-check a11y links site site-preview screenshots deploy clean

help: ## List the targets
	@grep -E '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-13s %s\n", $$1, $$2}'

setup: ## Install the Node packages (tooling, Windows interface, docs) and the pre-commit hook
	npm ci
	npm --prefix desktop/app ci
	npm --prefix docs ci
	npx lefthook install
	npx playwright install chromium

test: mac-test desktop-test ## Run every test suite

check: format-check test desktop-check i18n-check docs-check ## Formatting, tests, lint, type-check, translations, policy files, docs and versions, as CI runs them
	python3 scripts/make-policy-files.py --check
	python3 scripts/version.py

# Formatting: biome.json (web), macos/.swift-format, desktop/rustfmt.toml, ruff.toml (Python). Needs `make setup` and uv.
RUFF := uvx ruff@0.16.8

format: ## Format every file in place
	npx biome check --write .
	cd macos && swift format format --in-place --parallel -r Sources Tests Package.swift
	cd desktop && cargo fmt --all
	$(RUFF) format .
	$(RUFF) check --fix .

format-check: ## Fail if a file isn't formatted or has a lint error (as CI does)
	npx biome ci .
	cd macos && swift format lint --strict --parallel -r Sources Tests Package.swift
	cd desktop && cargo fmt --all --check
	$(RUFF) format --check .
	$(RUFF) check .
	python3 scripts/test-counts.py --check
	python3 scripts/tokens-check.py

version: ## Show the version, or set it everywhere with V=x.y.z (then commit and tag vx.y.z)
	python3 scripts/version.py $(V)

deps-check: ## Known vulnerabilities and licences, as the Dependencies workflow checks them (needs cargo-deny)
	@command -v cargo-deny >/dev/null || { echo "Needs cargo-deny: cargo install cargo-deny --locked"; exit 1; }
	cd desktop && cargo deny check advisories licenses sources bans
	cd desktop/app && npm audit --omit=dev --audit-level=high
	cd docs && npm audit --audit-level=high
	npm audit --audit-level=critical

# macOS (macos/): Swift 6 package, macOS 14+, Xcode 16.

mac-test: ## macOS unit tests (skipped where Swift isn't installed, as on Linux)
	@command -v swift >/dev/null || { echo "No Swift here: skipping the macOS tests."; exit 0; }; $(MAKE) -C macos test

mac-run: ## Build macos/build/Remora.app and launch it (quits any running Remora first)
	$(MAKE) -C macos run

mac-demo: ## Build the macOS app and open it with sample data in a window, nothing saved
	$(MAKE) -C macos app
	open -n macos/build/Remora.app --args --demo

# Windows (desktop/): Rust workspace and a Tauri 2 app with a Svelte interface; Rust and Node 22.

desktop-test: ## Windows and Linux tests: the crates and the Tauri shell (any OS)
	cd desktop && cargo test --workspace

desktop-check: ## Type-check, test and build the Windows and Linux interface, then clippy on the whole workspace
	cd desktop/app && npm run check && npm test && npm run build
	cd desktop && cargo clippy --workspace --all-targets -- -D warnings

desktop-demo: ## The Windows and Linux tray app with sample data in a window (runs on a Mac too)
	cd desktop/app && npm run tauri dev -- -- --demo

# Strings: English in the code, French in macos/scripts/translations_fr.py for both apps.

desktop-fixture: ## Regenerate the Windows and Linux preview's demo data (a test fails when it is stale)
	cd desktop && cargo run -q -p remora_app --example demo_view > app/src/fixtures/demo.json
	npx biome format --write desktop/app/src/fixtures/demo.json

i18n: ## Regenerate the French strings of both apps from the dictionary
	python3 macos/scripts/make-strings.py
	python3 desktop/app/scripts/make-i18n.py

policy-files: ## Write the ready-to-import policy files in policy/ (profile, .reg, ADMX/ADML, Jamf, Linux)
	python3 scripts/make-policy-files.py

test-counts: ## Write the number of tests into the READMEs and the Windows docs (the pre-commit hook does it too)
	python3 scripts/test-counts.py

i18n-check: ## Fail if a string has no French, or the strings or emoji tables are out of date (as CI does)
	python3 macos/scripts/make-strings.py --check
	python3 desktop/app/scripts/make-i18n.py --check
	python3 scripts/make-emoji.py --check

# Website (site/) and documentation (docs/).

docs: ## The documentation with live reload
	npm --prefix docs run dev

docs-build: ## Build the documentation into site/public/docs
	npm --prefix docs run build
	python3 site/csp.py

docs-check: ## Build the docs and fail if their CSP or an internal link is out of date (as CI does)
	npm --prefix docs run build
	python3 site/csp.py --check
	python3 scripts/check-links.py

a11y: docs-build ## Check every page of the website and docs with axe (WCAG 2.1 AA, light and dark)
	node scripts/a11y.mjs

links: docs-build ## Check that every internal link of the website and docs leads to a page
	python3 scripts/check-links.py

site: ## Write the marketing pages (site/public) from site/build.py
	python3 site/build.py

site-preview: site docs-build ## Pages and docs together on http://127.0.0.1:8787
	python3 -m http.server 8787 --bind 127.0.0.1 --directory site/public

screenshots: ## Recapture the app screenshots used by the site, docs and README
	macos/scripts/screenshots.sh

deploy: ## Publish the website and docs to Cloudflare (maintainers; needs a wrangler login)
	site/deploy.sh

clean: ## Remove build outputs (macOS build, Windows interface bundle, built docs)
	$(MAKE) -C macos clean
	rm -rf desktop/app/dist site/public/docs
