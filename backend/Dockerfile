# syntax=docker/dockerfile:1
# Backend image (issue #66): builds the `api` server + the `migration` binary,
# then ships them on a slim, non-root runtime.

FROM rust:1.97-bookworm AS builder
WORKDIR /build
# Copy the whole workspace and build the release binaries.
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --workspace --bins && \
    cp target/release/api /usr/local/bin/api && \
    cp target/release/migration /usr/local/bin/migration

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/api /usr/local/bin/api
COPY --from=builder /usr/local/bin/migration /usr/local/bin/migration
# Bind on all interfaces inside the container.
ENV ROCKET_ADDRESS=0.0.0.0 \
    ROCKET_PORT=8000 \
    APP_ENV=production
EXPOSE 8000
# Run as a non-root user.
USER 1000:1000
# Migrations run as a separate deploy step (`migration up`) under the owner role;
# the server itself connects as the non-owner acre_app role (see deploy/roles.sql).
CMD ["api"]
