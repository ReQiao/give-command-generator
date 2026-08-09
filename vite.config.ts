import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import obfuscatorPlugin from "vite-plugin-javascript-obfuscator";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    vue(),
    // 只在打生产包（`vite build` / `tauri build`）时混淆，dev 模式不启用——
    // 不然 HMR 和调试体验全废，而且开发阶段本来就不需要混淆。
    // 这只是提高逆向门槛，不是真正的防护：混淆后的 JS 依然会跑在用户机器上，
    // 有决心的人依然能慢慢还原逻辑，只是变慢、变贵。真要保护 AI 模式那条
    // "意图 -> 确定性指令" 的核心逻辑，得挪到服务器端跑，混淆顶多是顺手加固。
    obfuscatorPlugin({
      apply: "build",
      options: {
        compact: true,
        controlFlowFlattening: true,
        controlFlowFlatteningThreshold: 0.5,
        deadCodeInjection: true,
        deadCodeInjectionThreshold: 0.2,
        stringArray: true,
        stringArrayEncoding: ["base64"],
        stringArrayThreshold: 0.75,
        identifierNamesGenerator: "hexadecimal",
        renameGlobals: false,
        selfDefending: true,
        // debugProtection/disableConsoleOutput 故意不开：webview 环境里容易引发
        // 奇怪的卡顿/白屏，而且以后我们自己排查线上问题也需要能看 console。
      },
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
