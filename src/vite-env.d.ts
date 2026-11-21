/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set only for local test builds: `VITE_CORTEX_COMMIT=<commit> npm run bundle`. */
  readonly VITE_CORTEX_COMMIT?: string;
}
