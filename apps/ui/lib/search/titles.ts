import type { Facet } from "./facet";

/** Pages by title: fuzzy, so `QD` finds `Quiddity Design` (see `quiddity-view`'s search). */
export const titles: Facet = {
  id: "titles",
  title: "Pages",
  search: (query, graph) =>
    graph.searchTitles(query, 50).map(({ entity, score, ranges }) => ({
      key: entity.id,
      label: entity.title,
      ranges,
      score,
      target: { kind: "entity", id: entity.id },
    })),
};
