import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import vercel from "@astrojs/vercel";

const midnight = {
  name: "midnight",
  type: "dark",
  colors: {
    "editor.background": "#121212",
    "editor.foreground": "#e0e0e0",
  },
  tokenColors: [
    {
      scope: [
        "storage.type",
        "storage.modifier",
        "keyword.control",
        "keyword.operator.new",
      ],
      settings: { foreground: "#7aa2f7" },
    },
    {
      scope: [
        "string",
        "constant",
        "constant.numeric",
        "constant.language",
        "constant.character",
        "number",
      ],
      settings: { foreground: "#98c379" },
    },
  ],
};

const daylight = {
  name: "daylight",
  type: "light",
  colors: {
    "editor.background": "#f5f5f5",
    "editor.foreground": "#1a1a1a",
  },
  tokenColors: [
    {
      scope: [
        "storage.type",
        "storage.modifier",
        "keyword.control",
        "keyword.operator.new",
      ],
      settings: { foreground: "#3b5bdb" },
    },
    {
      scope: [
        "string",
        "constant",
        "constant.numeric",
        "constant.language",
        "constant.character",
        "number",
      ],
      settings: { foreground: "#2d7f3e" },
    },
  ],
};

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
