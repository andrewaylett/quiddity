export type EntityId = string;

export interface EntityRef {
  id: EntityId;
  title: string;
}

export interface Block {
  id: string;
  text: string;
  /** The properties written in `text`, in order, for rendering as chips. */
  properties: Property[];
  children: Block[];
}

/** A property's value, as written. */
export type PropertyValue =
  | { kind: "text"; text: string }
  /** `key` is how the link is written between its brackets: a key of `links`. */
  | { kind: "link"; name: string; key: string }
  | { kind: "ref"; id: string }
  | { kind: "list"; items: PropertyValue[] };

/** One property written in a block's text. */
export interface Property {
  key: string;
  value: PropertyValue | null;
  /** `#key(value)` within the text, rather than `key:: value` on a line. */
  inline: boolean;
  /** Where it is written, in UTF-16 units of the block's text. For a line
   * property, the whole line. */
  start: number;
  end: number;
  /** The entity that defines the property, if anyone has. */
  definition: EntityId | null;
}

/** Where a `[[link]]` goes. */
export type LinkTarget =
  | { kind: "entity"; id: EntityId }
  | { kind: "ambiguous"; ids: EntityId[] }
  | { kind: "missing" };

/** Every entity, and which link to which: one edge per pair, however many links. */
export interface Connections {
  nodes: EntityRef[];
  edges: { source: EntityId; target: EntityId }[];
}

export interface Entity extends EntityRef {
  /** False for journals and property entities: their date or name is what
   * identifies them. */
  renamable: boolean;
  blocks: Block[];
  /** Keyed by the text between a link's brackets. */
  links: Record<string, LinkTarget>;
}

/** What typing Backspace at the start of a block did: joined it to another. */
export interface Merged {
  /** The block that took the text. */
  into: string;
  /** Where the joined text begins in it, in UTF-16 units: the caret. */
  at: number;
}

/** The blocks of one entity that mention another. */
export interface Reference {
  source: EntityRef;
  /** Each with its children, for context. */
  blocks: Block[];
  links: Record<string, LinkTarget>;
}
