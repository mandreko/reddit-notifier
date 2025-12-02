FROM alpine:latest AS certs
RUN apk --no-cache add ca-certificates

FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
WORKDIR /app
RUN apk --no-cache add musl-dev openssl-dev openssl-libs-static

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
LABEL org.opencontainers.image.source=https://github.com/mandreko/reddit-notifier
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/release/reddit-notifier /app/
COPY --from=builder /app/target/release/reddit-notifier-tui /app/
COPY --from=builder /app/target/release/healthcheck /app/
USER 65534
VOLUME /data
# Set default ENV values, which user can over-ride
ENV DATABASE_URL=sqlite:///data/reddit-notifier.db
ENV POLL_INTERVAL_SECS=60
ENV REDDIT_USER_AGENT="reddit_notifier (https://github.com/mandreko/reddit-notifier)"

# Healthcheck: Verify database file exists and is readable
# Checks every 30s, times out after 5s, allows 3 retries before marking unhealthy
# Uses minimal healthcheck binary (no shell required)
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/app/healthcheck"]

CMD ["/app/reddit-notifier"]