import {lookupTargetToQuery, type LookupTarget} from "./target"

export interface VerificationSourceResponse {
  readonly code_hash: string
  readonly verified: boolean
  readonly bundles: readonly SourceBundle[]
}

export interface SourceBundle {
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly storage_revision: string
  readonly compiler: CompilerMetadata
  readonly files: readonly SourceFile[]
}

export interface CompilerMetadata {
  readonly language: string
  readonly version: string
  readonly entrypoint: string
  readonly params: unknown
}

export interface SourceFile {
  readonly path: string
  readonly content_hash: string
  readonly include_in_command: boolean | null
  readonly is_stdlib: boolean | null
  readonly has_include_directives: boolean | null
  readonly content: string
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
