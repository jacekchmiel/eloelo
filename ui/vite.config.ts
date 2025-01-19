import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import inlineSource from "vite-plugin-inline-source";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
    plugins: [react(), inlineSource()],

    // clearScreen: false,
    server: {
        port: 1420,
        strictPort: true,
        // host: host || false,
        // hmr: host
        //     ? {
        //         protocol: "ws",
        //         host,
        //         port: 1421,
        //     }
        //     : undefined,
    },
    build: {
        minify: false
    }
}));
