"use client";

import Link from "next/link";
import { useState } from "react";
import type { EntityId } from "@/lib/model";
import { Region, Skeleton } from "./boundary";
import { EntityBody, EntityTitle } from "./entity-view";
import { useView } from "./use-view";

const iconButton =
  "rounded px-1.5 py-0.5 text-sm text-slate-500 hover:bg-slate-200 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-700 dark:hover:text-slate-200";

/**
 * The header is outside the region that suspends, so a card can always be
 * collapsed or closed, even while its content is loading or has failed.
 */
function Secondary({ id }: { id: EntityId }) {
  const view = useView();
  const [collapsed, setCollapsed] = useState(false);
  return (
    <article className="rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-900">
      <header className="flex items-center gap-1 px-3 py-1.5">
        <h3 className="min-w-0 flex-1 truncate text-sm font-medium">
          <Region
            pending={<Skeleton className="h-4 w-24" />}
            failed={() => <span className="text-slate-500">{id}</span>}
          >
            <Link href={view.toMainFromSide(id)} className="hover:underline">
              <EntityTitle id={id} />
            </Link>
          </Region>
        </h3>
        <button
          type="button"
          className={iconButton}
          aria-expanded={!collapsed}
          aria-label={collapsed ? "Expand" : "Collapse"}
          onClick={() => setCollapsed((c) => !c)}
        >
          {collapsed ? "▸" : "▾"}
        </button>
        <Link href={view.toClosed(id)} className={iconButton} aria-label="Close">
          ✕
        </Link>
      </header>
      {!collapsed && (
        <div className="border-t border-slate-200 px-3 py-2 text-sm dark:border-slate-700">
          <Region
            pending={
              <div className="space-y-2" role="status" aria-label="Loading">
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-2/3" />
              </div>
            }
            failed={(error) => (
              <p role="alert" className="text-red-700 dark:text-red-300">
                {error.message}
              </p>
            )}
          >
            <EntityBody id={id} />
          </Region>
        </div>
      )}
    </article>
  );
}

export function RightPane() {
  const { open, toAllClosed } = useView();
  if (open.length === 0) return null;
  return (
    <>
      <div className="flex items-center justify-between px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
          Open
        </h2>
        <Link href={toAllClosed()} className={iconButton}>
          Close all
        </Link>
      </div>
      <div className="space-y-3 overflow-y-auto px-3 pb-3">
        {open.map((id) => (
          <Secondary key={id} id={id} />
        ))}
      </div>
    </>
  );
}
