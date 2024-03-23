FROM rust:slim-buster as builder

WORKDIR /usr/src/memflow-registry
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/memflow-registry /usr/local/bin/memflow-registry

ENV RUST_LOG=info
ENV MEMFLOW_ADDR=0.0.0.0:3000
ENV MEMFLOW_STORAGE_ROOT=/var/lib/memflow-registry/data/mfdata
ENV MEMFLOW_BEARER_TOKEN=token

RUN mkdir -p ${MEMFLOW_STORAGE_ROOT}

EXPOSE 3000
CMD [ "memflow-registry" ]
