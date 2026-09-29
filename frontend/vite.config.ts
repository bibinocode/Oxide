import { defineConfig } from "vite";

import { tanstackStart } from "@tanstack/react-start/plugin/vite";

import viteReact from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { nitro } from "nitro/vite";

const apiTarget = process.env.API_INTERNAL_URL ?? "http://127.0.0.1:3001";

const config = defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [
    nitro({
      rollupConfig: { external: [/^@sentry\//] },
      devProxy: {
        "/api/**": apiTarget,
        "/feed.xml": apiTarget,
        "/media/**": apiTarget,
        "/openapi.json": apiTarget,
        "/docs": apiTarget,
        "/docs/**": apiTarget,
      },
      routeRules: {
        "/api/**": { proxy: `${apiTarget}/api/**` },
        "/feed.xml": { proxy: `${apiTarget}/feed.xml` },
        "/media/**": { proxy: `${apiTarget}/media/**` },
        "/openapi.json": { proxy: `${apiTarget}/openapi.json` },
        "/docs": { proxy: `${apiTarget}/docs` },
        "/docs/**": { proxy: `${apiTarget}/docs/**` },
      },
    }),
    tailwindcss(),
    tanstackStart(),
    viteReact(),
  ],
  server: {
    host: "127.0.0.1",
    proxy: {
      "/api": apiTarget,
      "/feed.xml": apiTarget,
      "/media": apiTarget,
      "/openapi.json": apiTarget,
      "/docs": apiTarget,
    },
  },
});

export default config;
