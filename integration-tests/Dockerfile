FROM envoyproxy/envoy:v1.18.3

ARG ENVOY_YAML

RUN echo $ENVOY_YAML

# Used to create the proxy instance for integration tests. envoy.yaml template should be 
# populated with keys before using it here.
COPY $ENVOY_YAML /etc/envoy/envoy.yaml

# Copying build artifacts.
COPY ./artifacts/cache_filter.wasm /usr/local/bin/cache_filter.wasm
COPY ./artifacts/singleton_service.wasm /usr/local/bin/singleton_service.wasm
COPY ./artifacts/threescale_wasm_auth.wasm /usr/local/bin/threescale_wasm_auth.wasm

# Exposing ports.
EXPOSE 9095
EXPOSE 9000

RUN chmod go+r /etc/envoy/envoy.yaml /usr/local/bin/cache_filter.wasm /usr/local/bin/singleton_service.wasm /usr/local/bin/threescale_wasm_auth.wasm
CMD /usr/local/bin/envoy -c /etc/envoy/envoy.yaml -l trace

