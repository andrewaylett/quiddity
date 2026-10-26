"use client";

import { usePathname, useSearchParams } from "next/navigation";
import { useMemo } from "react";
import type { EntityId } from "@/lib/model";
import { parseView, viewHref, type View } from "@/lib/nav";

/** The current view, and hrefs derived from it that keep the rest of it. */
export function useView() {
  const pathname = usePathname();
  const params = useSearchParams();
  const view = useMemo(() => parseView(pathname, params), [pathname, params]);
  return {
    ...view,
    toMain: (id: EntityId) => viewHref({ ...view, main: id }),
    toSide: (id: EntityId): string =>
      viewHref({
        ...view,
        open: view.open.includes(id) ? view.open : [...view.open, id],
      }),
    toClosed: (id: EntityId) =>
      viewHref({ ...view, open: view.open.filter((o) => o !== id) }),
    toAllClosed: () => viewHref({ ...view, open: [] }),
    toMainFromSide: (id: EntityId) =>
      viewHref({ ...view, main: id, open: view.open.filter((o) => o !== id) }),
  } satisfies View & Record<string, unknown>;
}
