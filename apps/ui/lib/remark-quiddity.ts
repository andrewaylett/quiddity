import type { PhrasingContent, Root } from "mdast";
import { visit } from "unist-util-visit";

/** Stands in for an inline property in the text the Markdown parser sees. */
export const PROP_OPEN = "";
export const PROP_CLOSE = "";

const TOKEN = new RegExp(
  `\\[\\[([^\\]]+)\\]\\]|\\(\\(([^)]+)\\)\\)|${PROP_OPEN}(\\d+)${PROP_CLOSE}`,
  "g",
);

const custom = (
  name: string,
  properties: Record<string, string>,
  text: string,
): PhrasingContent =>
  ({
    type: "text",
    value: text,
    data: { hName: name, hProperties: properties },
  }) as PhrasingContent;

/**
 * Finds Quiddity's own constructs in the text of a Markdown document: links,
 * block references, and the placeholders left for inline properties. Looking
 * only at text nodes means code is left alone. Each becomes a custom element
 * for the renderer to draw.
 */
export function remarkQuiddity() {
  return (tree: Root) => {
    visit(tree, "text", (node, index, parent) => {
      if (!parent || index === undefined) return;
      const parts: PhrasingContent[] = [];
      let last = 0;
      for (const m of node.value.matchAll(TOKEN)) {
        if (m.index > last) parts.push({ type: "text", value: node.value.slice(last, m.index) });
        if (m[1] !== undefined) parts.push(custom("quiddity-link", { name: m[1] }, m[1]));
        else if (m[2] !== undefined) parts.push(custom("quiddity-ref", { id: m[2] }, m[0]));
        else parts.push(custom("quiddity-prop", { index: m[3]! }, ""));
        last = m.index + m[0].length;
      }
      if (parts.length === 0) return;
      if (last < node.value.length) parts.push({ type: "text", value: node.value.slice(last) });
      parent.children.splice(index, 1, ...parts);
      return index + parts.length;
    });
  };
}
