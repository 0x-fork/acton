import {describe, expect, test} from "bun:test"
import {readFile} from "node:fs/promises"

import {
  findConfigAdditions,
  hasConfigAdditions,
  inspectConfigBoc,
  mergeConfigManifest,
  parseArguments,
  type MainnetConfigManifest,
} from "../scripts/check-mainnet-config"

const manifestUrl = new URL("../scripts/mainnet-config-fields.json", import.meta.url)

describe("mainnet config audit", () => {
  test("the pinned mainnet config is covered by the append-only manifest", async () => {
    const [rawBoc, manifest] = await Promise.all([
      readFile(new URL("./fixtures/mainnet-config-84773657.boc.base64", import.meta.url), "utf8"),
      readFile(manifestUrl, "utf8").then(contents => JSON.parse(contents) as MainnetConfigManifest),
    ])

    const additions = findConfigAdditions(manifest, inspectConfigBoc(rawBoc.trim()))
    expect(additions).toEqual({
      parameterIds: [],
      fields: {},
      parseErrors: {},
    })
    expect(hasConfigAdditions(additions)).toBe(false)
  })

  test("reports only additions and ignores fields absent from the latest config", () => {
    const manifest: MainnetConfigManifest = {
      network: "mainnet",
      parameters: {18: ["mc_cell_price_ps", "old_optional_field"]},
    }
    const additions = findConfigAdditions(manifest, [
      {id: 18, fields: ["mc_cell_price_ps", "new_field"]},
      {id: 46, fields: []},
    ])

    expect(additions).toEqual({
      parameterIds: [46],
      fields: {18: ["new_field"]},
      parseErrors: {},
    })
  })

  test("fixes additions without removing historical fields", () => {
    const manifest: MainnetConfigManifest = {
      network: "mainnet",
      parameters: {18: ["mc_cell_price_ps", "old_optional_field"]},
    }
    const parameters = [
      {id: 18, fields: ["mc_cell_price_ps", "new_field"]},
      {id: -123, fields: []},
    ]

    const fixed = mergeConfigManifest(manifest, parameters)

    expect(fixed).toEqual({
      network: "mainnet",
      parameters: {
        "-123": [],
        18: ["mc_cell_price_ps", "new_field", "old_optional_field"],
      },
    })
    expect(hasConfigAdditions(findConfigAdditions(fixed, parameters))).toBe(false)
  })

  test("accepts --fix", () => {
    expect(parseArguments(["--fix"])).toEqual({bocPath: undefined, fix: true})
    expect(() => parseArguments(["--update"])).toThrow("Unknown argument: --update")
  })
})
