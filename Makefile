VERSION = $(patsubst "%",%, $(word 3, $(shell grep version Cargo.toml)))
BUILD_TIME = $(shell date +"%Y/%m/%d %H:%M:%S")
GIT_REVISION = $(shell git log -1 --format="%h")
BIN_NAME = gip

export BUILD_TIME
export GIT_REVISION

.PHONY: all doc test clean release_lnx release_win release_mac

all: test

doc:
	cargo doc --no-deps

test:
	cargo test -- --nocapture

clean:
	cargo clean

release_lnx:
	cargo build --release --target=x86_64-unknown-linux-musl
	zip -j ${BIN_NAME}-v${VERSION}-x86_64-lnx.zip target/x86_64-unknown-linux-musl/release/${BIN_NAME}

release_win:
	cargo build --release --target=x86_64-pc-windows-gnu
	zip -j ${BIN_NAME}-v${VERSION}-x86_64-win.zip target/x86_64-pc-windows-gnu/release/${BIN_NAME}.exe

release_mac:
	cargo build --release --target=x86_64-apple-darwin
	zip -j ${BIN_NAME}-v${VERSION}-x86_64-mac.zip target/x86_64-apple-darwin/release/${BIN_NAME}
