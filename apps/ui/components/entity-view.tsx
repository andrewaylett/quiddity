"use client";

import { Fragment, use, type ReactNode } from "react";
import type { Block, EntityId } from "@/lib/model";
import { store } from "@/lib/store";
import { EntityLink } from "./entity-link";

/** `[[Name]]` becomes a link. Resolution by name arrives with the core. */
function Inline({ text }: { text: string }) {
  const parts: ReactNode[] = [];
  const re = /\[\[([^\]]+)\]\]/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    parts.push(text.slice(last, m.index));
    const name = m[1]!;
    parts.push(
      <EntityLink
        key={m.index}
        id={name.toLowerCase()}
        className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
      >
        {name}
      </EntityLink>,
    );
    last = m.index + m[0].length;
  }
  parts.push(text.slice(last));
  return parts.map((p, i) => <Fragment key={i}>{p}</Fragment>);
}

function Outline({ blocks }: { blocks: Block[] }) {
  return (
    <ul className="space-y-1">
      {blocks.map((block) => (
        <li key={block.id} className="flex gap-2">
          <span aria-hidden className="mt-2.5 size-1.5 shrink-0 rounded-full bg-slate-400" />
          <div className="min-w-0 flex-1 py-0.5">
            <p className="break-words">
              <Inline text={block.text} />
            </p>
            {block.children.length > 0 && (
              <div className="mt-1">
                <Outline blocks={block.children} />
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
  return <Outline blocks={entity.blocks} />;
}

export function EntityTitle({ id }: { id: EntityId }) {
  return <>{use(store.entity(id)).title}</>;
}
