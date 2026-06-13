import {lookupTargetToQuery, type LookupTarget} from "./target"

export interface VerificationSourceResponse {
  readonly address: string | null
  readonly code_hash: string
  readonly verified: boolean
  readonly onchain: OnchainVerification
  readonly bundles: readonly SourceBundle[]
}

export interface OnchainVerification {
  readonly master_address: string
  readonly verification_record_address: string
}

export interface SourceBundle {
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly commit: string | null
  readonly bundle_path: string
  readonly language: string
  readonly compiler_version: string
  readonly entrypoint: string
  readonly compile_params: unknown
  readonly sources: readonly SourceFileSummary[]
  readonly files: readonly SourceFile[]
}

export interface SourceFileSummary {
  readonly path: string
  readonly is_entrypoint: boolean
}

export interface SourceFile {
  readonly path: string
  readonly sha256: string
  readonly content_base64: string
  readonly content_text: string | null
}

export class ApiRequestError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = "ApiRequestError"
    this.status = status
  }
}

export async function fetchVerificationSource(
  target: LookupTarget,
): Promise<VerificationSourceResponse> {
  const response = await fetch(`/api/v1/verification/source?${lookupTargetToQuery(target)}`, {
    headers: {
      accept: "application/json",
    },
  })

  const body = (await response.json().catch(() => undefined)) as {error?: string} | undefined
  if (!response.ok) {
    throw new ApiRequestError(response.status, body?.error || `Request failed: ${response.status}`)
  }

  return body as VerificationSourceResponse
}
