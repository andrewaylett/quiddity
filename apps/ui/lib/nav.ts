import type { EntityId } from "./model";

/**
 * The whole view state lives in the URL: `/entity/?id=<main>&open=<a>,<b>`.
 * `/` is the journals home and `/all/` lists every entity. The right pane is the ordered `open` list.
 */
export interface View {
  /** The page shown when there is no `main` entity: `/` or `/all/`. */
  path: string;
  main: EntityId | null;
  open: EntityId[];
}

export function parseView(pathname: string, params: URLSearchParams): View {
  const main = pathname.startsWith("/entity") ? params.get("id") : null;
  const open = (params.get("open") ?? "").split(",").filter(Boolean);
  const path = pathname.startsWith("/all") ? "/all/" : "/";
  return { path, main, open: [...new Set(open)] };
}

export function viewHref({ path, main, open }: View): string {
  const params = new URLSearchParams();
  if (main) params.set("id", main);
  if (open.length) params.set("open", open.join(","));
  const query = params.size ? `?${params}` : "";
  return `${main ? "/entity/" : path}${query}`;
}
