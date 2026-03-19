export const midnight = {
  name: "midnight",
  type: "dark",
  colors: {
    "editor.background": "#222222",
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
        "string.quoted",
        "constant.numeric",
        "constant.language",
        "constant.character",
        "number",
      ],
      settings: { foreground: "#98c379" },
    },
  ],
};

export const daylight = {
  name: "daylight",
  type: "light",
  colors: {
    "editor.background": "#ebebeb",
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
        "string.quoted",
        "constant.numeric",
        "constant.language",
        "constant.character",
        "number",
      ],
      settings: { foreground: "#2d7f3e" },
    },
  ],
};
