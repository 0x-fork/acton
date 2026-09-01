import styles from "./Metric.module.css"

interface MetricProps {
  readonly density?: "default" | "compact"
  readonly label: string
  readonly value: string
  readonly tone?: "default" | "good" | "warning"
}

/** Renders one value in the shared summary-strip format used across the dashboard */
export function Metric({density = "default", label, value, tone = "default"}: MetricProps) {
  return (
    <div className={styles.metric} data-density={density}>
      <span>{label}</span>
      <strong data-tone={tone}>{value}</strong>
    </div>
  )
}
