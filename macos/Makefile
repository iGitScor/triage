.PHONY: build test app run icon dmg clean

build:
	swift build -Xswiftc -warnings-as-errors

test:
	swift test -Xswiftc -warnings-as-errors

icon:
	swiftc -o .build/make-icon scripts/icon/main.swift Sources/Remora/Presentation/Components/RemoraArt.swift
	.build/make-icon $(CURDIR)

app:
	./scripts/build-app.sh

run: app
	@pkill -x Remora || true
	@# Reopening while the old copy is still quitting fails: wait for it (10 s at most).
	@for i in $$(seq 50); do pgrep -x Remora >/dev/null || break; sleep 0.2; done
	open build/Remora.app

# The DMG as released: the app, a shortcut to Applications and the licences, laid out on dmg/background.tiff.
# dmgbuild comes from dmg/requirements.txt, pinned by hash, in a venv of its own.
dmg:
	@test -d build/Remora.app || { echo "Build the app first: make app"; exit 1; }
	rm -rf build/Licences build/Remora.dmg && mkdir -p build/Licences
	cp ../LICENSE build/Licences/LICENSE.txt
	cp Resources/LICENCES.txt "build/Licences/Third-party licences.txt"
	cp Resources/Outfit-OFL.txt build/Licences/
	test -x .build/dmgbuild/bin/dmgbuild || { python3 -m venv .build/dmgbuild && \
		.build/dmgbuild/bin/pip install --quiet --require-hashes -r dmg/requirements.txt; }
	.build/dmgbuild/bin/dmgbuild -s dmg/settings.py -D app=build/Remora.app Remora build/Remora.dmg

clean:
	rm -rf .build build
