import {afterEach, expect, mock, spyOn, test} from "bun:test"

import {fetchWithRetry} from "../scripts/network/fetch-with-retry.ts"

const TEST_URL = "https://toncenter.com/api/v3/accountStates"
const originalSetTimeout = globalThis.setTimeout

const mockDelay = () =>
  spyOn(globalThis, "setTimeout").mockImplementation((callback, _milliseconds, ...args) =>
    originalSetTimeout(callback, 0, ...args),
  )

afterEach(() => mock.restore())

test("passes the API key and returns a successful response", async () => {
  const response = Response.json({ok: true})
  const request = spyOn(globalThis, "fetch").mockResolvedValue(response)

  expect(await fetchWithRetry(TEST_URL, {apiKey: "test-key"})).toBe(response)
  expect(request).toHaveBeenCalledTimes(1)
  const [, options] = request.mock.calls[0]
  expect(new Headers(options?.headers).get("X-API-Key")).toBe("test-key")
})

test("retries 429 five times by default with exponential delays", async () => {
  const response = new Response(null, {status: 429})
  const request = spyOn(globalThis, "fetch").mockResolvedValue(response)
  const delay = mockDelay()

  expect(await fetchWithRetry(TEST_URL)).toBe(response)
  expect(request).toHaveBeenCalledTimes(6)
  expect(delay.mock.calls.map(([, milliseconds]) => milliseconds)).toEqual([
    1000, 2000, 4000, 8000, 16_000,
  ])
})

test("returns as soon as the rate limit clears", async () => {
  const response = new Response(null, {status: 200})
  const request = spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(new Response(null, {status: 429}))
    .mockResolvedValueOnce(response)
  const delay = mockDelay()

  expect(await fetchWithRetry(TEST_URL)).toBe(response)
  expect(request).toHaveBeenCalledTimes(2)
  expect(delay.mock.calls.map(([, milliseconds]) => milliseconds)).toEqual([1000])
})

test.each([0, 2])("allows maxRetries=%i", async maxRetries => {
  const response = new Response(null, {status: 429})
  const request = spyOn(globalThis, "fetch").mockResolvedValue(response)
  const delay = mockDelay()

  expect(await fetchWithRetry(TEST_URL, {maxRetries})).toBe(response)
  expect(request).toHaveBeenCalledTimes(maxRetries + 1)
  expect(delay).toHaveBeenCalledTimes(maxRetries)
})

test.each([401, 503])("returns HTTP %i immediately", async status => {
  const response = new Response(null, {status})
  const request = spyOn(globalThis, "fetch").mockResolvedValue(response)
  const delay = mockDelay()

  expect(await fetchWithRetry(TEST_URL)).toBe(response)
  expect(request).toHaveBeenCalledTimes(1)
  expect(delay).not.toHaveBeenCalled()
})

test("propagates network errors immediately", async () => {
  const error = new TypeError("Network unavailable")
  const request = spyOn(globalThis, "fetch").mockRejectedValue(error)

  await expect(fetchWithRetry(TEST_URL)).rejects.toBe(error)
  expect(request).toHaveBeenCalledTimes(1)
})
