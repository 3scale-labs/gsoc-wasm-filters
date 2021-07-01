FROM envoyproxy/envoy:v1.18.3

# Dynamically bind the build argument passed from the test func to select the configuration file.
ARG envoy

RUN echo $envoy

COPY $envoy /etc/envoy/envoy.yaml
COPY ./artifacts/cache_filter.wasm /usr/local/bin/cache_filter.wasm
COPY ./artifacts/singleton_service.wasm /usr/local/bin/singleton_service.wasm
COPY ./artifacts/threescale_wasm_auth.wasm /usr/local/bin/threescale_wasm_auth.wasm
RUN chmod go+r /etc/envoy/envoy.yaml /usr/local/bin/cache_filter.wasm /usr/local/bin/singleton_service.wasm
CMD /usr/local/bin/envoy -c /etc/envoy/envoy.yaml -l trace
