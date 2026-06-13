# Server Deployment Guide

This document describes how to deploy the verifier backend as a Docker service on a server.

The Docker image contains the verifier backend, Node.js, the Tolk compiler worker, `@ton/tolk-js@1.4.1`, Git, and OpenSSH. It does not run a TON node by itself. The verifier needs a TonCenter-compatible API endpoint, which can be mainnet, testnet, or a localnet/lightnode endpoint.

## Architecture

At runtime the service needs:

- Verifier HTTP backend exposed on port `3000`.
- TonCenter-compatible API endpoint for code hash resolution and on-chain registry reads/writes.
- Registry master contract address.
- Wallet mnemonic for sending registration messages to the registry master.
- Git source repository for verified source bundles.
- Git credentials that allow pushing to the source repository.

The verifier stores verified source bundles in Git under:

```text
sources/{code_hash}/{source_bundle_hash}/
```

Each bundle contains:

- `manifest.json`
- `files/...`

## Prerequisites

Install on the server:

- Docker Engine
- Docker Compose plugin
- Git, only if you plan to build the image from the repository on the server

Recommended server setup:

- Linux VM with at least 2 CPU cores and 2 GB RAM
- Persistent Docker volume for `/var/lib/verifier/source-repo`
- Firewall allowing inbound traffic only to the reverse proxy or to port `3000` if exposing directly
- Secrets managed through environment files, Docker secrets, or your orchestrator

## Build The Image

From the repository root:

```bash
docker build -t ton-verifier:local .
```

For production, tag and push the image to your registry:

```bash
docker tag ton-verifier:local registry.example.com/ton-verifier:0.1.0
docker push registry.example.com/ton-verifier:0.1.0
```

On the server, either build locally from the checked-out repository or pull the published image:

```bash
docker pull registry.example.com/ton-verifier:0.1.0
```

## Configure The Service

Create an environment file on the server:

```bash
sudo mkdir -p /opt/ton-verifier
sudo install -m 600 /dev/null /opt/ton-verifier/verifier.env
```

Example `/opt/ton-verifier/verifier.env`:

```bash
VERIFIER_NETWORK=mainnet
VERIFIER_TONCENTER_BASE_URL=https://toncenter.com
VERIFIER_TONCENTER_API_KEY=
VERIFIER_REGISTRY_MASTER_ADDRESS=<registry-master-address>

WALLET_MNEMONIC=<24 words>

SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
SOURCE_REPOSITORY_BRANCH=main
SOURCE_REPOSITORY_AUTHOR_NAME=ton-verifier
SOURCE_REPOSITORY_AUTHOR_EMAIL=ton-verifier@example.invalid
SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
SOURCE_REPOSITORY_SSH_STRICT_HOST_KEY_CHECKING=accept-new

RUST_LOG=info
```

For testnet:

```bash
VERIFIER_NETWORK=testnet
VERIFIER_TONCENTER_BASE_URL=https://testnet.toncenter.com
```

For a localnet/lightnode on the Docker host:

```bash
VERIFIER_NETWORK=localnet
VERIFIER_TONCENTER_BASE_URL=http://host.docker.internal:5412
```

## Configure GitHub Source Storage

Use a dedicated repository for source storage. The verifier only needs push access to that repository.

Recommended authentication is an SSH deploy key with write access:

```bash
sudo install -m 700 -d /opt/ton-verifier/secrets
sudo install -m 600 source_repo_key /opt/ton-verifier/secrets/source_repo_key
```

The key should match:

```bash
SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
```

HTTPS remotes can also work, but then credentials must be provided through Docker secret mounts, a credential helper, or a tokenized remote URL. Avoid committing tokens to files or image layers.

## Docker Compose Deployment

Create `/opt/ton-verifier/docker-compose.yml`:

```yaml
services:
  verifier:
    image: ton-verifier:local
    restart: unless-stopped
    ports:
      - "3000:3000"
    env_file:
      - /opt/ton-verifier/verifier.env
    volumes:
      - source-repo:/var/lib/verifier/source-repo
      - /opt/ton-verifier/secrets/source_repo_key:/run/secrets/source_repo_key:ro
    extra_hosts:
      - "host.docker.internal:host-gateway"
    healthcheck:
      test: ["CMD", "node", "-e", "fetch('http://127.0.0.1:3000/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"]
      interval: 10s
      timeout: 3s
      retries: 6
      start_period: 10s

volumes:
  source-repo:
```

Start the service:

```bash
cd /opt/ton-verifier
docker compose up -d
```

Check status:

```bash
docker compose ps
docker compose logs -f verifier
curl -sS http://127.0.0.1:3000/healthz
```

Expected health response:

```json
{"ok":true}
```

## Systemd Wrapper

Use systemd to keep Docker Compose running after reboots.

Create `/etc/systemd/system/ton-verifier.service`:

