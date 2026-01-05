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
