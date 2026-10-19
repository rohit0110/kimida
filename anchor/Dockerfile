# KimiDa keeper: the off-chain backend that co-signs and broadcasts due sweeps.
# Build context is anchor/. Secrets (keeper key, .env) are NOT baked in; see docker-compose.yml.

FROM rust:1.89-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev clang && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY programs ./programs
COPY client ./client
COPY keeper ./keeper
RUN cargo build --release -p kimida-keeper --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 keeper \
    && mkdir -p /data && chown keeper /data
COPY --from=build /src/target/release/kimida-keeper /usr/local/bin/kimida-keeper
USER keeper
WORKDIR /data
# Config comes from env (RPC_URL, KEEPER_KEYPAIR, KEEPER_STATE, KEEPER_SAFETY_MARGIN,
# KEEPER_EXPECTED_GENESIS_HASH), see .env.example.
ENTRYPOINT ["kimida-keeper"]
CMD ["run", "--every", "5"]
