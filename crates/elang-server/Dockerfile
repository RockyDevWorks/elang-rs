# syntax=docker/dockerfile:1.7
#
# Multi-stage Dockerfile for the elang-server HTTP service.
#
# ⚠ BUILD CONTEXT — this Dockerfile expects the workspace root as its
# build context. The `COPY Cargo.toml Cargo.lock ./` and `COPY crates
# crates` steps rely on those files being at the context root; pointing
# the build at `crates/elang-server/` will fail at the first COPY. The
# workspace-root `fly.toml` configures Fly.io accordingly
# (`dockerfile = "crates/elang-server/Dockerfile"` with context = repo
# root); for local dev use `docker build -f crates/elang-server/Dockerfile
# -t elang-server:local .` from the workspace root.
#
# Build stage pulls from the workspace root so the same pinned dependency
# versions the CI matrix uses are the ones baked into the image — no
# `Cargo.lock` drift between local builds and CI. Runtime stage copies a
# single stripped binary; the resulting image is ≈ 30 MB compressed.

# Pinned to the multi-arch manifest digest so the image content is
# reproducible across rebuilds. Renovate's custom regex manager in
# `renovate.json` discovers both ARG values and proposes tag and digest
# updates. The defaults remain overridable via
# `docker build --build-arg RUST_IMAGE=...` for local experiments.
ARG RUST_IMAGE=rust:1.99-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8
# Use the numeric Debian alias so Renovate can order tag upgrades.
ARG DEBIAN_IMAGE=debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad

FROM ${RUST_IMAGE} AS build
WORKDIR /build

# Copy the locked workspace sources and the repertoire embedded at compile
# time. Source changes invalidate the build layer below.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY specs/schema/ipa-v1.json specs/schema/ipa-v1.json

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
    ELANG_SERVER_TIMEOUT_MS=30000 \
    ELANG_SERVER_MAX_CONCURRENT=4

ENTRYPOINT ["/usr/local/bin/elang-server"]
