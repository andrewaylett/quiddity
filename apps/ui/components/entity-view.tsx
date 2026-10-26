"use client";

import { Fragment, use, type ReactNode } from "react";
import type { Block, EntityId, LinkTarget } from "@/lib/model";
import { store } from "@/lib/store";
import { EntityLink } from "./entity-link";

const unresolved = "cursor-help underline decoration-dashed underline-offset-2";

/** `[[Name]]` becomes a link, as the core resolved it. */
function Inline({ text, links }: { text: string; links: Record<string, LinkTarget> }) {
  const parts: ReactNode[] = [];
  const re = /\[\[([^\]]+)\]\]/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    parts.push(text.slice(last, m.index));
    const name = m[1]!;
    const target = links[name] ?? { kind: "missing" };
    parts.push(
      target.kind === "entity" ? (
        <EntityLink
          key={m.index}
          id={target.id}
          className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
        >
          {name}
        </EntityLink>
      ) : (
        <span
          key={m.index}
          className={`text-slate-500 dark:text-slate-400 ${unresolved}`}
          title={target.kind === "ambiguous" ? "Several entities match" : "No such entity"}
        >
          {name}
        </span>
      ),
    );
    last = m.index + m[0].length;
  }
  parts.push(text.slice(last));
  return parts.map((p, i) => <Fragment key={i}>{p}</Fragment>);
}

export function Outline({ blocks, links }: { blocks: Block[]; links: Record<string, LinkTarget> }) {
  return (
    <ul className="space-y-1">
      {blocks.map((block) => (
        <li key={block.id} className="flex gap-2">
          <span aria-hidden className="mt-2.5 size-1.5 shrink-0 rounded-full bg-slate-400" />
          <div className="min-w-0 flex-1 py-0.5">
            <p className="whitespace-pre-line break-words">
              <Inline text={block.text} links={links} />
            </p>
            {block.children.length > 0 && (
              <div className="mt-1">
                <Outline blocks={block.children} links={links} />
              </div>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}

/** Suspends until the entity loads; throws to the nearest error boundary. */
export function EntityBody({ id }: { id: EntityId }) {
  const entity = use(store.entity(id));
  return <Outline blocks={entity.blocks} links={entity.links} />;
}

export function EntityTitle({ id }: { id: EntityId }) {
  return <>{use(store.entity(id)).title}</>;
}
