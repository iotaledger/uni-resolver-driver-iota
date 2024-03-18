FROM rust:1.67 AS build
COPY . ./app
WORKDIR /app
RUN cargo build --release

FROM ubuntu AS app
EXPOSE 8080
COPY --from=build /app/target/release/ /
ENV IOTA_CUSTOM_NETWORK_NAME=rms
ENV IOTA_CUSTOM_NODE_ENDPOINT=https://api.testnet.shimmer.network
# ENV IOTA_CUSTOM_NETWORK_NAME="snd"
# ENV IOTA_CUSTOM_NODE_ENDPOINT=http://localhost
CMD ["./uni-resolver-driver-iota"]
