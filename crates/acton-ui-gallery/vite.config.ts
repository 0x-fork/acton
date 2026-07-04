import path from "node:path"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@acton/ui": path.resolve(import.meta.dirname, "../acton-ui/src"),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3008,
  },
})
