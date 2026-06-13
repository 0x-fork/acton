import type {ReactNode} from "react"
import {Github} from "lucide-react"

import tonVerifierIcon from "../assets/ton-verifier-icons/icon.svg"
import {ThemeToggle} from "./ThemeToggle"

interface AppShellProps {
  readonly children: ReactNode
  readonly headerAccessory?: ReactNode
}

export function AppShell({children, headerAccessory}: AppShellProps) {
  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Acton Verifier home">
          <img className="brand-mark" src={tonVerifierIcon} alt="" aria-hidden="true" />
          <span className="brand-text">Acton Verifier</span>
        </a>
        <div className="topbar-actions">
          {headerAccessory}
          <ThemeToggle />
          <a
            className="topbar-github"
            href="https://github.com/ton-blockchain/acton"
            target="_blank"
            rel="noreferrer"
            aria-label="Open GitHub"
            title="GitHub"
          >
            <Github size={18} strokeWidth={2} aria-hidden="true" />
          </a>
        </div>
      </header>
      {children}
      <footer className="app-footer">
        <div className="footer-brand">
          <img className="brand-mark" src={tonVerifierIcon} alt="" aria-hidden="true" />
          <span>Acton Verifier</span>
        </div>
        <div className="footer-meta">
          <span className="footer-year">2026</span>
          <a
            className="footer-github"
            href="https://github.com/ton-blockchain/acton"
            target="_blank"
            rel="noreferrer"
            aria-label="Open GitHub"
            title="GitHub"
          >
            <Github size={18} strokeWidth={2} aria-hidden="true" />
          </a>
        </div>
      </footer>
    </div>
  )
}
