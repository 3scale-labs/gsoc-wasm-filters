.PHONY: build cache service clean clean-service clean-filter clean-auth integration run

build: export BUILD?=debug
build: auth
	@echo "> Building cache_filter and singleton_service"
    ifeq ($(BUILD), release)
		cargo build --target=wasm32-unknown-unknown --release $(CARGO_EXTRA_ARGS)
    else
		cargo build --target=wasm32-unknown-unknown $(CARGO_EXTRA_ARGS)
    endif
	cp target/wasm32-unknown-unknown/$(BUILD)/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
	cp target/wasm32-unknown-unknown/$(BUILD)/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

cache: export BUILD?=debug
cache:
	@echo "Building cache_filter"
    ifeq ($(BUILD), release)
		cargo build --package cache-filter --target=wasm32-unknown-unknown --release
    else
		cargo build --package cache-filter --target=wasm32-unknown-unknown
    endif
	cp target/wasm32-unknown-unknown/$(BUILD)/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm

service: export BUILD?=debug
service:
	@echo "> Building singleton_service"
    ifeq ($(BUILD), release)
		cargo build --package singleton-service --target=wasm32-unknown-unknown --release
    else
		cargo build --package singleton-service --target=wasm32-unknown-unknown
    endif
	cp target/wasm32-unknown-unknown/$(BUILD)/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

clean:
	@echo "> Cleaning all build artifacts"
	rm ./deployments/docker-compose/singleton_service.wasm
	rm ./deployments/docker-compose/cache_filter.wasm
	rm ./deployments/docker-compose/threescale_wasm_auth.wasm

clean-service:
	@echo "> Cleaning singleton_service build artifacts"
	rm ./deployments/docker-compose/singleton_service.wasm

clean-cache:
	@echo "> Cleaning cache_filter build artifacts"
	rm ./deployments/docker-compose/cache_filter.wasm

clean-auth:
	@echo "> Cleaning threescale_wasm_auth build artifacts"
	rm ./deployments/docker-compose/threescale_wasm_auth.wasm

auth: ## Build threescale_wasm_auth filter. 
	@echo "> Building threescale_wasm_auth filter"
	git submodule update --init
	cd threescale-wasm-auth && \
	make clean && \
	make build
	cp threescale-wasm-auth/target/wasm32-unknown-unknown/$(BUILD)/threescale_wasm_auth.wasm deployments/docker-compose/threescale_wasm_auth.wasm

integration:
	@echo "> Starting integration tests"
	mkdir -p integration-tests/artifacts
	cp deployments/docker-compose/cache_filter.wasm integration-tests/artifacts/cache_filter.wasm
	cp deployments/docker-compose/singleton_service.wasm integration-tests/artifacts/singleton_service.wasm
	cp deployments/docker-compose/threescale_wasm_auth.wasm integration-tests/artifacts/threescale_wasm_auth.wasm
	go clean -testcache
	go test -p 1 ./... -v
	rm -rf integration-tests/artifacts

run:
	@echo "> Starting services"
	docker-compose -f deployments/docker-compose/docker-compose.yaml up --build
	