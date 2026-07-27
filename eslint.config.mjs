import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTypeScript from "eslint-config-next/typescript";

export default defineConfig([
  ...nextVitals,
  ...nextTypeScript,
  {
    rules: {
      // Catalog artwork comes from an operator-configured API at runtime, so
      // its remote hosts cannot be enumerated in next.config.ts.
      "@next/next/no-img-element": "off",
    },
  },
  globalIgnores([".next/**", ".open-next/**", "api/build/**", "api/target/**", "cloudflare-env.d.ts"]),
]);
