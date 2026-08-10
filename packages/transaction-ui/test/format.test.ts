import {describe, expect, test} from "bun:test"

import {shortenMiddle} from "@acton/ui"
import {formatContractLetter} from "../src/lib/format"

describe("transaction UI formatting", () => {
  test("truncates long labels in the middle", () => {
    expect([
      shortenMiddle("short-name.ton", {maxLength: 20}),
      shortenMiddle("blackmarket-dot-tg-exch.ton", {maxLength: 20}),
    ]).toMatchInlineSnapshot(`
      [
        "short-name.ton",
        "blackmarke…-exch.ton",
      ]
    `)
  })

  test("keeps contract labels inside the ASCII alphabet after Z", () => {
    expect([0, 1, 25, 26, 27, 51, 52, 701, 702].map(formatContractLetter)).toMatchInlineSnapshot(`
      [
        "A",
        "B",
        "Z",
        "AA",
        "AB",
        "AZ",
        "BA",
        "ZZ",
        "AAA",
      ]
    `)
  })

  test("falls back for invalid contract label indexes", () => {
    expect(
      [-1, Number.NaN, Number.POSITIVE_INFINITY].map(formatContractLetter),
    ).toMatchInlineSnapshot(`
        [
          "?",
          "?",
          "?",
        ]
      `)
  })
})
