# Actonscan backend

The backend indexes canonical TON blocks directly from LiteServer and exposes
rolling network TPS statistics for Actonscan.

```sh
ACTONSCAN_CONFIG=apps/actonscan-backend/config.toml \
  cargo run --package actonscan-backend
```

The local server listens on `127.0.0.1:3008`. Its public endpoints are:

- `GET /healthz`
- `GET /openapi.json`
- `GET /api/v1/stats/tps`

Configuration is read from `config.toml`. Set `ACTONSCAN_CONFIG` to use a
different path. The repository config is suitable when the process starts from
the workspace root; the container uses `docker/config.toml`.
