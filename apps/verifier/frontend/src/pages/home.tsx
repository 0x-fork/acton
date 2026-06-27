import {createRoot} from "react-dom/client"

import {AppShell} from "../components/AppShell"
import {SearchBox} from "../components/SearchBox"
import "../styles.css"

function HomePage() {
  return (
    <AppShell>
      <main className="home-page">
        <section className="home-panel" aria-labelledby="home-title">
          <div className="home-kicker">TON source registry</div>
          <h1 id="home-title">Find verified contract sources</h1>
          <p className="home-copy">
            Search by contract address or code hash. The verifier checks the source registry and
            returns the stored source bundle when the code hash is verified.
          </p>
          <SearchBox />
        </section>
      </main>
    </AppShell>
  )
}

createRoot(document.getElementById("root")!).render(<HomePage />)
