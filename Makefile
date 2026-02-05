.PHONY: all build test proto clean wasm

all: proto build test

# Build the hudlc compiler
build:
	cargo build --release

# Run all tests
test:
	cargo test
	go test ./...

# Generate Go proto bindings from proto/views.proto
proto:
	protoc --go_out=. --go_opt=paths=source_relative proto/views.proto
	mv proto/views.pb.go pkg/hudl/pb/

# Compile example templates to WASM
wasm:
	cargo run --release -- examples -o views.wasm

# Clean build artifacts
clean:
	cargo clean
	rm -f views.wasm
	rm -rf hudl_build
