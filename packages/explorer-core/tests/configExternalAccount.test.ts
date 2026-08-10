import {describe, expect, test} from "bun:test"

import {getExternalAccountExplorerLink} from "../src/pages/configExternalAccount"

describe("external config account explorer links", () => {
  test.each([
    ["Ethereum", "Etherscan", "https://etherscan.io/address/0x1234"],
    ["BSC", "BscScan", "https://bscscan.com/address/0x1234"],
    ["Polygon", "PolygonScan", "https://polygonscan.com/address/0x1234"],
  ] as const)("maps %s accounts to %s", (externalChain, name, href) => {
    expect(getExternalAccountExplorerLink(externalChain, false, "0x1234")).toEqual({
      href,
      name,
    })
  })

  test("uses BscScan Testnet for BSC accounts on testnet", () => {
    expect(getExternalAccountExplorerLink("BSC", true, "0x1234")).toEqual({
      href: "https://testnet.bscscan.com/address/0x1234",
      name: "BscScan",
    })
  })

  test.each([
    "Ethereum",
    "Polygon",
  ] as const)("does not use a mainnet explorer for %s testnet accounts", externalChain => {
    expect(getExternalAccountExplorerLink(externalChain, true, "0x1234")).toBeUndefined()
  })
})
