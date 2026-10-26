"use client";

import { use } from "react";
import { store } from "@/lib/store";
import { Region, Skeleton } from "./boundary";
import { EntityLink } from "./entity-link";

function List() {
  const entities = use(store.entities());
  if (entities.length === 0) return <p>Nothing here yet.</p>;
  return (
    <ul className="columns-1 gap-8 sm:columns-2">
      {entities.map((e) => (
        <li key={e.id} className="break-inside-avoid py-0.5">
          <EntityLink
            id={e.id}
            className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
          >
            {e.title}
          </EntityLink>
        </li>
      ))}
    </ul>
  );
}

export function AllEntities() {
  return (
    <article className="mx-auto max-w-3xl px-6 py-8">
      <h1 className="mb-4 text-2xl font-semibold">All entities</h1>
      <Region
        pending={<Skeleton className="h-4 w-1/2" />}
        failed={(error) => (
          <p role="alert" className="text-red-700 dark:text-red-300">
            {error.message}
          </p>
        )}
      >
        <List />
      </Region>
    </article>
  );
}
