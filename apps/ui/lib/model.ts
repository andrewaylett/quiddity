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

/** Where a `[[link]]` goes. */
export type LinkTarget =
  | { kind: "entity"; id: EntityId }
  | { kind: "ambiguous"; ids: EntityId[] }
  | { kind: "missing" };

export interface Entity extends EntityRef {
  blocks: Block[];
  /** Keyed by the text between a link's brackets. */
  links: Record<string, LinkTarget>;
}

/**
 * What the UI needs from the core. The Tauri shell implements it over IPC;
 * fixtures stand in when the UI runs in a plain browser. Promises must be
 * stable per argument so they can be read with `use()`.
 */
export interface Store {
  /** Entities whose `favourite` property is true. */
  favourites(): Promise<EntityRef[]>;
  /** Every entity, by name. */
  entities(): Promise<EntityRef[]>;
  /** Today's journal page. */
  today(): Promise<EntityRef>;
  entity(id: EntityId): Promise<Entity>;
}
