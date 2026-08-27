import {expect, mock, test} from "bun:test"

import {VerifierMetadataRegistry} from "../src/metadata/verifierRegistry"

const mockFetch = (
  implementation: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>,
) => Object.assign(mock(implementation), {preconnect: globalThis.fetch.preconnect})

const CODE_HASH = "a".repeat(64)

test("stalled verifier ABI requests time out without blocking metadata resolution", async () => {
  const originalFetch = globalThis.fetch
  let requestSignal: AbortSignal | null | undefined
  globalThis.fetch = mockFetch((_input, init) => {
    requestSignal = init?.signal
    return rejectWhenAborted(requestSignal)
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    expect(requestSignal?.aborted).toBe(true)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("a timed-out verifier lookup is retried instead of being cached as missing", async () => {
  const originalFetch = globalThis.fetch
  let requestCount = 0
  globalThis.fetch = mockFetch((_input, init) => {
    requestCount += 1
    if (requestCount === 1) {
      return rejectWhenAborted(init?.signal)
    }
    return Promise.resolve(
      Response.json({
        items: [{code_hash: CODE_HASH, abi: {contract_name: "RecoveredContract"}}],
      }),
    )
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    const recovered = await registry.getCompilerAbis([CODE_HASH])

    expect(recovered[CODE_HASH]?.compiler_abi).toEqual({contract_name: "RecoveredContract"})
    expect(requestCount).toBe(2)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("stalled verifier source requests also fall back after the request deadline", async () => {
  const originalFetch = globalThis.fetch
  let requestSignal: AbortSignal | null | undefined
  globalThis.fetch = mockFetch((_input, init) => {
    requestSignal = init?.signal
    return rejectWhenAborted(requestSignal)
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getSource({codeHash: CODE_HASH})).toEqual({
      code_hash: CODE_HASH,
      verified: false,
      bundle: null,
    })
    expect(requestSignal?.aborted).toBe(true)
  } finally {
    globalThis.fetch = originalFetch
  }
})

function rejectWhenAborted(signal: AbortSignal | null | undefined): Promise<Response> {
  return new Promise((_resolve, reject) => {
    if (!signal) {
      reject(new Error("Expected verifier request to have an AbortSignal"))
      return
    }
    const rejectWithReason = () => reject(signal.reason)
    if (signal.aborted) {
      rejectWithReason()
      return
    }
    signal.addEventListener("abort", rejectWithReason, {once: true})
  })
}
