import {useEffect, useMemo} from "react"
import {
  Activity,
  Boxes,
  Clock3,
  Gauge,
  Network,
  RadioTower,
  RefreshCw,
  ShieldCheck,
} from "lucide-react"
import {InlineLoader, TechnicalValue, ThemeSwitch} from "@acton/ui"

import {NetworkDashboardContent} from "./NetworkDashboard"
import {createObservabilityClient, useLocalNodeName, useObservability} from "./observability"
import styles from "./App.module.css"

export function App() {
  const client = useMemo(() => createObservabilityClient(), [])
  const nodeName = useLocalNodeName(client)
  const {network, now, tps} = useObservability(client)

  useEffect(() => {
    document.title = nodeName ? `Network health · ${nodeName}` : "Localton Network"
  }, [nodeName])

  if (!network) {
    return (
      <main className={styles.bootState}>
        <InlineLoader
          message="Reading network state"
          subtext="Waiting for the local observability service"
        />
      </main>
    )
  }

  return (
    <div className={styles.appShell}>
      <aside className={styles.sidebar}>
        <a className={styles.brand} href="#overview" aria-label="Localton network overview">
          <span className={styles.brandMark} aria-hidden="true">
            <Network size={17} strokeWidth={1.8} />
          </span>
          <span>Localton</span>
        </a>
        <nav className={styles.navigation} aria-label="Network sections">
          <NavigationLink href="#overview" icon={<Gauge size={15} />} label="Overview" />
          <NavigationLink href="#throughput" icon={<Activity size={15} />} label="Throughput" />
          <NavigationLink href="#elections" icon={<Clock3 size={15} />} label="Elections" />
          <NavigationLink href="#nodes" icon={<RadioTower size={15} />} label="Nodes" />
          <NavigationLink href="#validators" icon={<ShieldCheck size={15} />} label="Validators" />
          <NavigationLink href="#shards" icon={<Boxes size={15} />} label="Shards" />
        </nav>
        <div className={styles.sidebarFooter}>
          <ThemeSwitch />
        </div>
      </aside>

      <div className={styles.workspace}>
        <header className={styles.topbar}>
          <div className={styles.networkTitle}>
            <h1>Network health</h1>
            <TechnicalValue value={network.network_id} copyLabel="network ID" />
          </div>
          <div className={styles.refreshState}>
            <RefreshCw size={13} aria-hidden="true" />
            {`Updated ${Math.max(0, now - network.generated_at)}s ago`}
          </div>
        </header>

        <main className={styles.content}>
          <NetworkDashboardContent network={network} now={now} tps={tps} />
        </main>
      </div>
    </div>
  )
}

function NavigationLink({
  href,
  icon,
  label,
}: {
  readonly href: string
  readonly icon: React.ReactNode
  readonly label: string
}) {
  return (
    <a href={href}>
      {icon}
      <span>{label}</span>
    </a>
  )
}
