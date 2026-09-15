import {createRoot} from "react-dom/client"

import {AppShell} from "../components/AppShell"
import {createVerifierApi} from "../lib/api"
import {clearLegacyVerifierStorage} from "../lib/legacy-storage"
import {StatisticsPage} from "../pages/StatisticsPage"
import "../global.css"

clearLegacyVerifierStorage()

const api = createVerifierApi()

function StatisticsEntry() {
  return (
    <AppShell>
      <StatisticsPage api={api} />
    </AppShell>
  )
}

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(<StatisticsEntry />)
