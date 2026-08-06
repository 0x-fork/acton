import {describe, expect, test} from "bun:test"

import {parseBlockSearchQuery} from "../src/components/ExplorerSearch"

describe("parseBlockSearchQuery", () => {
  const expectedBlock = {
    workchain: -1,
    shard: "8000000000000000",
    seqno: 84_386_743,
  }

  test("accepts a full block ID without parentheses", () => {
    expect(parseBlockSearchQuery("-1,8000000000000000,84386743")).toEqual(expectedBlock)
  })

  test("keeps accepting a full block ID in parentheses", () => {
    expect(parseBlockSearchQuery("(-1,8000000000000000,84386743)")).toEqual(expectedBlock)
  })

  test("rejects unmatched parentheses", () => {
    expect(parseBlockSearchQuery("(-1,8000000000000000,84386743")).toBeUndefined()
    expect(parseBlockSearchQuery("-1,8000000000000000,84386743)")).toBeUndefined()
  })
})
