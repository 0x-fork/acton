# Docker Deployment

Production deployments should pull the CI-built image:

```bash
docker pull ghcr.io/i582/verifier:latest
```

Local development can build the image:

```bash
docker build -t ton-verifier:local .
```

Or use the local build override:

```bash
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d --build
```

Run with generated config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_NETWORK=localnet \
  -e VERIFIER_TONCENTER_BASE_URL=http://host.docker.internal:5412 \
  -e VERIFIER_REGISTRY_MASTER_ADDRESS=<registry-master-address> \
  -e WALLET_MNEMONIC="<24 words>" \
  -e SOURCE_REPOSITORY_URL=https://github.com/i582/test-verify-repo \
  -e SOURCE_REPOSITORY_BRANCH=main \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  ghcr.io/i582/verifier:latest
```

Or mount a full TOML config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_CONFIG=/etc/verifier/config.toml \
  -v ./config.toml:/etc/verifier/config.toml:ro \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  ghcr.io/i582/verifier:latest
```

For SSH Git remotes, mount a deploy key and pass:

```bash
-e SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
-e SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
-v ./source_repo_key:/run/secrets/source_repo_key:ro
```

The image contains:

- `verifier` Rust backend
- Node.js runtime
- `compiler-worker/compile-tolk.mjs`
- NPM dependencies, including `@ton/tolk-js@1.4.1`
- Git and OpenSSH client for source storage commit/push
