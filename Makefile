.PHONY: build cache service clean clean-service clean-filter clean-auth integration run

build: export BUILD?=debug
build: auth
	@echo "> Building cache_filter and singleton_service"
	make cache
	make service
	cp target/wasm32-unknown-unknown/$(BUILD)/cache_filter.wasm ./deployments/docker-compose/cache_filter.wasm
	cp target/wasm32-unknown-unknown/$(BUILD)/singleton_service.wasm ./deployments/docker-compose/singleton_service.wasm

cache: export BUILD?=debug
cache:
	@echo "Building cache_filter"
    ifeq ($(BUILD), release)
		cargo build --package cache-filter --target=wasm32-unknown-unknown --release $(CACHE_EXTRA_ARGS)
    else
		cargo build --package cache-filter --target=wasm32-unknown-unknown $(CACHE_EXTRA_ARGS) 
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

clean-apisonator:
	docker rm my-redis -f
	docker rm listener -f
	docker rm worker -f

auth: ## Build threescale_wasm_auth filter. 
	@echo "> Building threescale_wasm_auth filter"
	git submodule foreach git pull origin main
	cd threescale-wasm-auth && \
	make clean && \
	make build
	cp threescale-wasm-auth/compose/wasm/threescale_wasm_auth.wasm deployments/docker-compose/threescale_wasm_auth.wasm

apisonator: ## Runs apisonator and redis container
	docker run -p 6379:6379 -d --name my-redis redis --databases 2
	docker run -e CONFIG_QUEUES_MASTER_NAME=redis://redis:6379/0 \
            -e CONFIG_REDIS_PROXY=redis://redis:6379/1 -e CONFIG_INTERNAL_API_USER=root \
            -e CONFIG_INTERNAL_API_PASSWORD=root -p 3000:3000 -d --link my-redis:redis \
            --name apisonator quay.io/3scale/apisonator 3scale_backend start

local-services: clean-apisonator
	@echo "> Starting local services for integration tests"
	docker-compose -f integration-tests/docker-compose.yaml up --build -d

integration: local-services
	@echo "> Starting integration tests"
	mkdir -p integration-tests/artifacts
	cp deployments/docker-compose/cache_filter.wasm integration-tests/artifacts/cache_filter.wasm
	cp deployments/docker-compose/singleton_service.wasm integration-tests/artifacts/singleton_service.wasm
	cp deployments/docker-compose/threescale_wasm_auth.wasm integration-tests/artifacts/threescale_wasm_auth.wasm
	go clean -testcache
	go test -p 1 ./... -v
	rm -rf integration-tests/artifacts
	docker-compose -f integration-tests/docker-compose.yaml down

run: export METRICS?=false	
run: clean-apisonator	
	@echo "> Starting services"	
    ifeq ($(METRICS), true)	
		docker-compose -f deployments/docker-compose/metrics/docker-compose.yaml up
    else
		docker-compose -f deployments/docker-compose/docker-compose.yaml up --build
    endif


	