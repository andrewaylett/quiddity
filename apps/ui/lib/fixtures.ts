import type {
  Block,
  Entity,
  EntityId,
  EntityRef,
  LinkTarget,
  Reference,
  Store,
} from "./model";

const b = (id: string, text: string, ...children: Block[]): Block => ({
  id,
  text,
  children,
});

type Raw = Omit<Entity, "links">;

const days = Array.from({ length: 30 }, (_, i): Raw => {
  const date = new Date(Date.UTC(2026, 9, 26 - i)).toISOString().slice(0, 10);
  return {
    id: `day-${date}`,
    title: date,
    blocks: [
      b(`${date}-p`, `type:: journal\ndate:: ${date}`),
      b(`${date}-1`, i % 3 === 0 ? "Worked on [[Quiddity]]." : "A quiet day."),
      ...(i % 5 === 0 ? [b(`${date}-2`, "Read about loro again")] : []),
    ],
  };
});

const entities: Raw[] = [
  ...days,
  {
    id: "all",
    title: "All entities",
    blocks: [b("a1", "Every entity in the graph, by name.")],
  },
  {
    id: "quiddity",
    title: "Quiddity",
    blocks: [
      b("q1", "favourite:: true\ntype:: project"),
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
    blocks: [
      b("r1", "Designing Data-Intensive Applications"),
      b("r2", "Notes on loro and other CRDTs"),
    ],
  },
  {
    id: "slow",
    title: "Slow entity",
    blocks: [b("s1", "This one took its time. The others carried on without it.")],
  },
  // "broken" is deliberately absent: opening it exercises the error boundary.
];

export const slowIds = new Set(["slow"]);

const byId = new Map(entities.map((e) => [e.id, e]));
const ref = (id: EntityId): EntityRef => {
  const e = byId.get(id);
  return e ? { id: e.id, title: e.title } : { id, title: id };
};

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

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The blocks (outermost first) of other entities that match, grouped. */
function find(target: EntityId, matches: (text: string) => boolean): Reference[] {
  const groups: Reference[] = [];
  for (const e of entities) {
    if (e.id === target) continue;
    const found: Block[] = [];
    const walk = (blocks: Block[]) => {
      for (const block of blocks) {
        if (matches(block.text)) found.push(block);
        else walk(block.children);
      }
    };
    walk(e.blocks);
    if (found.length) {
      groups.push({ source: ref(e.id), blocks: found, links: linksIn(found) });
    }
  }
  // Recent days first, then everything else by name, as the core does.
  const journal = (r: Reference) => r.source.id.startsWith("day-");
  return groups.sort(
    (a, b) =>
      Number(journal(b)) - Number(journal(a)) ||
      (journal(a) ? b.source.title.localeCompare(a.source.title) : a.source.title.localeCompare(b.source.title)),
  );
}

const delay = <T>(value: T, ms: number) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

const once = <T>(f: () => Promise<T>): (() => Promise<T>) => {
  let p: Promise<T> | undefined;
  return () => (p ??= f());
};

const cached = <T>(f: (id: EntityId) => Promise<T>) => {
  const cache = new Map<EntityId, Promise<T>>();
  return (id: EntityId) => {
    let p = cache.get(id);
    if (!p) cache.set(id, (p = f(id)));
    return p;
  };
};

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
  journals: once(() => delay(days.map((d) => ref(d.id)), 100)),
  entity: cached((id) =>
    delay(null, slowIds.has(id) ? 3000 : 80).then(() => {
      const e = byId.get(id);
      if (!e) throw new Error(`No entity ${id}`);
      return { ...e, links: linksIn(e.blocks) };
    }),
  ),
  references: cached((id) => {
    const title = ref(id).title.toLowerCase();
    return delay(
      find(id, (t) => t.toLowerCase().includes(`[[${title}]]`)),
      120,
    );
  }),
  unlinkedReferences: cached((id) => {
    const word = new RegExp(`(?<![\\w])${escape(ref(id).title)}(?![\\w])`, "i");
    return delay(
      find(id, (t) => word.test(t.replace(/\[\[[^\]]*\]\]/g, " ")) && !/^\w+::/.test(t)),
      120,
    );
  }),
};
