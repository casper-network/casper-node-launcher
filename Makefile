# This supports environments where $HOME/.cargo/env has not been sourced (CI, CLion Makefile runner)
CARGO  = $(or $(shell which cargo),  $(HOME)/.cargo/bin/cargo)
RUSTUP = $(or $(shell which rustup), $(HOME)/.cargo/bin/rustup)

RUST_TOOLCHAIN := $(shell cat rust-toolchain)

CARGO_OPTS := --locked
CARGO := $(CARGO) $(CARGO_TOOLCHAIN) $(CARGO_OPTS)

DISABLE_LOGGING = RUST_LOG=MatchesNothing

.PHONY: all
all: build

.PHONY: build
build:
	$(CARGO) build $(CARGO_FLAGS)

.PHONY: test
test:
	$(DISABLE_LOGGING) $(CARGO) test $(CARGO_FLAGS)

.PHONY: check-format
check-format:
	$(CARGO) fmt --all -- --check

.PHONY: format
format:
	$(CARGO) fmt --all

.PHONY: lint
lint:
	$(CARGO) clippy --all-targets --all-features --workspace -- -D warnings -A renamed_and_removed_lints

.PHONY: audit
audit:
	$(CARGO) audit

.PHONY: build-docs-stable
build-docs-stable: $(CRATES_WITH_DOCS_RS_MANIFEST_TABLE)

doc-stable/%: CARGO_TOOLCHAIN += +stable
doc-stable/%:
	$(CARGO) doc $(CARGO_FLAGS) --manifest-path "$*/Cargo.toml" --no-deps

.PHONY: check
check: \
	build-docs-stable \
	build \
	check-format \
	lint \
	audit \
	test


.PHONY: clean
clean:
	$(CARGO) clean

.PHONY: deb
deb: setup-cargo-packagers
	$(CARGO) deb

.PHONY: publish
publish:
	./publish.sh

.PHONY: setup-cargo-packagers
setup-cargo-packagers: setup
	$(CARGO) install cargo-deb

.PHONY: setup-audit
setup-audit: setup
	$(CARGO) install cargo-audit

.PHONY: setup
setup: rust-toolchain
	$(RUSTUP) update --no-self-update
	$(RUSTUP) toolchain install --no-self-update $(RUST_TOOLCHAIN)
