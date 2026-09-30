# Docker Deployment

Production deployments should pull the CI-built image:

```bash
docker pull ghcr.io/ton-blockchain/verifier:latest
```

Local development can build the image:

```bash
docker build -f apps/verifier/Dockerfile -t ton-verifier:local .
```

After configuring the source repository credentials and secret mounts,
initialize an empty source repository with a one-off container:

```bash
docker compose run --rm --no-deps -e VERIFIER_MODE=init verifier
```

The preparation script creates the required root commit with the source-storage
Git attributes and pushes the configured branch. Init mode then exits without
starting the backend. Do not persist `VERIFIER_MODE=init` on a service with an
automatic restart policy. The verifier refuses to start when the root commit is
missing or the current `.gitattributes` no longer contains
`<source_repository.storage_root>/** -text`.

Or use the local build override:

```bash
docker compose \
  -f apps/verifier/docker-compose.yml \
  -f apps/verifier/docker-compose.local.yml \
  up -d --build
```

Run with generated config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_TONCENTER_MAINNET_BASE_URL=https://toncenter.com \
  -e VERIFIER_TONCENTER_TESTNET_BASE_URL=https://testnet.toncenter.com \
  -e VERIFIER_PAYMENT_PRIMARY_NETWORK=testnet \
  -e 'VERIFIER_PAYMENT_ADDRESS=0:<64-hex-character-wallet-address>' \
  -e VERIFIER_PAYMENT_MIN_AMOUNT_NANO=500000000 \
  -e SOURCE_REPOSITORY_URL=https://github.com/i582/test-verify-repo \
  -e SOURCE_REPOSITORY_AUTH_MODE=none \
  -e SOURCE_REPOSITORY_STORAGE_ROOT=sources \
  -e SOURCE_REPOSITORY_BRANCH=main \
  -e SOURCE_REPOSITORY_COMMIT_ENABLED=true \
  -e SOURCE_REPOSITORY_PUSH_ENABLED=true \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  -v verifier-registry-index:/var/lib/verifier/registry-index \
  -v verifier-payment-ledger:/var/lib/verifier/payment-ledger \
  ghcr.io/ton-blockchain/verifier:latest
```

The verifier accepts payments on TON mainnet or testnet, selected with
`VERIFIER_PAYMENT_PRIMARY_NETWORK`. The payment address must use raw basechain
form. At startup, the service rebuilds the payment ledger from the selected
network's wallet history and reports `503` until the scan is complete. This
example sets the minimum payment to `0.5 GRAM`.

Set `VERIFIER_READ_ONLY=true` to reject tickets and submissions for new code
hashes while keeping verified source and metadata lookups available.

Set `VERIFIER_COMPILER_DISABLED` to a comma-separated list of compiler rules:

```bash
-e VERIFIER_COMPILER_DISABLED='tact,func@0.4.4,tolk@1.4.1'
```

Docker Compose also forwards this variable from the shell or `.env` file. The
entrypoint writes the list to `[compiler].disabled` when generating the TOML
config. An unset or empty value produces an empty list. Whitespace around entries
is ignored; empty entries or malformed `name@version` rules fail application
initialization. All compilers and versions are allowed unless a rule matches.
Names are case-insensitive, and versions are compared as literal strings without
format validation or range expansion. Disabled compilers return HTTP 403 before
a payment quote is issued or a verification payment is claimed. Already verified
bundles remain available.

New verification tickets require `compiler` and `compiler_version`. Clients with
`User-Agent: acton/<version>` at or below `1.2.0` may omit both fields. For backward
compatibility, these clients are exempt from `compiler.disabled` in both
`/api/v1/take_ticket` and `/api/v1/verify`, even when compiler metadata is supplied.
All other clients are checked against the deny list before a payment quote or
verification payment claim.
A missing or unrecognized User-Agent does not grant an exception. Verification
still uses the existing language and compiler parameters needed for compilation.

Blueprint clients must use version `0.47.1` or newer. Requests with an older
`User-Agent: blueprint/<version>` receive HTTP 400 with an upgrade message on
`/api/v1/verification/status`, `/api/v1/take_ticket`, and `/api/v1/verify`.

As with the other generated settings, an existing config file takes precedence.
Use `VERIFIER_FORCE_GENERATE_CONFIG=1` to regenerate it from the environment.

Or mount a full TOML config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_CONFIG=/etc/verifier/config.toml \
  -v ./config.toml:/etc/verifier/config.toml:ro \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  -v verifier-registry-index:/var/lib/verifier/registry-index \
  -v verifier-payment-ledger:/var/lib/verifier/payment-ledger \
  ghcr.io/ton-blockchain/verifier:latest
```

For SSH Git remotes, mount a deploy key and pass:

```bash
-e SOURCE_REPOSITORY_AUTH_MODE=ssh
-e SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
-e SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
-v ./source_repo_key:/run/secrets/source_repo_key:ro
```

For an HTTPS remote with credentials embedded in its URL, pass:

```bash
-e SOURCE_REPOSITORY_AUTH_MODE=url
-e SOURCE_REPOSITORY_URL=https://x-access-token:<token>@github.com/owner/repository.git
```

The `url` mode stores the credential in the checkout's Git remote configuration.
Avoid it for short-lived credentials, and make sure deployment output and error
logs do not expose the URL. Use `none` only for repositories that need no Git
credentials.

For a GitHub App installation, mount its private key and pass the App and
installation IDs explicitly:

```bash
-e SOURCE_REPOSITORY_AUTH_MODE=github_app
-e SOURCE_REPOSITORY_URL=https://github.com/owner/repository.git
-e SOURCE_REPOSITORY_GITHUB_APP_ID=<app-id>
-e SOURCE_REPOSITORY_GITHUB_APP_INSTALLATION_ID=<installation-id>
-e SOURCE_REPOSITORY_GITHUB_APP_PRIVATE_KEY_FILE=/run/secrets/github-app.pem
-v ./github-app.pem:/run/secrets/github-app.pem:ro
```

The verifier requests the installation token directly from the configured
installation ID whenever Git requests credentials. It does not store the token
in the remote URL or look up the installation by repository.

The image contains:

- `verifier` Rust backend
- `verifier-prepare-source-repository` initialization command and `yq`
- Node.js runtime
- `compiler-worker/compile.mjs`
- Static NPM compiler packages for supported FunC, Tact, and Tolk versions
- Git and OpenSSH client for source storage commit/push
