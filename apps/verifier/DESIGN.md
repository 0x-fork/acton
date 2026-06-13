# Code Hash Verification Registry

## Summary

This project provides a verification service for on-chain contract code hashes.
Developers submit source files and compilation parameters. The off-chain service
rebuilds the contract, compares the resulting code hash with the code hash they
want to verify, and stores the verified source package if the hashes match.

The system verifies a pure code hash, not a specific deployed address. A deployed
address can be described as "using verified code" only if its current on-chain
code hash is present in the registry.

GitHub is used only as a storage backend for source packages and metadata. The
verification proof lives on-chain.

## Goals

- Allow developers to publish source code that reproducibly compiles to a known
  on-chain code hash.
- Let third parties check whether a code hash has verified source code without
  trusting the backend or GitHub account.
- Keep the public verification key simple: `code_hash`.
- Store enough compilation metadata to make verification reproducible.
- Support multiple valid source packages for the same code hash.
- Keep the on-chain footprint small while still committing to the verified data.

## Non-Goals

- The system does not prove that a particular source package is the only possible
  source for a code hash.
- The system does not verify the contract data, initial state, owner, balance, or
  any address-specific property.
- The system does not guarantee that a deployed address will keep using the same
  code forever. Checkers must read the current code hash from the chain.
- The system does not rely on GitHub as a trusted registry. GitHub is only a
  convenient content storage and audit log.

## Terminology

- `code_hash`: The blockchain-native hash of the compiled contract code cell or
  equivalent code artifact. This is the primary verification key.
- `source bundle`: The complete set of source files and metadata required to
  rebuild a contract.
- `source_bundle_hash`: A deterministic hash of the canonical source bundle
  manifest and file contents.
- `compiler_fingerprint`: The exact compiler identity used for reproduction.
  This should include compiler name, version, build hash if available, standard
  library version, and any relevant toolchain metadata.
- `verification entry`: The on-chain record proving that a `code_hash` has at
  least one verified source bundle.
- `master contract`: The on-chain registry root. It maps `code_hash` values to
  deterministic verification sub-contracts or registry entries.
- `verification sub-contract`: A deterministic on-chain contract associated with
  one `code_hash`. Its existence and state represent the on-chain verification
  proof for that code hash.

## Design Principle

The canonical claim is:

```text
source bundle + compiler fingerprint + compilation flags -> code_hash
```

The registry does not claim:

```text
source bundle -> address
```

This distinction must be reflected in API names, UI labels, documentation, and
on-chain data structures.

Correct wording:

- "This code hash is verified."
- "This address currently uses a verified code hash."
- "This source package compiles to this code hash."

Incorrect wording:

- "This address is verified."
- "This source code proves the deployed contract state."

## Architecture

The system has three main parts:

1. Off-chain verification backend.
2. GitHub-backed source storage.
3. On-chain verification registry.

### Off-Chain Verification Backend

The backend receives verification requests from developers.

Input:

- Target `code_hash`.
- Source files.
- Optional compiler flags.
- Optional build configuration.
- Optional metadata such as project name, description, license, repository URL,
  and contact information.

Responsibilities:

- Validate the request format.
- Canonicalize source file paths and metadata.
- Resolve or validate compiler configuration.
- Compile the submitted sources in a reproducible environment.
- Compute the resulting `code_hash`.
- Compare the computed hash with the submitted target `code_hash`.
- Reject the request if the hashes do not match.
- Create a source bundle and deterministic `source_bundle_hash`.
- Store the source bundle in GitHub.
- Submit or help submit the on-chain verification transaction.
- Return the verification status to the caller.

The backend is not a trusted proof authority. It is an automation service. Any
third party must be able to independently rebuild the source bundle and compare
the resulting `code_hash`.

### GitHub Storage

GitHub stores verified source bundles and metadata.

GitHub provides:

- Public file hosting.
- Commit history.
- Human review surface.
- Easy mirroring and backup.

GitHub does not provide:

- The authoritative verification proof.
- Trust that a bundle is verified.
- Protection against a malicious or compromised storage account.

The authoritative proof is the on-chain registry entry. GitHub content is useful
only when its hash matches data committed on-chain.

