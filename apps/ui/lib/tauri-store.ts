import { invoke } from "@tauri-apps/api/core";
import type { Entity, EntityId, EntityRef, Store } from "./model";

const once = <T>(f: () => Promise<T>): (() => Promise<T>) => {
  let p: Promise<T> | undefined;
  return () => (p ??= f());
};

const entityCache = new Map<EntityId, Promise<Entity>>();

/** The core, over Tauri commands. Results are cached until edits exist. */
export const tauriStore: Store = {
  favourites: once(() => invoke<EntityRef[]>("favourites")),
  entities: once(() => invoke<EntityRef[]>("entities")),
  today: once(() => invoke<EntityRef>("today")),
  entity(id) {
    let p = entityCache.get(id);
    if (!p) {
      p = invoke<Entity>("entity", { id });
      entityCache.set(id, p);
    }
    return p;
  },
};
