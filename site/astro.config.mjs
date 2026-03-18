import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import vercel from "@astrojs/vercel";
import { midnight, daylight } from "./src/themes.mjs";

export default defineConfig({
  output: "static",
  adapter: vercel(),
  build: {
    format: "file",
  },
  integrations: [mdx()],
  markdown: {
    shikiConfig: {
      themes: {
        light: daylight,
        dark: midnight,
      },
      wrap: true,
    },
  },
});
