import {expect, test} from "bun:test"

import {
  NFT_CARD_IMAGE_SOURCE_KEYS,
  NFT_COLLECTION_CARD_IMAGE_SOURCE_KEYS,
  NFT_IMAGE_SOURCE_KEYS,
  TOKEN_PLACEHOLDER_IMAGE,
  deduplicateImageSources,
  getImageSources,
  replaceBrokenImageWithFallback,
} from "../src/components/imageFallbacks"

const imageContent = {
  _image_small: "https://images.example/small.png",
  preview: "https://images.example/preview.png",
  _image_medium: "https://images.example/medium.png",
  _image_big: "https://images.example/big.png",
  image_url: "https://images.example/original.png",
}

test("uses small images for previews and larger images for NFT cards", () => {
  expect({
    preview: getImageSources(imageContent, NFT_IMAGE_SOURCE_KEYS),
    card: getImageSources(imageContent, NFT_CARD_IMAGE_SOURCE_KEYS),
  }).toMatchInlineSnapshot(`
    {
      "card": [
        "https://images.example/medium.png",
        "https://images.example/big.png",
        "https://images.example/original.png",
        "https://images.example/preview.png",
        "https://images.example/small.png",
      ],
      "preview": [
        "https://images.example/small.png",
        "https://images.example/preview.png",
        "https://images.example/medium.png",
        "https://images.example/big.png",
        "https://images.example/original.png",
      ],
    }
  `)
})

test("uses larger collection artwork in collection cards", () => {
  expect(
    getImageSources(
      {
        collection_image_small: "https://images.example/collection-small.png",
        collection_image_medium: "https://images.example/collection-medium.png",
        collection_image_big: "https://images.example/collection-big.png",
        ...imageContent,
      },
      NFT_COLLECTION_CARD_IMAGE_SOURCE_KEYS,
    ),
  ).toMatchInlineSnapshot(`
    [
      "https://images.example/collection-medium.png",
      "https://images.example/collection-big.png",
      "https://images.example/collection-small.png",
      "https://images.example/medium.png",
      "https://images.example/big.png",
      "https://images.example/original.png",
      "https://images.example/preview.png",
      "https://images.example/small.png",
    ]
  `)
})

test("deduplicates a combined image fallback chain", () => {
  expect(
    deduplicateImageSources([
      "https://images.example/small.png",
      "https://images.example/original.png",
      "https://images.example/small.png",
      TOKEN_PLACEHOLDER_IMAGE,
      "https://images.example/original.png",
    ]),
  ).toEqual(["https://images.example/small.png", "https://images.example/original.png"])
})

test("a duplicated fallback chain reaches the placeholder instead of restarting", () => {
  const image = {
    src: "https://images.example/small.png",
    getAttribute: () => image.src,
  }
  const event = {currentTarget: image} as unknown as Parameters<
    typeof replaceBrokenImageWithFallback
  >[0]
  const sources = [
    "https://images.example/small.png",
    "https://images.example/original.png",
    "https://images.example/small.png",
    "https://images.example/original.png",
  ]

  replaceBrokenImageWithFallback(event, sources)
  expect(image.src).toBe("https://images.example/original.png")

  replaceBrokenImageWithFallback(event, sources)
  expect(image.src).toBe(TOKEN_PLACEHOLDER_IMAGE)

  replaceBrokenImageWithFallback(event, sources)
  expect(image.src).toBe(TOKEN_PLACEHOLDER_IMAGE)
})
