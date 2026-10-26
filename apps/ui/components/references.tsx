"use client";

import { useState } from "react";
import type { EntityId, Reference } from "@/lib/model";
import { useRead } from "@/lib/use-graph";
import { Region, Skeleton } from "./boundary";
import { EntityLink } from "./entity-link";
import { Outline } from "./entity-view";

const heading =
  "text-xs font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400";

function Groups({ references }: { references: Reference[] }) {
  return (
    <ul className="space-y-4">
      {references.map(({ source, blocks, links }) => (
        <li key={source.id}>
          <h3 className="mb-1 font-medium">
            <EntityLink
              id={source.id}
              className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
            >
              {source.title}
            </EntityLink>
          </h3>
          <Outline blocks={blocks} links={links} />
        </li>
      ))}
    </ul>
  );
}

const count = (references: Reference[]) =>
  references.reduce((n, r) => n + r.blocks.length, 0);

function Linked({ id }: { id: EntityId }) {
  const references = useRead((g) => g.references(id));
  if (references.length === 0) return null;
  return (
    <section aria-label="Linked references">
      <h2 className={`${heading} mb-3`}>Linked references ({count(references)})</h2>
      <Groups references={references} />
    </section>
  );
}

function UnlinkedList({ id }: { id: EntityId }) {
  const references = useRead((g) => g.unlinkedReferences(id));
  if (references.length === 0) {
    return <p className="text-sm text-slate-500 dark:text-slate-400">None.</p>;
  }
  return <Groups references={references} />;
}

/** Fetched only once opened, since finding them means reading every block. */
function Unlinked({ id }: { id: EntityId }) {
  const [shown, setShown] = useState(false);
  return (
    <details onToggle={(e) => setShown(e.currentTarget.open)}>
      <summary className={`${heading} cursor-pointer select-none`}>Unlinked references</summary>
      {shown && (
        <div className="mt-3">
          <Region
            pending={<Skeleton className="h-4 w-1/2" />}
            failed={(error) => (
              <p role="alert" className="text-sm text-red-700 dark:text-red-300">
                {error.message}
              </p>
            )}
          >
            <UnlinkedList id={id} />
          </Region>
        </div>
      )}
    </details>
  );
}

/** At the foot of an entity. Each part loads, and fails, on its own. */
export function References({ id }: { id: EntityId }) {
  return (
    <footer className="mt-10 space-y-6 border-t border-slate-200 pt-6 dark:border-slate-700">
      <Region
        key={id}
        pending={<Skeleton className="h-4 w-1/3" />}
        failed={(error) => (
          <p role="alert" className="text-sm text-red-700 dark:text-red-300">
            {error.message}
          </p>
        )}
      >
        <Linked id={id} />
      </Region>
      <Unlinked key={id} id={id} />
    </footer>
  );
}
