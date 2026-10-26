import { fixtureStore } from "./fixtures";
import type { Store } from "./model";
import { tauriStore } from "./tauri-store";

/**
 * The prerender and a plain browser use fixtures. Nothing prerendered depends
 * on the data (everything reads the URL, so it suspends), so the server and
 * the app choosing differently cannot mismatch.
 */
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const store: Store = inTauri ? tauriStore : fixtureStore;
