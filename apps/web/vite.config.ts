import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const apiProxyTarget = process.env.GARMIN_FIT_API_PROXY ?? "http://127.0.0.1:3000";

export default defineConfig({
  plugins: [tanstackRouter({ target: "react", autoCodeSplitting: true }), react()],
  server: {
    allowedHosts: ["garmin2.codesook.dev"],
    proxy: {
      "/api": apiProxyTarget,
      "/healthz": apiProxyTarget,
    },
  },
});
