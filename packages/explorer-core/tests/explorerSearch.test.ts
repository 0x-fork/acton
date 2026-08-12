import {describe, expect, test} from "bun:test"

import {
  parseBlockSearchQuery,
  recoverSearchValueFromUrl,
  resolveSearchTarget,
} from "../src/components/ExplorerSearch"
import {createExplorerRoutes} from "../src/hooks/explorerRoutesContext"

const ZERO_ADDRESS = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"

describe("parseBlockSearchQuery", () => {
  const expectedBlock = {
    workchain: -1,
    shard: "8000000000000000",
    seqno: 123_456,
  }

  test("accepts a full block ID without parentheses", () => {
    expect(parseBlockSearchQuery("-1,8000000000000000,123456")).toEqual(expectedBlock)
  })

  test("keeps accepting a full block ID in parentheses", () => {
    expect(parseBlockSearchQuery("(-1,8000000000000000,123456)")).toEqual(expectedBlock)
  })

  test("accepts a colon-separated full block ID", () => {
    expect(parseBlockSearchQuery("-1:8000000000000000:123456")).toEqual(expectedBlock)
  })

  test("accepts a parenthesized colon-separated full block ID", () => {
    expect(parseBlockSearchQuery("(-1:8000000000000000:123456)")).toEqual(expectedBlock)
  })

  test("rejects mixed separators", () => {
    expect(parseBlockSearchQuery("-1:8000000000000000,123456")).toBeUndefined()
    expect(parseBlockSearchQuery("-1,8000000000000000:123456")).toBeUndefined()
  })

  test("rejects unmatched parentheses", () => {
    expect(parseBlockSearchQuery("(-1:8000000000000000:123456")).toBeUndefined()
    expect(parseBlockSearchQuery("-1:8000000000000000:123456)")).toBeUndefined()
  })
})

describe("resolveSearchTarget URL recovery", () => {
  test("recovers supported search targets from URL path segments", () => {
    const urls = [
      `https://viewer.example/${ZERO_ADDRESS}`,
      `https://explorer.example/address/${ZERO_ADDRESS}`,
      `explorer.example/address/${ZERO_ADDRESS}`,
      `https://explorer.example/address/${ZERO_ADDRESS}?network=mainnet#tokens`,
      `https://testnet.explorer.example/address/${ZERO_ADDRESS}`,
      `http://localhost:3000/address/${ZERO_ADDRESS}`,
      `localhost:3000/address/${ZERO_ADDRESS}`,
      `https://explorer.example/vesting/${ZERO_ADDRESS}`,
      `https://explorer.example/accounts/${ZERO_ADDRESS}#nfts`,
      `https://explorer.example/accounts/${ZERO_ADDRESS}/transactions`,
    ]
    const addressFormat = {bounceable: true, testOnly: false}
    const routes = createExplorerRoutes("", addressFormat)
    const expected = {
      displayValue: ZERO_ADDRESS,
      path: `/address/${ZERO_ADDRESS}`,
    }

    expect(urls.map(url => resolveSearchTarget(url, addressFormat, routes))).toEqual(
      urls.map(() => expected),
    )
    expect(urls.map(url => recoverSearchValueFromUrl(url))).toEqual(urls.map(() => ZERO_ADDRESS))

    const rawAddress = `0:${"0".repeat(64)}`
    expect(
      recoverSearchValueFromUrl(
        `https://explorer.example/address/${rawAddress}?network=mainnet#tokens`,
      ),
    ).toBe(rawAddress)
  })

  test("recovers transaction hashes from explorer URLs", () => {
    const transactionHash = "ab".repeat(32)
    const urls = [
      `https://explorer.example/tx/${transactionHash}?network=mainnet`,
      `https://viewer.example/transaction/${transactionHash}`,
      `explorer.example/tx/${transactionHash}`,
      `https://testnet.explorer.example/tx/${transactionHash}`,
      `http://localhost:3000/tx/${transactionHash}`,
      `https://explorer.example/transactions/${transactionHash}#trace`,
      `https://explorer.example/traces/${transactionHash}/details`,
    ]
    const addressFormat = {bounceable: true, testOnly: false}
    const routes = createExplorerRoutes("", addressFormat)
    const expected = {
      displayValue: transactionHash,
      path: `/tx/${transactionHash}`,
    }

    expect(urls.map(url => recoverSearchValueFromUrl(url))).toEqual(urls.map(() => transactionHash))
    expect(urls.map(url => resolveSearchTarget(url, addressFormat, routes))).toEqual(
      urls.map(() => expected),
    )
  })

  test("does not recover search targets outside complete URL path segments", () => {
    const transactionHash = "ab".repeat(32)
    const urls = [
      "https://explorer.example/accounts/unknown",
      `https://explorer.example/accounts/prefix-${ZERO_ADDRESS}-suffix`,
      `https://explorer.example/search?address=${ZERO_ADDRESS}`,
      `https://explorer.example/search#${ZERO_ADDRESS}`,
      `https://explorer.example/tx/${transactionHash.slice(1)}`,
      `https://explorer.example/tx/prefix-${transactionHash}-suffix`,
      `https://explorer.example/search?tx=${transactionHash}`,
      `https://explorer.example/search#${transactionHash}`,
    ]
    const addressFormat = {bounceable: true, testOnly: false}
    const routes = createExplorerRoutes("", addressFormat)

    expect(urls.map(url => resolveSearchTarget(url, addressFormat, routes))).toEqual(
      urls.map(() => undefined),
    )
    expect(urls.map(url => recoverSearchValueFromUrl(url))).toEqual(urls.map(() => undefined))
  })
})