```ini
[Unit]
Description=TON Verifier Backend
Requires=docker.service
After=docker.service network-online.target
Wants=network-online.target

[Service]
Type=oneshot
WorkingDirectory=/opt/ton-verifier
RemainAfterExit=yes
ExecStart=/usr/bin/docker compose up -d
ExecStop=/usr/bin/docker compose down
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
```

Enable it:

```bash
sudo systemctl daemon-reload
sudo systemctl enable ton-verifier
sudo systemctl start ton-verifier
sudo systemctl status ton-verifier
```

## Reverse Proxy

If exposing publicly, put the service behind Nginx, Caddy, Traefik, or another reverse proxy with TLS.

Minimal Nginx location:

```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Keep direct port `3000` closed to the public internet if the reverse proxy is the public entrypoint.

## Smoke Test

Check service health:

```bash
curl -sS http://127.0.0.1:3000/healthz
```

Check verification status for a known code hash:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/verification/status?code_hash=<code_hash>'
```

Submit a verification request:

```bash
curl -sS -X POST http://127.0.0.1:3000/api/v1/verify \
  -F code_hash=<code_hash> \
  -F language=tolk \
  -F 'compile_params={"compiler_version":"1.4.1"}' \
  -F 'sources=[{"path":"main.tolk","is_entrypoint":true},{"path":"imports/math.tolk","is_entrypoint":false}]' \
  -F 'files=@./main.tolk;filename=main.tolk' \
  -F 'files=@./imports/math.tolk;filename=imports/math.tolk'
```

Successful response should contain:

```json
{
  "verification_result": "match",
  "source_storage": {
    "provider": "git",
    "commit": "..."
  },
  "onchain_registration": {
    "status": "confirmed"
  }
}
```

Fetch verified source:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/verification/source?code_hash=<code_hash>'
```

## Updating

Build or pull the new image:

```bash
docker pull registry.example.com/ton-verifier:0.1.1
```

Update the image tag in `/opt/ton-verifier/docker-compose.yml`, then restart:

```bash
cd /opt/ton-verifier
docker compose up -d
docker compose logs -f verifier
```

Run smoke checks after every update.

## Backup

Back up:

- GitHub source repository
- `/opt/ton-verifier/verifier.env`
- SSH deploy key or Git credentials
- Docker Compose file
- Any local wallet/secret files

The Docker `source-repo` volume is a local clone. The authoritative source storage should be the remote Git repository after every successful push.

## Troubleshooting

### `source_repository.path` missing

The service has no Git storage path configured. Set:

```bash
SOURCE_REPOSITORY_PATH=/var/lib/verifier/source-repo
```

or mount a TOML config with `[source_repository].path`.

### Git push fails

Check:

```bash
docker compose exec verifier git -C /var/lib/verifier/source-repo remote -v
docker compose exec verifier git -C /var/lib/verifier/source-repo status
```

For SSH remotes, verify the key mount:

```bash
docker compose exec verifier test -r /run/secrets/source_repo_key
```

Common causes:

- Deploy key does not have write access.
- `SOURCE_REPOSITORY_URL` points to HTTPS while only SSH credentials are mounted.
- Host key verification blocks the first connection.
- The branch configured in `SOURCE_REPOSITORY_BRANCH` is protected.

### On-chain registration fails

Check:

- `VERIFIER_REGISTRY_MASTER_ADDRESS` is correct for the selected network.
- `VERIFIER_NETWORK` matches the address format and wallet id.
- `VERIFIER_TONCENTER_BASE_URL` is reachable from inside the container.
- Wallet mnemonic is configured and funded.
- `VERIFIER_REGISTRY_REGISTER_VALUE_NANO` is enough for the registration transaction.

Connectivity check:

```bash
docker compose exec verifier node -e "fetch(process.env.VERIFIER_TONCENTER_BASE_URL + '/api/v2/jsonRPC').catch(e=>{console.error(e); process.exit(1)})"
```

### Compiler fails

Check that the worker exists:

```bash
docker compose exec verifier test -f /app/compiler-worker/compile-tolk.mjs
```

Check Node:

```bash
docker compose exec verifier node --version
```

Only Tolk compiler version `1.4.1` is currently supported.

### Container starts but `/healthz` is not reachable

Check:

```bash
docker compose logs verifier
docker compose ps
docker compose exec verifier cat /etc/verifier/config.toml
```

Make sure the generated config has:

```toml
[server]
bind_addr = "0.0.0.0:3000"
```

Binding to `127.0.0.1:3000` inside the container will not expose the service correctly through Docker port mapping.

## Operational Notes

- Keep only one verifier instance writing to the same Git checkout. The current Git storage lock is process-local.
- Use a dedicated Git repository for source storage.
- Do not bake mnemonics or deploy keys into the image.
- Prefer SSH deploy keys scoped to one repository.
- Use a reverse proxy for TLS and request size limits.
- Monitor logs for failed Git pushes, compiler errors, and TonCenter API errors.
