import {useEffect, useState} from "react"
import type {FC, ImgHTMLAttributes, SyntheticEvent} from "react"

import {TOKEN_PLACEHOLDER_IMAGE, deduplicateImageSources} from "./imageFallbacks"

interface NftImageProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "onError" | "src" | "srcSet"> {
  readonly sources: readonly string[]
  readonly blurred?: boolean
  readonly blurredClassName: string
}

interface ResolvedNftImage {
  readonly src: string
  readonly blurred: boolean
}

const getInitialImage = (sources: readonly string[], blurred: boolean): ResolvedNftImage => {
  const primarySource = sources[0]
  if (!primarySource) {
    return {src: TOKEN_PLACEHOLDER_IMAGE, blurred: false}
  }

  return {src: primarySource, blurred}
}

export const NftImage: FC<NftImageProps> = ({
  sources,
  blurred = false,
  blurredClassName,
  className = "",
  alt = "",
  ...imageProps
}) => {
  const sourcesKey = deduplicateImageSources(sources).join("\u0000")
  const [image, setImage] = useState<ResolvedNftImage>(() => getInitialImage(sources, blurred))

  useEffect(() => {
    const imageSources = sourcesKey ? sourcesKey.split("\u0000") : []
    setImage(getInitialImage(imageSources, blurred))
  }, [blurred, sourcesKey])

  const handleImageError = (event: SyntheticEvent<HTMLImageElement>) => {
    const imageSources = sourcesKey ? sourcesKey.split("\u0000") : []
    const currentSource = event.currentTarget.getAttribute("src")
    const currentIndex = currentSource ? imageSources.indexOf(currentSource) : -1
    const nextSource = imageSources[currentIndex + 1]
    setImage({
      src: nextSource ?? TOKEN_PLACEHOLDER_IMAGE,
      blurred: nextSource !== undefined && blurred,
    })
  }

  return (
    <img
      {...imageProps}
      src={image.src}
      alt={alt}
      className={`${className}${image.blurred ? ` ${blurredClassName}` : ""}`}
      onError={handleImageError}
    />
  )
}
