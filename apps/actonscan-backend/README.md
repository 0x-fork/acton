# Actonscan backend

The backend indexes canonical TON blocks directly from LiteServer and exposes
rolling network TPS statistics for Actonscan.

The backend stores its checkpoint and TPS samples in SQLite. After a restart,
the indexer continues from the stored checkpoint and restores the TPS windows.

```sh
ACTONSCAN_CONFIG=apps/actonscan-backend/config.toml \
  cargo run --package actonscan-backend
```

The local server listens on `127.0.0.1:3008`. Its public endpoints are:

- `GET /healthz`
- `GET /openapi.json`
- `GET /api/v1/stats/tps`

The backend reads the configuration from `config.toml`. Set `ACTONSCAN_CONFIG`
to use a different path. The repository configuration is suitable when the
process starts from the workspace root. The container uses `docker/config.toml`.

Set `[storage].database_path` to the SQLite database path. The backend creates
the parent directory when it does not exist.

The Docker image declares `/var/lib/actonscan` as a volume. Without an explicit
mount, Docker creates an anonymous volume.

For deployments, mount a named volume at `/var/lib/actonscan`. When you replace
the container, use the same volume name.
