import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import inlineSource from "vite-plugin-inline-source";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
    plugins: [react(), inlineSource()],
    server: {
        port: 1420,
        strictPort: true,
    },
    build: {
        minify: false,
        rollupOptions: {
            input: {
                app: 'ui/index.html'
            },
        },
        outDir: 'ui/dist',
    },
}));
