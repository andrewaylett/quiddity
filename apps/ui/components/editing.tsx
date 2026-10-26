"use client";

import { useMemo, useState } from "react";
import { useGraph } from "@/lib/use-graph";
import type { Block, EntityId, LinkTarget } from "@/lib/model";
import { EditingContext, useEditing, type Editing } from "./editing-context";
import { Outline } from "./entity-view";

function flatten(blocks: Block[], out: string[] = []): string[] {
  for (const block of blocks) {
    out.push(block.id);
    flatten(block.children, out);
  }
  return out;
}

function AddBlock({ empty }: { empty: boolean }) {
  const editing = useEditing()!;
  return (
    <button
      type="button"
      className={`mt-1 block w-full rounded px-2 py-1 text-left text-sm text-slate-500 hover:bg-slate-200 dark:text-slate-400 dark:hover:bg-slate-700 ${
        empty ? "" : "opacity-0 focus:opacity-100 hover:opacity-100"
      }`}
      onClick={() => {
        const id = editing.graph.createBlock(editing.entityId, undefined, "");
        editing.edit(id, "start");
      }}
    >
      {empty ? "Empty. Click to add a block." : "+ Add a block"}
    </button>
  );
}

/** An entity's blocks, which can be edited in place. */
export function EditableOutline({
  entityId,
  blocks,
  links,
}: {
  entityId: EntityId;
  blocks: Block[];
  links: Record<string, LinkTarget>;
}) {
  const graph = useGraph();
  const [current, setCurrent] = useState<Editing["current"]>(null);
  const order = useMemo(() => flatten(blocks), [blocks]);
  const editing = useMemo<Editing>(
    () => ({
      graph,
      entityId,
      current,
      order,
      edit: (id, caret) => {
        graph.settle();
        setCurrent({ id, caret });
      },
      stop: (id) => {
        graph.settle();
        setCurrent((c) => (c?.id === id ? null : c));
      },
    }),
    [graph, entityId, current, order],
  );
  return (
    <EditingContext.Provider value={editing}>
      <Outline blocks={blocks} links={links} />
      <AddBlock empty={blocks.length === 0} />
    </EditingContext.Provider>
  );
}
