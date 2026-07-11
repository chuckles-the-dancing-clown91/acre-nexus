// ESLint 9 flat config. As of Next 16, `eslint-config-next` ships native flat
// configs, so we spread them directly (the old `FlatCompat` bridge is no longer
// needed and is incompatible with the new flat presets).
import nextCoreWebVitals from "eslint-config-next/core-web-vitals";
import nextTypescript from "eslint-config-next/typescript";

const eslintConfig = [
  ...nextCoreWebVitals,
  ...nextTypescript,
  {
    // Next 16 bundles the React-Compiler-aware react-hooks plugin, whose new
    // `set-state-in-effect` rule flags a pattern used throughout this codebase
    // (initialising client state from localStorage / fetches inside an effect).
    // Keep it as a visible warning and adopt the refactor incrementally rather
    // than churn every page in the framework-upgrade PR.
    rules: {
      "react-hooks/set-state-in-effect": "warn",
    },
  },
  {
    ignores: [
      ".next/**",
      "node_modules/**",
      "out/**",
      "build/**",
      "coverage/**",
      "playwright-report/**",
      "test-results/**",
      "next-env.d.ts",
    ],
  },
];

export default eslintConfig;
