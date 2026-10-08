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
	open build/Remora.app

clean:
	rm -rf .build build
