# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /frontend

COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend ./
RUN npm run build

FROM rust:1.95-slim-bookworm AS rust-builder

WORKDIR /build

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml clippy.toml ./
COPY src ./src
COPY --from=frontend-builder /frontend/dist ./frontend/dist

RUN cargo build --release --locked

FROM node:22-bookworm-slim AS compiler-worker

WORKDIR /compiler-worker

COPY compiler-worker/package.json compiler-worker/package-lock.json ./
RUN npm ci --omit=dev

COPY compiler-worker/compile.mjs ./compile.mjs
COPY compiler-worker/languages ./languages
COPY compiler-worker/fixtures ./fixtures

FROM node:22-bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=rust-builder /build/target/release/verifier /usr/local/bin/verifier
COPY --from=compiler-worker /compiler-worker /app/compiler-worker
COPY docker/entrypoint.sh /usr/local/bin/verifier-entrypoint

RUN chmod +x /usr/local/bin/verifier-entrypoint \
    && mkdir -p /etc/verifier /var/lib/verifier/source-repo \
    && chown -R node:node /etc/verifier /var/lib/verifier /app/compiler-worker

USER node

ENV VERIFIER_CONFIG=/etc/verifier/config.toml \
    VERIFIER_BIND_ADDR=0.0.0.0:3000 \
    VERIFIER_NETWORK=mainnet \
    VERIFIER_REGISTRY_REGISTER_VALUE_NANO=500000000 \
    VERIFIER_REGISTRY_CONFIRMATION_ATTEMPTS=20 \
    VERIFIER_REGISTRY_CONFIRMATION_DELAY_MS=1000 \
    VERIFIER_WALLET_KIND=v5r1 \
    VERIFIER_WALLET_WORKCHAIN=0 \
    SOURCE_REPOSITORY_PATH=/var/lib/verifier/source-repo \
    SOURCE_REPOSITORY_REMOTE=origin \
    SOURCE_REPOSITORY_BRANCH=main \
    SOURCE_REPOSITORY_AUTHOR_NAME=ton-verifier \
    SOURCE_REPOSITORY_AUTHOR_EMAIL=ton-verifier@example.invalid \
    VERIFIER_COMPILER_NODE_BIN=node \
    VERIFIER_COMPILER_WORKER_PATH=/app/compiler-worker/compile.mjs \
    VERIFIER_COMPILER_TIMEOUT_MS=5000

EXPOSE 3000

ENTRYPOINT ["tini", "--", "verifier-entrypoint"]
CMD ["verifier"]
