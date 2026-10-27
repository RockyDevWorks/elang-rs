# syntax=docker/dockerfile:1.7
#
# Multi-stage Dockerfile for the elang-server HTTP service.
#
# Build stage pulls from the workspace root so the same pinned dependency
# versions the CI matrix uses are the ones baked into the image — no
# `Cargo.lock` drift between local builds and CI. Runtime stage copies a
# single stripped binary; the resulting image is ≈ 30 MB compressed.

ARG RUST_IMAGE=rust:1.99-bookworm
ARG DEBIAN_IMAGE=debian:bookworm-slim

FROM ${RUST_IMAGE} AS build
WORKDIR /build

# Copy the workspace manifest and lockfile first so dependency resolution
# caches independently of source changes.
COPY Cargo.toml Cargo.lock ./
COPY crates crates

# Build with the workspace's pinned dependencies and release profile.
# `--locked` enforces Cargo.lock equality so a stale lockfile fails the
# build instead of silently introducing a new transitive dep.
RUN cargo build --locked --release -p elang-server \
    && strip --strip-all target/release/elang-server

FROM ${DEBIAN_IMAGE} AS runtime
# `ca-certificates` lets the binary reach HTTPS services if a future
# build adds outbound calls (today it has none, but the image is the
# place to keep the dependency).
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root by default — even though Fly runs containers as `fly` user
# already, a stricter deployment target can rely on this.
RUN groupadd --system --gid 1001 elang \
    && useradd --system --uid 1001 --gid elang --no-create-home elang

COPY --from=build /build/target/release/elang-server /usr/local/bin/elang-server
USER elang

EXPOSE 8080

# Defaults match the binary's own env-var contract (see README).
ENV ELANG_SERVER_PORT=8080 \
    ELANG_SERVER_MAX_BODY_BYTES=10485760 \
    ELANG_SERVER_TIMEOUT_MS=30000

ENTRYPOINT ["/usr/local/bin/elang-server"]