### On-Chain Verification Registry

The on-chain registry provides a chain-native proof that a `code_hash` has one or
more verified source bundles.

The master contract exposes a get method:

```text
get_verification(code_hash) -> verification_sub_contract_address
```

A checker passes a `code_hash` into the master contract and receives the
deterministic verification sub-contract address. The checker then inspects that
sub-contract:

- If the sub-contract is inactive, the code hash is not verified.
- If the sub-contract is active, the code hash has at least one verified source
  bundle.
- The sub-contract state commits to the verified bundle identifiers, such as
  `source_bundle_hash` values.

The sub-contract address must be deterministically derived from the `code_hash`.
This allows anyone who knows only the `code_hash` to locate the verification
record.

## Verification Target

The system verifies only `code_hash`.

This means:

- Different deployed addresses with the same code hash share the same
  verification result.
- Verification remains valid even if the source was submitted for a different
  address, as long as the code hash is identical.
- Address-specific state is out of scope.
- A checker must first read the current code hash of the address they care about,
  then query the registry with that code hash.

## Source Bundle

A source bundle is the reproducible unit of verification.

It contains:

- All source files required by the build.
- A manifest.
- Compiler configuration.
- Compilation flags.
- Optional project metadata.
- Optional links to upstream repositories or releases.

The bundle must be canonicalized before hashing.

Canonicalization rules should include:

- Stable path normalization.
- No absolute local paths.
- Stable file ordering.
- Stable metadata ordering.
- Explicit text encoding.
- Explicit handling of line endings.
- Explicit handling of executable bits if relevant.
- Explicit exclusion of ignored or generated files unless they are required for
  compilation.

## Manifest

Each source bundle should include a `manifest.json` file.

Suggested fields:

```json
{
  "schema_version": 1,
  "code_hash": "0x...",
  "source_bundle_hash": "0x...",
  "compiler": {
    "name": "compiler-name",
    "version": "compiler-version",
    "fingerprint": "compiler-build-or-binary-hash",
    "stdlib": "stdlib-version-or-hash"
  },
  "build": {
    "entrypoint": "contracts/main.tolk",
    "flags": [],
    "environment": {}
  },
  "files": [
    {
      "path": "contracts/main.tolk",
      "sha256": "..."
    }
  ],
  "storage": {
    "provider": "github",
    "repository": "owner/repository",
    "commit": "...",
    "path": "verified/<code_hash>/<source_bundle_hash>/"
  },
  "submitted_at": "2026-06-13T00:00:00Z"
}
```

The exact schema can evolve, but the following fields are required for the first
version:

- `schema_version`
- `code_hash`
- `source_bundle_hash`
- `compiler`
- `build`
- `files`

## Storage Layout

Recommended GitHub layout:

```text
verified/
  <code_hash>/
    <source_bundle_hash>/
      manifest.json
      sources/
        ...
```

This layout allows:

- Multiple source bundles per code hash.
- Stable content-addressed paths.
- Simple mirroring.
- Simple lookup by `code_hash`.

The Git commit SHA may be stored in the manifest and returned by the API, but it
should not be the only identifier for a source bundle. The stable identifier is
`source_bundle_hash`.

## On-Chain Data Model

The master contract maps a `code_hash` to a deterministic verification
sub-contract address.

The verification sub-contract is keyed by `code_hash` and should store or commit
to:

- `code_hash`
- One or more `source_bundle_hash` values
- Optional schema version
- Optional storage pointer hash
- Optional verifier public key or backend identity
- Optional timestamp or logical registration time

The minimal useful proof is:

```text
code_hash -> active verification sub-contract -> source_bundle_hash list
```

If storing a full list is too expensive, the sub-contract may store a Merkle root
or another compact commitment. In that case, the API must provide inclusion
proofs for individual `source_bundle_hash` values.

## Verification Flow

### Submission Flow

1. Developer submits:
   - target `code_hash`
   - source files
   - compiler configuration
   - optional metadata
2. Backend validates request shape and size limits.
3. Backend canonicalizes file paths and build metadata.
4. Backend compiles the submitted sources using the requested compiler
   configuration.
