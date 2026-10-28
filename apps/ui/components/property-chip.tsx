"use client";

import type { ReactNode } from "react";
import { useRouter } from "next/navigation";
import { useGraph } from "@/lib/use-graph";
import type { LinkTarget, Property, PropertyValue } from "@/lib/model";
import { EntityLink } from "./entity-link";
import { useView } from "./use-view";

const unresolved = "cursor-help underline decoration-dashed underline-offset-2";

/** A link to an entity, as the core resolved it. */
export function LinkOrNot({
  name,
  linkKey = name,
  links,
}: {
  name: string;
  linkKey?: string;
  links: Record<string, LinkTarget>;
}) {
  const target = links[linkKey] ?? { kind: "missing" };
  if (target.kind === "entity") {
    return (
      <EntityLink
        id={target.id}
        className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
      >
        {name}
      </EntityLink>
    );
  }
  if (target.kind === "missing") return <NewEntityLink name={name} linkKey={linkKey} />;
  return (
    <span
      className={`text-slate-500 dark:text-slate-400 ${unresolved}`}
      title="Several entities match"
    >
      {name}
    </span>
  );
}

/** A link to an entity that doesn't exist yet. Following it makes the entity. */
function NewEntityLink({ name, linkKey }: { name: string; linkKey: string }) {
  const graph = useGraph();
  const view = useView();
  const router = useRouter();
  return (
    <button
      type="button"
      className="text-sky-700 underline decoration-dashed underline-offset-2 hover:decoration-solid dark:text-sky-300"
      title="No such entity yet. Open it to make it."
      onClick={(e) => {
        const id = graph.followLink(linkKey);
        router.push(e.shiftKey ? view.toSide(id) : view.toMain(id));
      }}
    >
      {name}
    </button>
  );
}

function Value({
  value,
  links,
}: {
  value: PropertyValue;
  links: Record<string, LinkTarget>;
}) {
  switch (value.kind) {
    case "text":
      return <>{value.text}</>;
    case "link":
      return <LinkOrNot name={value.name} linkKey={value.key} links={links} />;
    case "ref":
      return <span className="text-slate-500 dark:text-slate-400">(({value.id}))</span>;
    case "list":
      return (
        <>
          {value.items.map((item, i) => (
            <span key={i}>
              {i > 0 && ", "}
              <Value value={item} links={links} />
            </span>
          ))}
        </>
      );
  }
}

/** A property's key, which goes to the entity that defines the property. */
function PropertyKey({
  property,
  className,
  children,
}: {
  property: Property;
  className: string;
  children: ReactNode;
}) {
  const graph = useGraph();
  const view = useView();
  const router = useRouter();
  if (property.definition !== null) {
    return (
      <EntityLink id={property.definition} className={`${className} hover:underline`}>
        {children}
      </EntityLink>
    );
  }
  // Nobody has defined it yet, so following it does.
  return (
    <button
      type="button"
      className={`${className} underline decoration-dashed underline-offset-2 hover:decoration-solid`}
      title="Not defined yet. Open it to define it."
      onClick={(e) => {
        const id = graph.defineProperty(property.key);
        router.push(e.shiftKey ? view.toSide(id) : view.toMain(id));
      }}
    >
      {children}
    </button>
  );
}

/** A property, as a pill: the key, then its value if it has one. */
export function PropertyChip({
  property,
  links,
}: {
  property: Property;
  links: Record<string, LinkTarget>;
}) {
  return (
    <span className="inline-flex items-baseline gap-1 rounded-full bg-slate-200 px-2 py-0.5 text-xs leading-5 dark:bg-slate-700">
      {property.value === null ? (
        <PropertyKey property={property} className="">
          #{property.key}
        </PropertyKey>
      ) : (
        <>
          <PropertyKey property={property} className="text-slate-500 dark:text-slate-400">
            {property.key}
          </PropertyKey>
          <span>
            <Value value={property.value} links={links} />
          </span>
        </>
      )}
    </span>
  );
}
