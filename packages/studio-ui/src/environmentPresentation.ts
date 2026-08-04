import type {EnvironmentConfig, EnvironmentStatus} from "./studioApi"

export const environmentStatusLabels = {
  starting: "Starting",
  running: "Running",
  stopping: "Stopping",
  stopped: "Stopped",
  failed: "Failed",
} satisfies Record<EnvironmentStatus, string>

export function formatEnvironmentType(config: EnvironmentConfig) {
  if (config.kind === "actonLocalnet") return "Simulated localnet"
  if (config.kind === "fullTonNetwork") return "Full localnet"
  return config.network === "mainnet" ? "Mainnet" : "Testnet"
}
