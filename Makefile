# Smoke test Makefile
# Usage: make smoke-test-<project>
# Example: make smoke-test-discourse

SMOKE_TEST_DIR := temp/smoke-tests
RV := ./target/release/rv

# Build rv in release mode
.PHONY: build
build:
	cargo build --release --locked --all-features --bin rv

$(RV): build

# Generic smoke test function
# Args: (1) project name, (2) git repo URL
define smoke_test
	@mkdir -p $(SMOKE_TEST_DIR)
	@if [ ! -d "$(SMOKE_TEST_DIR)/$(1)" ]; then \
		echo "Cloning $(1)..."; \
		git clone --depth 1 $(2) $(SMOKE_TEST_DIR)/$(1); \
	else \
		echo "$(1) already cloned, using existing copy"; \
	fi
	@echo "Installing Ruby version for $(1)..."
	cd $(SMOKE_TEST_DIR)/$(1) && ../../../$(RV) ruby install
	@echo "Running rv ci for $(1)..."
	cd $(SMOKE_TEST_DIR)/$(1) && ../../../$(RV) ci
	@echo "Smoke test for $(1) passed!"
endef

# Project targets (add more here as needed)
.PHONY: smoke-test-discourse
smoke-test-discourse: $(RV)
	$(call smoke_test,discourse,https://github.com/discourse/discourse.git)

# Clean up smoke test cloned repos (keeps temp/ directory)
.PHONY: smoke-test-clean
smoke-test-clean:
	rm -rf $(SMOKE_TEST_DIR)/*
