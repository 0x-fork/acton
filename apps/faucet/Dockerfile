FROM ubuntu:noble

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY dist/docker/faucet-backend /usr/local/bin/faucet-backend

RUN chmod +x /usr/local/bin/faucet-backend

ENV PORT=3000 \
    HOST=0.0.0.0 \
    LOG_LEVEL=info \
    LOG_FORMAT=json \
    FAUCET_AMOUNT=500_000_000 \
    POW_DIFFICULTY=20

ENTRYPOINT ["/usr/local/bin/faucet-backend"]

HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://127.0.0.1:3000/health || exit 1
