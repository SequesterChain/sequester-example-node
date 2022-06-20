.PHONY: run-dev
run-dev:
	./target/release/node-template --dev --ws-external

.PHONY: build-release
build-release:
	cargo build --release

.PHONY: purge-dev
purge-dev:
	./target/release/node-template purge-chain --dev
	
.PHONY: init
init:
	./scripts/init.sh

.PHONY: docker-run
docker-run:
	./scripts/docker_run.sh

.PHONY: test
test:
	SKIP_WASM_BUILD=1 cargo test --release --all