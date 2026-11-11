import { defineConfig } from "cf/config";

export default defineConfig({
  accountId: "b0fe73f3c73291cb334d667822142ba8",
  worker: {
    name: "scripturec",
    compatibilityDate: "2026-11-10",
    observability: {
      enabled: true,
      traces: { enabled: true },
    },
  },
});
