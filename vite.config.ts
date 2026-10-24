import { defineConfig } from "vite";
import {fileURLToPath} from 'node:url';

export default defineConfig(({mode})=>({
  resolve:{alias:mode==='web'?[]:[{find:/^\.\/web\/backend$/,replacement:fileURLToPath(new URL('./src/web/disabled.ts',import.meta.url))}]},
  clearScreen: false,
  worker: {format:'es'},
  build: {target:'es2022'},
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/release/**", "**/output/**", "**/target/**", "**/tools/asr/venv/**", "**/dev/sample/**"],
    },
  },
}));
