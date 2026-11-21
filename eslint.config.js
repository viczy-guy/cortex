import js from "@eslint/js";
import tseslint from "typescript-eslint";

// Hook payloads are untrusted (AGENTS.md): no API that parses a string as markup.
const markup = ["innerHTML", "outerHTML", "insertAdjacentHTML"].map((property) => ({
  property,
  message: "Build DOM with el() from src/shared/dom.ts; set text with textContent.",
}));

export default tseslint.config(
  { ignores: ["dist/", "target/", "src-tauri/"] },
  js.configs.recommended,
  tseslint.configs.recommended,
  { rules: { "no-restricted-properties": ["error", ...markup] } },
  {
    files: ["scripts/**"],
    languageOptions: { globals: { process: "readonly", console: "readonly", Buffer: "readonly" } },
  },
);
