import { createContext } from "react";

import type { AppStoreValue } from "./types";

/** Isolated so HMR of AppStore.tsx does not recreate the context identity. */
export const AppStoreContext = createContext<AppStoreValue | null>(null);
