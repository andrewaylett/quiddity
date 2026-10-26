"use client";

import { use, useEffect } from "react";
import type { EntityId } from "@/lib/model";
import { recordVisit } from "@/lib/recents";
import { store } from "@/lib/store";
import { Region, Skeleton } from "./boundary";
import { EntityBody, EntityTitle } from "./entity-view";

function Visit({ id }: { id: EntityId }) {
  const entity = use(store.entity(id));
  useEffect(() => recordVisit(entity), [entity]);
  return null;
}

export function MainEntity({ id }: { id: EntityId }) {
  return (
    <article className="mx-auto max-w-3xl px-6 py-8">
      {/* Keyed so navigating resets the boundary rather than keeping an error. */}
      <Region
        key={id}
        pending={
          <div className="space-y-3" role="status" aria-label="Loading">
            <Skeleton className="h-8 w-1/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        }
        failed={(error) => (
          <div role="alert">
            <h1 className="text-2xl font-semibold">Couldn't open this</h1>
            <p className="mt-2 text-red-700 dark:text-red-300">{error.message}</p>
          </div>
        )}
      >
        <Visit id={id} />
        <h1 className="mb-4 text-2xl font-semibold">
          <EntityTitle id={id} />
        </h1>
        <EntityBody id={id} />
      </Region>
    </article>
  );
}
