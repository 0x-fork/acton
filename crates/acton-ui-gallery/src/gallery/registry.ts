import {buttonGallery} from "./buttonGallery"
import {breadcrumbsGallery} from "./breadcrumbsGallery"
import {checkboxGallery} from "./checkboxGallery"
import {contentTabsGallery} from "./contentTabsGallery"
import {dataTableGallery} from "./dataTableGallery"
import {disclosureToggleGallery} from "./disclosureToggleGallery"
import {inlineActionsGallery} from "./inlineActionsGallery"
import {inlineButtonGallery} from "./inlineButtonGallery"
import {markdownTextGallery} from "./markdownTextGallery"
import {pillTabsGallery} from "./pillTabsGallery"
import {popoverGallery} from "./popoverGallery"
import {rawDataBlockGallery} from "./rawDataBlockGallery"
import {skeletonGallery} from "./skeletonGallery"
import {themeSwitchGallery} from "./themeSwitchGallery"
import {toastGallery} from "./toastGallery"
import type {ComponentGallery} from "./types"

export const galleries = [
  buttonGallery,
  breadcrumbsGallery,
  inlineButtonGallery,
  inlineActionsGallery,
  disclosureToggleGallery,
  contentTabsGallery,
  pillTabsGallery,
  markdownTextGallery,
  popoverGallery,
  toastGallery,
  rawDataBlockGallery,
  dataTableGallery,
  skeletonGallery,
  checkboxGallery,
  themeSwitchGallery,
] satisfies readonly ComponentGallery[]
