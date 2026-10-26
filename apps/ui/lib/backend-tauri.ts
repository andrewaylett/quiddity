import { invoke } from "@tauri-apps/api/core";
import type { Backend } from "./graph";

/** The shell holds the graph on disk; bytes cross as raw IPC bodies. */
export const tauriBackend: Backend = {
  async load() {
    return new Uint8Array(await invoke<ArrayBuffer>("load_graph"));
  },
  async push(updates) {
    await invoke("push_updates", updates);
  },
};
