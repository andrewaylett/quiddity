"use client";

import { use } from "react";
import { store } from "@/lib/store";
import { Region, Skeleton } from "./boundary";
import { MainEntity } from "./main-entity";

function Today() {
  return <MainEntity id={use(store.today()).id} />;
}

/** The journals home. Today's page for now; scroll-back comes next. */
export function HomeEntity() {
  return (
    <Region
      pending={<Skeleton className="m-8 h-8 w-1/3" />}
      failed={(error) => (
        <p role="alert" className="px-6 py-8 text-red-700 dark:text-red-300">
          {error.message}
        </p>
      )}
    >
      <Today />
    </Region>
  );
}
