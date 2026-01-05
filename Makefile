# Smoke test Makefile
# Usage: make smoke-test-<project>
# Example: make smoke-test-discourse

RV := ./target/release/rv

# Build rv in release mode
.PHONY: build
build:
	cargo build --release --locked --all-features --bin rv

$(RV): build

# Project smoke tests (scripts in bin/smoke-tests/)
.PHONY: smoke-test-discourse
smoke-test-discourse:
	docker build -f smoke-tests/discourse/Dockerfile -t rv-smoke-discourse .
	@echo ""
	@echo "✅ Smoke test passed: rv ci successfully installed all Discourse gems"

.PHONY: smoke-test-fastlane
smoke-test-fastlane: $(RV)
	./bin/smoke-tests/fastlane

.PHONY: smoke-test-huginn
smoke-test-huginn: $(RV)
	./bin/smoke-tests/huginn

# Clean up smoke test cloned repos
.PHONY: smoke-test-clean
smoke-test-clean:
	rm -rf temp/smoke-tests/*
