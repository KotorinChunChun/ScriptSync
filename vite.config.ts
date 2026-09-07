import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/release/**", "**/output/**", "**/target/**", "**/tools/asr/venv/**", "**/dev/sample/**"],
    },
  },
});
