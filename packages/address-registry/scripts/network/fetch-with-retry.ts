export interface FetchWithRetryOptions {
  readonly apiKey?: string
  /** Retries after the initial request. */
  readonly maxRetries?: number
}

const wait = (milliseconds: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, milliseconds))

export const fetchWithRetry = async (
  url: string | URL,
  {apiKey, maxRetries = 5}: FetchWithRetryOptions = {},
): Promise<Response> => {
  const headers = apiKey ? {"X-API-Key": apiKey} : undefined

  for (let retry = 0; ; retry += 1) {
    // biome-ignore lint/performance/noAwaitInLoops: retries must remain sequential
    const response = await fetch(url, {headers})
    if (response.status !== 429 || retry >= maxRetries) {
      return response
    }

    await wait(1000 * 2 ** retry)
  }
}
