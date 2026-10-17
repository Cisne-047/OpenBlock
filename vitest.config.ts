import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["legacy/tests/**/*.test.ts"],
    environment: "node",
    testTimeout: 20000,
  },
});
