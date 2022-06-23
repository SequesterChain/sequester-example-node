.PHONY: run-dev
run-dev:
	./target/release/node-template --dev --ws-external

.PHONY: build-release
build-release:
	cargo +nightly build --release

.PHONY: purge-dev
purge-dev:
	./target/release/node-template purge-chain --dev
	
.PHONY: init
init:
	./scripts/init.sh