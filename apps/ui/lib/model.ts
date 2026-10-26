export type EntityId = string;

export interface EntityRef {
  id: EntityId;
  title: string;
}

export interface Block {
  id: string;
  text: string;
  children: Block[];
}

export interface Entity extends EntityRef {
  blocks: Block[];
}

/**
 * What the UI needs from the core. Fixtures implement it today; Tauri IPC and
 * the WASM replica will implement it later. Promises must be stable per
 * argument so they can be read with `use()`.
 */
export interface Store {
  entrypoints(): Promise<EntityRef[]>;
  favourites(): Promise<EntityRef[]>;
  entity(id: EntityId): Promise<Entity>;
}
