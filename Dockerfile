FROM lukemathwalker/cargo-chef:latest-rust-latest as chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY . . 
# Compute lock file for project
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# Build project's dependencies
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
# Build project 
RUN cargo build --release --bin uni-resolver-driver-iota

FROM debian:bullseye-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean-up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/uni-resolver-driver-iota uni-resolver-driver-iota
EXPOSE 8080
ENV IOTA_CUSTOM_NETWORK_NAME=rms
ENV IOTA_CUSTOM_NODE_ENDPOINT=https://api.testnet.shimmer.network
ENTRYPOINT [ "./uni-resolver-driver-iota" ]