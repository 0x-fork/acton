import type {FC} from "react"

import type {TonClient} from "../api/client"
import {ExplorerSearch} from "../components/ExplorerSearch"

import styles from "./ExplorerIndexPage.module.css"

interface ExplorerIndexPageProps {
  readonly client: TonClient
  readonly fillAvailableHeight?: boolean
}

export const ExplorerIndexPage: FC<ExplorerIndexPageProps> = ({
  client,
  fillAvailableHeight = false,
}) => {
  const pageClassName = fillAvailableHeight
    ? `${styles.inputPage} ${styles.inputPageFillAvailableHeight}`
    : styles.inputPage

  return (
    <div className={pageClassName}>
      <div className={styles.centeredInputContainer}>
        <header className={styles.logoSection}>
          <h1 className={styles.logoTitle}>
            <span>Explore</span>
            <span className={styles.logoTitleAccent}>any address</span>
          </h1>
        </header>

        <ExplorerSearch autoFocus client={client} />
      </div>

      <footer className={styles.footer}>
        <span className={styles.footerCredit}>
          <span className={styles.footerBrand}>actonscan</span>
          <span className={styles.footerBy}>by</span>
          <a
            className={styles.footerCreditLink}
            href="https://t.me/toncore"
            target="_blank"
            rel="noreferrer"
          >
            TON Core
          </a>
        </span>
        <a href="https://ton-blockchain.github.io/acton/docs/welcome">Documentation</a>
        <a href="https://github.com/ton-blockchain/acton">GitHub</a>
      </footer>
    </div>
  )
}
