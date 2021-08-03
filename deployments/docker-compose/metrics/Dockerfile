FROM envoyproxy/envoy:v1.18.3

COPY ./metrics/envoy.yaml /etc/envoy/envoy.yaml
COPY ./cache_filter.wasm /usr/local/bin/cache_filter.wasm
COPY ./singleton_service.wasm /usr/local/bin/singleton_service.wasm
COPY ./threescale_wasm_auth.wasm /usr/local/bin/threescale_wasm_auth.wasm
RUN chmod go+r /etc/envoy/envoy.yaml /usr/local/bin/cache_filter.wasm /usr/local/bin/singleton_service.wasm
# -l trace or debug can be added depending on the log level required for testing/demos.
CMD /usr/local/bin/envoy -c /etc/envoy/envoy.yaml --service-cluster envoyproxy
