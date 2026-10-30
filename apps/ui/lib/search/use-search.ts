"use client";

import { useEffect, useState, useSyncExternalStore } from "react";
import { useGraph } from "../use-graph";
import type { Facet, Hit } from "./facet";

export interface Section {
  facet: Facet;
  hits: Hit[];
}

/**
 * Runs every facet on `query` and keeps each one's latest answer, so the
 * panel fills in as facets finish and an old answer stays up (rather than
 * flickering empty) until its replacement arrives. Searches again when the
 * graph changes. Empty sections are kept; the panel decides what to show.
 */
export function useSearch(facets: Facet[], query: string): Section[] {
  const graph = useGraph();
  const revision = useSyncExternalStore(graph.subscribe, graph.getRevision, graph.getRevision);
  const [answers, setAnswers] = useState<Record<string, Hit[]>>({});
  const trimmed = query.trim();

  useEffect(() => {
    let current = true;
    if (!trimmed) {
      setAnswers({});
      return;
    }
    for (const facet of facets) {
      void Promise.resolve()
        .then(() => facet.search(trimmed, graph))
        .catch((error: unknown) => {
          console.error(`Search of ${facet.id} failed`, error);
          return [];
        })
        .then((hits) => current && setAnswers((old) => ({ ...old, [facet.id]: hits })));
    }
    return () => {
      current = false;
    };
  }, [facets, trimmed, graph, revision]);

  return facets.map((facet) => ({ facet, hits: trimmed ? (answers[facet.id] ?? []) : [] }));
}
