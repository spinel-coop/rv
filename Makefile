RV := ./target/release/rv

# Build rv in release mode
.PHONY: build
build:
	./bin/build-rv

# Run all tests
.PHONY: test
test:
	cargo test --locked

$(RV): build

# All smoke tests use Docker and build rv inside the container
.PHONY: smoke-test-discourse
smoke-test-discourse:
	./bin/smoke-tests/discourse

.PHONY: smoke-test-fastlane
smoke-test-fastlane:
	./bin/smoke-tests/fastlane

.PHONY: smoke-test-huginn
smoke-test-huginn:
	./bin/smoke-tests/huginn

# Clean up smoke test cloned repos
.PHONY: smoke-test-clean
smoke-test-clean:
	rm -rf temp/smoke-tests/*

# Integration tests (run in Docker containers)
.PHONY: integration-test-alpine
integration-test-alpine:
	docker run --rm -v "$(PWD):/rv" -w /rv rust:alpine sh -c "apk add --no-cache build-base && ./bin/build-rv && ./target/release/rv --version && ./target/release/rv --help"

.PHONY: integration-test-arch
integration-test-arch:
	docker run --rm --platform linux/amd64 -v "$(PWD):/rv" -w /rv archlinux:base-devel sh -c "pacman -Syu --noconfirm rust && ./bin/build-rv && ./target/release/rv --version && ./target/release/rv --help"
