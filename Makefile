.PHONY: check test self-test release clean install

RUST ?= 1.97.1

check:
	cargo +$(RUST) fmt --all -- --check
	cargo +$(RUST) check --all-targets
	cargo +$(RUST) clippy --all-targets -- -D clippy::correctness

test: check
	cargo +$(RUST) test --all-targets

self-test:
	python3 scripts/self_test.py

release: test self-test
	bash scripts/build-release.sh

install:
	cargo +$(RUST) install --path . --locked

clean:
	cargo clean
	rm -rf dist .codespace artifacts/self-context.txt artifacts/self-test-report.json artifacts/SELF_TEST.md
