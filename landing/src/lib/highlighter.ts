import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import type { ThemeRegistrationRaw } from "shiki";
import langRust from "shiki/langs/rust.mjs";
import langDiff from "shiki/langs/diff.mjs";

/** Brand-tuned Shiki theme: greens / teals / cyans on a transparent surface. */
const evenTheme: ThemeRegistrationRaw = {
  name: "even",
  type: "dark",
  fg: "#c9ddd6",
  bg: "#03121f",
  colors: {
    "editor.background": "#00000000",
    "editor.foreground": "#c9ddd6",
  },
  settings: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#5e7976", fontStyle: "italic" } },
    { scope: ["keyword", "storage", "storage.type", "storage.modifier", "keyword.control"], settings: { foreground: "#47ff9c" } },
    { scope: ["string", "string.quoted", "punctuation.definition.string"], settings: { foreground: "#74e0ff" } },
    { scope: ["constant.numeric", "constant.language", "constant.character"], settings: { foreground: "#ffc266" } },
    { scope: ["entity.name.function", "support.function"], settings: { foreground: "#a6f0c6" } },
    { scope: ["entity.name.type", "support.type", "entity.name.class", "storage.type.core.rust"], settings: { foreground: "#9be3ff" } },
    { scope: ["variable", "meta.definition.variable", "variable.other"], settings: { foreground: "#c9ddd6" } },
    { scope: ["punctuation", "meta.brace", "keyword.operator"], settings: { foreground: "#7e9b95" } },
    { scope: ["markup.inserted", "punctuation.definition.inserted"], settings: { foreground: "#47ff9c" } },
    { scope: ["markup.deleted", "punctuation.definition.deleted"], settings: { foreground: "#ff7a6b" } },
    { scope: ["meta.diff.range", "punctuation.definition.range.diff"], settings: { foreground: "#9db6b1" } },
  ],
};

let instance: Promise<HighlighterCore> | null = null;

export function getHighlighter() {
  if (!instance) {
    instance = createHighlighterCore({
      themes: [evenTheme],
      langs: [langRust, langDiff],
      engine: createJavaScriptRegexEngine(),
    });
  }
  return instance;
}

export const THEME = "even";
