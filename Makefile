.PHONY: build cache service release clean clean-service clean-filter clean-auth

build:
		cargo build --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
		cp target/wasm32-unknown-unknown/debug/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

cache:
		cargo build --package cache-filter --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm

service:
		cargo build --package singleton-service --target=wasm32-unknown-unknown
		cp target/wasm32-unknown-unknown/debug/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

release: auth
		cargo build --target=wasm32-unknown-unknown --release
		cp target/wasm32-unknown-unknown/release/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
		cp target/wasm32-unknown-unknown/release/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

clean: clean-service clean-cache clean-auth
		cargo clean

clean-service:
		rm ./deployments/docker-compose/singleton_service.wasm

clean-cache:
		rm ./deployments/docker-compose/cache_filter.wasm

clean-auth:
		rm ./deployments/docker-compose/threescale_wasm_auth.wasm

auth:
		git submodule update --init
		cd threescale-wasm-auth && \
		make clean && \
		export BUILD=release && \
		make build
		cp threescale-wasm-auth/target/wasm32-unknown-unknown/release/threescale_wasm_auth.wasm deployments/docker-compose/threescale_wasm_auth.wasm

