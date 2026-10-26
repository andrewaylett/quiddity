"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { useGraph, useRead } from "@/lib/use-graph";
import { Region, Skeleton } from "./boundary";
import { EntityLink } from "./entity-link";
import { useView } from "./use-view";

function List() {
  const entities = useRead((g) => g.entities());
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

function NewEntity() {
  const graph = useGraph();
  const router = useRouter();
  const view = useView();
  const [name, setName] = useState("");
  return (
    <form
      className="mb-6 flex gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        const trimmed = name.trim();
        if (!trimmed) return;
        router.push(view.toMain(graph.createEntity(trimmed)));
      }}
    >
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="New entity"
        aria-label="Name of a new entity"
        className="min-w-0 flex-1 rounded border border-slate-300 bg-transparent px-2 py-1 dark:border-slate-600"
      />
      <button
        type="submit"
        className="rounded border border-slate-300 px-3 py-1 hover:bg-slate-200 dark:border-slate-600 dark:hover:bg-slate-700"
      >
        Create
      </button>
    </form>
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
        <NewEntity />
        <List />
      </Region>
    </article>
  );
}
