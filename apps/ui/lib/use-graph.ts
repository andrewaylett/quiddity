"use client";

import { use, useSyncExternalStore } from "react";
import { bootGraph, type Graph } from "./graph";

/** The graph; suspends until it has loaded. */
export function useGraph(): Graph {
  return use(bootGraph());
}

/**
 * Reads from the graph, and re-renders when it changes. The graph keeps each
 * answer until the next edit, so what this returns is stable between them.
 */
export function useRead<T>(read: (graph: Graph) => T): T {
  const graph = useGraph();
  useSyncExternalStore(graph.subscribe, graph.getRevision, graph.getRevision);
  return read(graph);
}
