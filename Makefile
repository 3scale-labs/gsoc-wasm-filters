.PHONY: build
build:
		cargo build --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
		cp target/wasm32-unknown-unknown/debug/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

.PHONY: cache
cache:
		cargo build --package cache-filter --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm

.PHONY: service
service:
		cargo build --package singleton-service --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

.PHONY: release
release:
		cargo build --target=wasm32-unknown-unknown --release
		cp target/wasm32-unknown-unknown/release/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
		cp target/wasm32-unknown-unknown/release/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

.PHONY: clean
clean:
		rm ./deployments/docker-compose/cache_filter.wasm
		rm ./deployments/docker-compose/singleton_service.wasm


		