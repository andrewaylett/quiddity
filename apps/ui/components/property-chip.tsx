"use client";

import type { LinkTarget, Property, PropertyValue } from "@/lib/model";
import { EntityLink } from "./entity-link";

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
  return target.kind === "entity" ? (
    <EntityLink
      id={target.id}
      className="text-sky-700 underline-offset-2 hover:underline dark:text-sky-300"
    >
      {name}
    </EntityLink>
  ) : (
    <span
      className={`text-slate-500 dark:text-slate-400 ${unresolved}`}
      title={target.kind === "ambiguous" ? "Several entities match" : "No such entity"}
    >
      {name}
    </span>
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
        <span>#{property.key}</span>
      ) : (
        <>
          <span className="text-slate-500 dark:text-slate-400">{property.key}</span>
          <span>
            <Value value={property.value} links={links} />
          </span>
        </>
      )}
    </span>
  );
}
