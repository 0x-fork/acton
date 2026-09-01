import styles from "./StatusPill.module.css"

interface StatusPillProps {
  readonly online: boolean
}

/** Presents observer availability consistently in node and observer tables */
export function StatusPill({online}: StatusPillProps) {
  return (
    <span className={styles.statusPill} data-online={online ? "true" : "false"}>
      <span aria-hidden="true" />
      {online ? "Online" : "Offline"}
    </span>
  )
}
