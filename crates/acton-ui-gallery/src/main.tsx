import {StrictMode} from "react"
import {createRoot} from "react-dom/client"

import "@acton/ui/styles/tokens.css"
import {App} from "./App"
import "./main.css"

const root = document.getElementById("root")

if (!root) {
  throw new Error("Root element was not found")
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
