import assert from "node:assert/strict";
import test from "node:test";
import { getMethodId } from "@ton/core";

import { generatedTolkAbiSources } from "./tact-abi.mjs";

test("converts a Tact ABI to Tolk types and compiler ABI", async () => {
  const tactAbi = {
    name: "StableMaster",
    types: [
      {
        name: "Balance",
        header: null,
        fields: [
          {
            name: "amount",
            type: {
              kind: "simple",
              type: "uint",
              optional: false,
              format: "coins",
            },
          },
        ],
      },
      {
        name: "Payload",
        header: 0x12345678,
        fields: [
          {
            name: "signature",
            type: {
              kind: "simple",
              type: "fixed-bytes",
              optional: false,
              format: 64,
            },
          },
          {
            name: "balance",
            type: {
              kind: "simple",
              type: "Balance",
              optional: true,
              format: "ref",
            },
          },
          {
            name: "tail",
            type: {
              kind: "simple",
              type: "slice",
              optional: false,
              format: "remainder",
            },
          },
        ],
      },
      {
        name: "UnsupportedPayload",
        header: 0x87654321,
        fields: [
          {
            name: "unsupported",
            type: { kind: "tuple", items: [] },
          },
        ],
      },
      {
        name: "StableMaster$Data",
        header: null,
        fields: [
          {
            name: "balances",
            type: {
              kind: "dict",
              key: "uint",
              keyFormat: 256,
              value: "Balance",
              valueFormat: "ref",
            },
          },
          {
            name: "legacyBalances",
            type: {
              kind: "dict",
              key: "uint",
              keyFormat: 256,
              value: "Balance",
            },
          },
          {
            name: "owner",
            type: { kind: "simple", type: "address", optional: true },
          },
          {
            name: "sequenceNumber",
            type: {
              kind: "simple",
              type: "int",
              optional: false,
              format: 257,
            },
          },
        ],
      },
    ],
    receivers: [
      { receiver: "internal", message: { kind: "typed", type: "Payload" } },
      {
        receiver: "internal",
        message: { kind: "typed", type: "UnsupportedPayload" },
      },
      { receiver: "external", message: { kind: "typed", type: "Payload" } },
    ],
    getters: [
      {
        name: "getBalance",
        methodId: 777,
        arguments: [
          {
            name: "owner",
            type: { kind: "simple", type: "address", optional: false },
          },
        ],
        returnType: { kind: "simple", type: "Balance", optional: false },
      },
      {
        name: "seqno",
        arguments: [],
        returnType: {
          kind: "simple",
          type: "int",
          optional: false,
          format: 257,
        },
      },
      {
        name: "address",
        arguments: [],
        returnType: {
          kind: "simple",
          type: "address",
          optional: false,
        },
      },
      {
        name: "random",
        methodId: 7777,
        arguments: [],
        returnType: {
          kind: "simple",
          type: "int",
          optional: false,
          format: 257,
        },
      },
    ],
    errors: { 401: { message: "Unauthorized sender" } },
  };

  const sources = await generatedTolkAbiSources(
    { name: tactAbi.name, abi: JSON.stringify(tactAbi) },
    [
      {
        path: "output/verifier_StableMaster.abi",
        content: JSON.stringify(tactAbi),
      },
    ],
  );

  assert.deepEqual(
    sources.map((source) => source.path),
    ["output/StableMaster.types.tolk", "output/StableMaster.abi.json"],
  );
  assert.match(sources[0].content, /storage: StableMasterData/);
  assert.match(sources[0].content, /signature: bits512/);
  assert.match(sources[0].content, /balance: Cell<StableMasterBalance>\?/);
  assert.match(
    sources[0].content,
    /balances: map<uint256, Cell<StableMasterBalance>>/,
  );
  assert.match(
    sources[0].content,
    /legacyBalances: map<uint256, Cell<StableMasterBalance>>/,
  );
  assert.match(sources[0].content, /tail: RemainingBitsAndRefs/);
  assert.match(sources[0].content, /sequenceNumber: int257/);
  assert.match(
    sources[0].content,
    /\/\/ Tact method ID: 777\nget fun getBalance/,
  );
  assert.match(sources[0].content, /get fun seqno/);
  assert.doesNotMatch(sources[0].content, /tactAbiGetter_/);
  assert.match(
    sources[0].content,
    /\/\/ Tact getter name: address\n\/\/ Tact method ID: 69216\nget fun address_/,
  );
  assert.match(
    sources[0].content,
    /\/\/ Tact getter name: random\n\/\/ Tact method ID: 7777\nget fun random_/,
  );
  assert.doesNotMatch(sources[0].content, /onInternalMessage/);
  assert.doesNotMatch(sources[0].content, /UnsupportedPayload/);

  const abi = JSON.parse(sources[1].content);
  assert.equal(abi.contract_name, "StableMaster");
  assert.equal(abi.compiler_name, "tolk");
  assert.equal(abi.compiler_version, "1.4.2");
  assert.equal(abi.incoming_messages.length, 1);
  assert.equal(abi.incoming_external.length, 1);
  const customGetter = abi.get_methods.find(
    (getter) => getter.name === "getBalance",
  );
  assert.equal(customGetter.tvm_method_id, 777);
  const defaultGetter = abi.get_methods.find(
    (getter) => getter.name === "seqno",
  );
  assert.equal(defaultGetter.tvm_method_id, getMethodId("seqno"));
  const addressGetter = abi.get_methods.find(
    (getter) => getter.name === "address",
  );
  assert.equal(addressGetter.tvm_method_id, getMethodId("address"));
  const randomGetter = abi.get_methods.find(
    (getter) => getter.name === "random",
  );
  assert.equal(randomGetter.tvm_method_id, 7777);
  assert.deepEqual(abi.thrown_errors, [
    {
      kind: "enum_member",
      name: "StableMasterErrors.UnauthorizedSender",
      description: "Unauthorized sender",
      err_code: 401,
    },
  ]);
});

test("omits generated ABI when the Tact ABI cannot be converted", async () => {
  const tactAbi = {
    name: "UnsupportedStorage",
    types: [
      {
        name: "UnsupportedStorage$Data",
        fields: [
          {
            name: "raw",
            type: { kind: "simple", type: "slice", optional: false },
          },
        ],
      },
    ],
  };

  const sources = await generatedTolkAbiSources(
    { name: tactAbi.name, abi: JSON.stringify(tactAbi) },
    [],
  );

  assert.equal(sources, undefined);
});