5. Backend computes the resulting `code_hash`.
6. Backend compares the computed `code_hash` with the submitted target.
7. If the hashes do not match, backend returns a mismatch error.
8. If the hashes match, backend builds a source bundle.
9. Backend computes `source_bundle_hash`.
10. Backend writes the bundle to GitHub storage.
11. Backend sends or prepares the on-chain transaction that registers
    `source_bundle_hash` under `code_hash`.
12. Backend returns the final status.

### Third-Party Check Flow

To check whether an address uses verified code:

1. Read the current code hash of the address from the blockchain.
2. Call the master contract get method with that `code_hash`.
3. Derive or receive the verification sub-contract address.
4. Check whether the verification sub-contract is active.
5. Read the committed `source_bundle_hash` values.
6. Download matching source bundles from GitHub or another mirror.
7. Hash the downloaded bundle and compare it with the on-chain
   `source_bundle_hash`.
8. Recompile the bundle using the manifest.
9. Compare the locally computed code hash with the original `code_hash`.

If all checks pass, the address currently uses a verified code hash.

## Multiple Bundles for the Same Code Hash

The registry should allow multiple source bundles for the same `code_hash`.

Reasons:

- Different source trees can compile to identical bytecode.
- Comments, formatting, unused code, or build wrappers may not affect output.
- Multiple compiler configurations may produce the same final code hash.
- Historical submissions should not need to overwrite each other.

The UI and API should expose this explicitly:

```text
code_hash -> [source_bundle_hash_1, source_bundle_hash_2, ...]
```

The system may choose to mark one bundle as preferred for display, but this
preference must not affect the verification proof.

## API Design

Suggested backend endpoints:

```text
POST /verify
GET /status/{verification_id}
GET /code-hashes/{code_hash}
GET /code-hashes/{code_hash}/bundles
GET /bundles/{source_bundle_hash}
```

### POST /verify

Submits a verification request.

Request:

- `code_hash`
- files
- compiler configuration
- flags
- optional metadata

Response:

- `verification_id`
- status
- computed `source_bundle_hash` if available
- mismatch details if compilation succeeded but hash comparison failed

### GET /code-hashes/{code_hash}

Returns registry-level information for a code hash:

- whether an on-chain verification entry exists
- verification sub-contract address
- known source bundle hashes
- GitHub storage locations

### GET /bundles/{source_bundle_hash}

Returns bundle metadata:

- manifest
- storage location
- on-chain inclusion status
- compilation reproduction instructions

## Status Model

Verification should use explicit states:

- `submitted`: Request accepted.
- `validating`: Backend is checking request shape and limits.
- `compiling`: Backend is compiling the submitted source.
- `mismatch`: Compilation succeeded, but the resulting code hash does not match.
- `matched`: Compilation succeeded and code hash matched.
- `stored`: Source bundle was written to GitHub.
- `registering_onchain`: On-chain registration is pending.
- `verified`: On-chain proof is active.
- `failed`: Verification failed for a non-mismatch reason.

This avoids ambiguous partial success.

Examples:

- If GitHub storage succeeds but the on-chain transaction fails, the status is
  not `verified`.
- If on-chain registration succeeds but the backend loses local state, the
  checker can still recover truth from the chain and GitHub content hashes.
- If compilation fails, the result must include enough diagnostics for the
  submitter to fix the build.

## Reproducible Compilation Requirements

The backend must capture every input that can affect the compiled code hash:

- Compiler name.
- Compiler version.
- Compiler binary or build fingerprint if possible.
- Standard library version or hash.
- Build flags.
- Entrypoint.
- Include paths.
- Dependency versions.
- Target chain or VM version if relevant.
- Optimization settings.
- Any environment variables that affect compilation.

The long-term goal is that an independent checker can rebuild the bundle without
using the original backend.

## Trust Model

Trusted:

- The blockchain consensus for on-chain registry state.
- The deterministic compiler execution used by independent checkers.
- Cryptographic hash functions used for `code_hash` and `source_bundle_hash`.

Not trusted:

- The backend as a source of truth.
- The GitHub account as a source of truth.
- The submitter.
- A single frontend or explorer.

