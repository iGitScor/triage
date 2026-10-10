.PHONY: build test app run icon clean

build:
	swift build

test:
	swift test

icon:
	swiftc -o .build/make-icon scripts/icon/main.swift Sources/Remora/Presentation/Components/RemoraArt.swift
	.build/make-icon $(CURDIR)

app:
	./scripts/build-app.sh

run: app
	-pkill -x Remora
	@# Reopening while the old copy is still quitting fails: wait for it (10 s at most).
	@for i in $$(seq 50); do pgrep -x Remora >/dev/null || break; sleep 0.2; done
	open build/Remora.app

clean:
	rm -rf .build build
