import type {ReactNode} from "react"

export type GalleryNote = Readonly<{
  title: string
  items: readonly string[]
}>

export type GallerySection = Readonly<{
  id: string
  title: string
  description: string
  content: ReactNode
}>

export type ComponentGallery = Readonly<{
  id: string
  title: string
  status: string
  summary: string
  importStatement: string
  agentSummary: string
  usage: readonly string[]
  avoid: readonly string[]
  sections: readonly GallerySection[]
}>
