.PHONY: build bundle release clean

TARGET = wasm32-wasip1
CRATE_NAME = nd_lyrics

build:
	cargo build --release --target $(TARGET)
	mkdir -p bundle
	cp manifest.json bundle/
	cp target/$(TARGET)/release/*.wasm bundle/plugin.wasm
	cd bundle && zip -r ../nd-lyrics.ndp .
	rm -rf bundle
