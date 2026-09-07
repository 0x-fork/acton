export const adminOperationPhases: Record<string, string> = {
  preparing: "Preparing operation",
  stopping: "Stopping network",
  backingUp: "Saving recovery snapshots",
  suspending: "Suspending validators",
  building: "Building hardfork",
  installing: "Installing hardfork",
  verifying: "Verifying state on every node",
  resuming: "Checking block production",
  indexing: "Waiting for the indexer",
  restoring: "Restoring previous state",
  completed: "Changes applied",
  failed: "Operation failed",
}

const adminOperationSteps: readonly string[] = [
  "preparing",
  "stopping",
  "backingUp",
  "suspending",
  "building",
  "installing",
  "verifying",
  "resuming",
  "indexing",
  "restoring",
]

/** Adds stable progress numbering to active administrative operation phases */
export function formatAdminOperationProgress(phase: string) {
  const label = adminOperationPhases[phase] ?? phase
  const index = adminOperationSteps.indexOf(phase)

  return index >= 0 ? `[${index + 1}/${adminOperationSteps.length}] ${label}` : label
}
