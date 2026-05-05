FROM ubuntu:noble

COPY dist/docker/faucet-backend /usr/local/bin/faucet-backend

RUN chmod +x /usr/local/bin/faucet-backend

ENV PORT=3000 \
    HOST=0.0.0.0 \
    LOG_LEVEL=info \
    LOG_FORMAT=json

ENTRYPOINT ["/usr/local/bin/faucet-backend"]

HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://127.0.0.1:3000/health || exit 1
