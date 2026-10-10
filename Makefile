# One entry point for the whole repository. Each target calls the folder's own tooling (macos/Makefile, cargo, npm,
# the Python scripts), so nothing here duplicates how a part is built. `make` alone lists the targets.
# See CONTRIBUTING.md. On Windows without make, run the commands from windows/README.md directly.

.DEFAULT_GOAL := help
.PHONY: help setup test check version deps-check mac-test mac-run mac-demo win-test win-check win-demo i18n i18n-check \
	docs docs-build site site-preview screenshots deploy clean

help: ## List the targets
	@grep -E '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-13s %s\n", $$1, $$2}'

setup: ## Install the Node packages (Windows interface, docs)
	npm --prefix windows/app ci
	npm --prefix docs ci

test: mac-test win-test ## Run every test suite

check: test win-check i18n-check ## Tests, lint, type-check, translations and versions, as CI runs them
	python3 scripts/version.py

version: ## Show the version, or set it everywhere with V=x.y.z (then commit and tag vx.y.z)
	python3 scripts/version.py $(V)

deps-check: ## Known vulnerabilities and licences, as the Dependencies workflow checks them (needs cargo-deny)
	@command -v cargo-deny >/dev/null || { echo "Needs cargo-deny: cargo install cargo-deny --locked"; exit 1; }
	cd windows && cargo deny check advisories licenses sources bans
	cd windows/app && npm audit --omit=dev --audit-level=high
	cd docs && npm audit --audit-level=critical

# macOS (macos/): Swift 6 package, macOS 14+, Xcode 16.

mac-test: ## macOS unit tests
	$(MAKE) -C macos test

mac-run: ## Build macos/build/Remora.app and launch it (quits any running Remora first)
	$(MAKE) -C macos run

mac-demo: ## Build the macOS app and open it with sample data in a window, nothing saved
	$(MAKE) -C macos app
	open -n macos/build/Remora.app --args --demo

# Windows (windows/): Rust workspace and a Tauri 2 app with a Svelte interface; Rust and Node 22.

win-test: ## Windows library crates' tests (any OS, no Node needed)
	cd windows && cargo test -p remora_core -p remora_plugins -p remora_app

win-check: ## Type-check, test and build the Windows interface, then clippy on the whole workspace
	cd windows/app && npm run check && npm test && npm run build
	cd windows && cargo clippy --workspace --all-targets -- -D warnings

win-demo: ## The Windows tray app with sample data in a window (runs on a Mac too)
	cd windows/app && npm run tauri dev -- -- --demo

# Strings: English in the code, French in macos/scripts/translations_fr.py for both apps.

i18n: ## Regenerate the French strings of both apps from the dictionary
	python3 macos/scripts/make-strings.py
	python3 windows/app/scripts/make-i18n.py

i18n-check: ## Fail if a string has no French, or the strings or emoji tables are out of date (as CI does)
	python3 macos/scripts/make-strings.py --check
	python3 windows/app/scripts/make-i18n.py --check
	python3 scripts/make-emoji.py --check

# Website (site/) and documentation (docs/).

docs: ## The documentation with live reload
	npm --prefix docs run dev

docs-build: ## Build the documentation into site/public/docs
	npm --prefix docs run build
	python3 site/csp.py

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
	rm -rf windows/app/dist site/public/docs
