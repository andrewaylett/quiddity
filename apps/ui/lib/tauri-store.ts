import { invoke } from "@tauri-apps/api/core";
import type { Entity, EntityId, EntityRef, Reference, Store } from "./model";

const once = <T>(f: () => Promise<T>): (() => Promise<T>) => {
  let p: Promise<T> | undefined;
  return () => (p ??= f());
};

/** One cached promise per id, so `use()` sees the same one each render. */
const byId = <T>(command: string) => {
  const cache = new Map<EntityId, Promise<T>>();
  return (id: EntityId) => {
    let p = cache.get(id);
    if (!p) {
      p = invoke<T>(command, { id });
      cache.set(id, p);
    }
    return p;
  };
};

/** The core, over Tauri commands. Results are cached until edits exist. */
export const tauriStore: Store = {
  favourites: once(() => invoke<EntityRef[]>("favourites")),
  entities: once(() => invoke<EntityRef[]>("entities")),
  journals: once(() => invoke<EntityRef[]>("journals")),
  entity: byId<Entity>("entity"),
  references: byId<Reference[]>("references"),
  unlinkedReferences: byId<Reference[]>("unlinked_references"),
};
