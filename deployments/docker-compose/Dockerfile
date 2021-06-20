FROM envoyproxy/envoy:v1.18.3

COPY ./envoy.yaml /etc/envoy/envoy.yaml
COPY ./cache_filter.wasm /usr/local/bin/cache_filter.wasm
COPY ./singleton_service.wasm /usr/local/bin/singleton_service.wasm
RUN chmod go+r /etc/envoy/envoy.yaml /usr/local/bin/cache_filter.wasm /usr/local/bin/singleton_service.wasm
CMD /usr/local/bin/envoy -c /etc/envoy/envoy.yaml