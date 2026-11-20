import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src-tauri", "plugins", "src/lib/bindings.ts"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  reactHooks.configs.flat["recommended-latest"],
  {
    languageOptions: { globals: { ...globals.browser } },
    rules: {
      // These three come from the React Compiler, which Melomango doesn't use. They flag
      // patterns used on purpose: refs holding the latest value (written while rendering),
      // <audio> elements kept in state and changed directly, and "loadingâ€¦" set when an
      // effect starts loading. Kept visible as warnings rather than blocking.
      "react-hooks/refs": "warn",
      "react-hooks/immutability": "warn",
      "react-hooks/set-state-in-effect": "warn",
    },
  },
  {
    files: ["tests/**/*.ts", "scripts/**/*.mjs", "*.config.*"],
    languageOptions: { globals: { ...globals.node } },
  },
);
