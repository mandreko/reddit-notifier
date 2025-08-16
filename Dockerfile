FROM alpine:latest AS certs
RUN apk --no-cache add ca-certificates

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
USER root
WORKDIR /app
ENV CARGO_INCREMENTAL=0
ENV CARGO_PROFILE_RELEASE_LTO=true
ENV CARGO_PROFILE_RELEASE_OPT_LEVEL=z
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/root/.cargo \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM scratch
LABEL authors="Matt Andreko <mandreko@gmail.com>"
LABEL org.opencontainers.image.source=https://github.com/mandreko/reddit-discord-notification-bot
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/release/reddit-notifier /app/
USER 65534
VOLUME /data
CMD ["/app/reddit-notifier"]