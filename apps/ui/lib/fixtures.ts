import type { Block, Entity, EntityId, EntityRef, LinkTarget, Store } from "./model";

const b = (id: string, text: string, ...children: Block[]): Block => ({
  id,
  text,
  children,
});

const entities: Omit<Entity, "links">[] = [
  {
    id: "journals",
    title: "Journals",
    blocks: [b("j1", "Today's page, then scroll-back.")],
  },
  {
    id: "all",
    title: "All entities",
    blocks: [b("a1", "Every entity in the graph, by name.")],
  },
  {
    id: "quiddity",
    title: "Quiddity",
    blocks: [
      b("q1", "type:: project"),
      b(
        "q2",
        "Local-first, multiplayer second brain.",
        b("q3", "Built on [[Loro]] and [[iroh]]."),
        b("q4", "Shift-click a link to open it at the side."),
      ),
      b("q5", "Open questions", b("q6", "Eviction of unsynced edits.")),
    ],
  },
  {
    id: "loro",
    title: "Loro",
    blocks: [b("l1", "A CRDT library. Documents are movable trees of text.")],
  },
  {
    id: "iroh",
    title: "iroh",
    blocks: [b("i1", "QUIC with hole punching, addressed by public key.")],
  },
  {
    id: "reading",
    title: "Reading list",
    blocks: [b("r1", "Designing Data-Intensive Applications")],
  },
  {
    id: "slow",
    title: "Slow entity",
    blocks: [b("s1", "This one took its time. The others carried on without it.")],
  },
  // "broken" is deliberately absent: opening it exercises the error boundary.
];

/** `[[Name]]` resolves by lowercased name, as the fixtures' ids are. */
function linksIn(blocks: Block[], links: Record<string, LinkTarget> = {}) {
  for (const block of blocks) {
    for (const m of block.text.matchAll(/\[\[([^\]]+)\]\]/g)) {
      const name = m[1]!;
      const id = name.toLowerCase();
      links[name] = byId.has(id) ? { kind: "entity", id } : { kind: "missing" };
    }
    linksIn(block.children, links);
  }
  return links;
}

export const slowIds = new Set(["slow"]);

const byId = new Map(entities.map((e) => [e.id, e]));
const ref = (id: EntityId): EntityRef => {
  const e = byId.get(id);
  return e ? { id: e.id, title: e.title } : { id, title: id };
};

const delay = <T>(value: T, ms: number) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

const cache = new Map<EntityId, Promise<Entity>>();

export const fixtureStore: Store = {
  favourites: once(() =>
    delay([ref("quiddity"), ref("reading"), ref("slow"), ref("broken")], 100),
  ),
  entities: once(() =>
    delay(
      entities.map((e) => ref(e.id)).sort((a, b) => a.title.localeCompare(b.title)),
      100,
    ),
  ),
  today: once(() => delay(ref("journals"), 100)),
  entity(id) {
    let p = cache.get(id);
    if (!p) {
      const e = byId.get(id);
      p = delay(null, slowIds.has(id) ? 3000 : 80).then(() => {
        if (!e) throw new Error(`No entity ${id}`);
        return { ...e, links: linksIn(e.blocks) };
      });
      cache.set(id, p);
    }
    return p;
  },
};

function once<T>(f: () => Promise<T>): () => Promise<T> {
  let p: Promise<T> | undefined;
  return () => (p ??= f());
}
