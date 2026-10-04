import js from "@eslint/js";
import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import * as reactHooks from "eslint-plugin-react-hooks";
import * as importX from "eslint-plugin-import-x";
import jsxA11y from "eslint-plugin-jsx-a11y";
import globals from "globals";

export default tseslint.config(
  {
    ignores: ["dist/**", "src/types/api.ts"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  react.configs.flat.recommended,
  react.configs.flat["jsx-runtime"],
  jsxA11y.flatConfigs.recommended,
  importX.configs["flat/recommended"],
  importX.configs["flat/typescript"],
  {
    settings: {
      react: { version: "detect" },
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      globals: { ...globals.browser, ...globals.es2021 },
    },
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs["recommended-latest"].rules,
      // TypeScript already reports undefined identifiers with full type
      // information; the core rule only sees syntax and false-positives on
      // ambient/global types.
      "no-undef": "off",
      // The generated OpenAPI client covers prop shapes; this rule is
      // redundant noise on every typed component.
      "react/prop-types": "off",
      // tsc's module resolution is the source of truth; this rule can't see
      // path aliases or bundler-only resolution the way tsc/vite do.
      "import-x/no-unresolved": "off",
      // False positives on packages that export the same function both ways
      // (`clsx`, `react-dom/client`); tsc already rejects a wrong import.
      "import-x/no-named-as-default": "off",
      "import-x/no-named-as-default-member": "off",
    },
  },
  {
    files: ["*.config.{js,ts}"],
    languageOptions: {
      globals: { ...globals.node },
    },
    rules: {
      // Same false positive as above (`tseslint.configs` vs the named export).
      "import-x/no-named-as-default-member": "off",
    },
  },
);