The backend may be trusted operationally to perform compilation and submission,
but correctness must be independently verifiable.

## Security Considerations

### Malicious Source Bundles

Submitted files may be malicious or intentionally expensive to compile.

Mitigations:

- Compile in sandboxed environments.
- Enforce request size limits.
- Enforce compilation time limits.
- Restrict network access during compilation.
- Restrict filesystem access during compilation.
- Store only canonicalized accepted files.

### Compiler Ambiguity

Different compiler builds with the same version string may produce different
output.

Mitigations:

- Store compiler binary fingerprints where possible.
- Prefer pinned compiler releases.
- Store Docker image digests or equivalent build environment identifiers if
  builds depend on containerized environments.

### GitHub Tampering

GitHub content may be edited, force-pushed, deleted, or replaced.

Mitigations:

- Store `source_bundle_hash` on-chain.
- Use content-addressed storage paths.
- Mirror storage to other backends when needed.
- Treat GitHub commit history as useful metadata, not proof.

### Backend Key Compromise

If the backend wallet or signing key is compromised, an attacker may register
incorrect or misleading bundle hashes if the registry accepts that key as an
authorized writer.

Mitigations:

- Keep backend signing keys isolated.
- Consider multisig or governance for registry updates.
- Add optional challenge or dispute mechanisms if invalid entries must be
  removable.
- Make independent recompilation easy so bad entries can be detected publicly.
- Consider requiring submitter-side signatures or user-wallet registration for
  source bundles when that fits the product model.

### Duplicate and Spam Submissions

Attackers may submit many bundles for the same code hash.

Mitigations:

- Enforce backend rate limits.
- Require fees or deposits for on-chain registration if needed.
- Deduplicate identical `source_bundle_hash` values.
- Apply size limits and per-code-hash bundle limits if storage costs become a
  problem.

## Failure Handling

The system must be safe under partial failure.

Important cases:

- Compilation fails: no storage write, no on-chain registration.
- Hash mismatch: no storage write, no on-chain registration.
- GitHub write fails: no on-chain registration should be submitted.
- On-chain registration fails: bundle may exist in GitHub, but it is not
  verified.
- On-chain registration succeeds but API state is lost: registry state remains
  authoritative.
- GitHub content is unavailable: code hash may still be verified on-chain, but
  source download is temporarily unavailable.

The backend should make operations idempotent using:

- `code_hash`
- `source_bundle_hash`
- deterministic storage paths
- deterministic on-chain sub-contract addresses

## Product and UX Rules

The product must consistently explain that verification is code-hash based.

Recommended labels:

- "Verified code hash"
- "Source packages"
- "This address uses verified code"
- "Code hash"
- "Bundle hash"

Avoid:

- "Verified address"
- "Verified contract owner"
- "Verified deployment"
- "Verified state"

For an address page, the UI should show:

- Address.
- Current code hash.
- Whether that code hash is verified.
- Verification sub-contract address.
- Source bundle list.
- Compiler and build metadata for each bundle.

## Open Questions

- Should on-chain registration store a list of `source_bundle_hash` values
  directly, or store a compact commitment such as a Merkle root?
- Should invalid or disputed bundles be removable, deprecated, or only marked as
  disputed?
- Who is allowed to register new bundles for an existing code hash?
- Should the first version require backend-managed registration, user wallet
  registration, or both?
- What exact canonicalization format should be used for source bundle hashing?
- Should source bundles be mirrored outside GitHub from the beginning?

## First Version Recommendation

The first implementation should keep the model simple:

1. Use `code_hash` as the only lookup key.
2. Store source bundles in GitHub under
   `verified/<code_hash>/<source_bundle_hash>/`.
3. Include a strict `manifest.json` with compiler, flags, file hashes, and
   storage metadata.
4. Register `source_bundle_hash` under `code_hash` on-chain.
5. Let the master contract map `code_hash` to a deterministic verification
   sub-contract.
6. Treat the sub-contract being active as the on-chain proof that the code hash
   has verified source.
7. Let the sub-contract commit to one or more source bundle hashes.
8. Make the public checker flow independent from the backend.
