"use client";

import { Fragment, useContext, type ReactNode } from "react";
import type { Block, EntityId, LinkTarget } from "@/lib/model";
import { useRead } from "@/lib/use-graph";
import { BlockEditor } from "./block-editor";
import { EditableOutline } from "./editing";
import { EditingContext } from "./editing-context";
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

/** One block: its text, rendered or being edited, then its children. */
function BlockRow({ block, links }: { block: Block; links: Record<string, LinkTarget> }) {
  const editing = useContext(EditingContext);
  const current = editing?.current;
  const isEditing = current?.id === block.id;
  return (
    <li className="flex gap-2">
      <span aria-hidden className="mt-2.5 size-1.5 shrink-0 rounded-full bg-slate-400" />
      <div className="min-w-0 flex-1 py-0.5">
        {isEditing ? (
          <BlockEditor id={block.id} caret={current.caret} />
        ) : (
          <p
            className={`min-h-[1.5em] whitespace-pre-line break-words ${
              editing ? "cursor-text" : ""
            }`}
            tabIndex={editing ? 0 : undefined}
            onClick={(e) => {
              if (editing && !(e.target as HTMLElement).closest("a")) {
                editing.edit(block.id, "end");
              }
            }}
            onKeyDown={(e) => {
              if (editing && e.target === e.currentTarget && e.key === "Enter") {
                e.preventDefault();
                editing.edit(block.id, "end");
              }
            }}
          >
            <Inline text={block.text} links={links} />
          </p>
        )}
        {block.children.length > 0 && (
          <div className="mt-1">
            <Outline blocks={block.children} links={links} />
          </div>
        )}
      </div>
    </li>
  );
}

/** Blocks as an outline. Editable if inside an `EditableOutline`. */
export function Outline({
  blocks,
  links,
}: {
  blocks: Block[];
  links: Record<string, LinkTarget>;
}) {
  return (
    <ul className="space-y-1">
      {blocks.map((block) => (
        <BlockRow key={block.id} block={block} links={links} />
      ))}
    </ul>
  );
}

/** An entity's blocks, editable in place. Throws if there is no such entity. */
export function EntityBody({ id }: { id: EntityId }) {
  const entity = useRead((g) => g.entity(id));
  return <EditableOutline entityId={id} blocks={entity.blocks} links={entity.links} />;
}

export function EntityTitle({ id }: { id: EntityId }) {
  return <>{useRead((g) => g.entity(id)).title}</>;
}
