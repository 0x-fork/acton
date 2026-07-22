import type {ReactNode} from "react"
import {Github} from "lucide-react"

import tonVerifierIcon from "../assets/ton-verifier-icons/icon.svg"
import {SearchBox} from "./SearchBox"
import {ThemeToggle} from "./ThemeToggle"
import styles from "./AppShell.module.css"

interface AppShellProps {
  readonly children: ReactNode
  readonly headerAccessory?: ReactNode
}

export function AppShell({children, headerAccessory}: AppShellProps) {
  const pathname = window.location.pathname
  const isHomePage = pathname === "/"
  const headerClassName = isHomePage ? `${styles.header} ${styles.headerHome}` : styles.header

  return (
    <div className={styles.appShell}>
      <header className={headerClassName}>
        <div className={styles.headerInner}>
          <div className={styles.headerPrimary}>
            <a className={styles.brand} href="/" aria-label="TON Verifier home">
              <img className={styles.brandIcon} src={tonVerifierIcon} alt="" aria-hidden="true" />
              <span>TON Verifier</span>
            </a>
            <nav className={styles.nav} aria-label="TON Verifier navigation">
              <a
                className={`${styles.navLink} ${
                  pathname === "/verified" ? styles.navLinkActive : ""
                }`}
                href="/verified"
              >
                Verified contracts
              </a>
            </nav>
          </div>
          {!isHomePage && (
            <div className={styles.headerSearch}>
              {headerAccessory ?? <SearchBox variant="header" />}
            </div>
          )}
          <div className={styles.headerActions}>
            <ThemeToggle />
            <a
              className={styles.githubButton}
              href="https://github.com/i582/verifier"
              target="_blank"
              rel="noreferrer"
              aria-label="Open GitHub"
              title="GitHub"
            >
              <Github size={18} strokeWidth={2} aria-hidden="true" />
            </a>
          </div>
        </div>
      </header>
      <main className={styles.main}>{children}</main>
    </div>
  )
}
