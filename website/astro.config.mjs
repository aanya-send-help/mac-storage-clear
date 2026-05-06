import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";

export default defineConfig({
  site: "https://mac-storage-clear.flek.ai",
  integrations: [tailwind()],
  trailingSlash: "ignore",
  build: {
    format: "directory",
  },
});
