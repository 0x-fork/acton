import {InfoPopover} from "@acton/ui"
import {CircleDot, Network} from "lucide-react"

import {formatEnvironmentNetwork, formatEnvironmentType} from "../environmentPresentation"
import type {StudioEnvironment} from "../studioApi"

import styles from "./EnvironmentInfoValue.module.css"

interface EnvironmentInfoValueProps {
  readonly environment: StudioEnvironment
  readonly property: "type" | "network"
}

/**
 * Displays an environment classification with the shared explanation used across Studio
 *
 * The component owns this wording so identical Type and Network values do not acquire
 * different meanings as they appear in the environment list, Dashboard, and Settings
 */
export function EnvironmentInfoValue({environment, property}: EnvironmentInfoValueProps) {
  const value =
    property === "type"
      ? formatEnvironmentType(environment.config)
      : formatEnvironmentNetwork(environment)
  const description =
    property === "type"
      ? getEnvironmentTypeDescription(environment)
      : getEnvironmentNetworkDescription(environment)
  const ValueIcon = getEnvironmentValueIcon(environment, property)

  return (
    <span className={styles.value}>
      {ValueIcon ? <ValueIcon className={styles.icon} size={15} aria-hidden="true" /> : undefined}
      <span className={styles.label}>{value}</span>
      <InfoPopover ariaLabel={`About ${property} ${value}`}>{description}</InfoPopover>
    </span>
  )
}

function getEnvironmentValueIcon(
  environment: StudioEnvironment,
  property: EnvironmentInfoValueProps["property"],
) {
  if (property === "network" && environment.config.kind !== "remoteTonNetwork") {
    return undefined
  }

  if (
    environment.config.kind === "fullTonNetwork" ||
    environment.config.kind === "remoteTonNetwork"
  ) {
    return Network
  }

  return CircleDot
}

function getEnvironmentTypeDescription(environment: StudioEnvironment): string {
  const {config} = environment

  if (config.kind === "fullTonNetwork") {
    return "This environment runs a complete local TON network and full indexer, supports actions, and reproduces full-node API behavior, but starts more slowly and uses more memory and disk space"
  }

  if (config.kind === "remoteTonNetwork") {
    return `This environment connects Studio directly to the public TON ${environment.network.label} without starting a local network`
  }

  if (config.forkNetwork) {
    return "This environment starts quickly, pins a remote TON block, resolves account state as needed, and executes new blocks locally using Acton's simplified implementation"
  }

  return "This environment starts quickly with compatible blocks, LiteAPI, TON Center v2/v3, Streaming API, and Emulate API; it uses Acton's simplified implementation rather than a real TON network"
}

function getEnvironmentNetworkDescription(environment: StudioEnvironment): string {
  const {config} = environment

  if (config.kind === "remoteTonNetwork") {
    return `This environment uses the public TON ${environment.network.label}`
  }

  if (config.kind === "actonSimulatedLocalnet" && config.forkNetwork) {
    const sourceNetwork = environment.network.label.replace(/ fork$/i, "")

    return `This environment pins a ${sourceNetwork} block, resolves account state from ${sourceNetwork} as needed, and executes new blocks locally`
  }

  return "This environment uses an isolated TON state created for it"
}
