# quiddity-syntax

The reference parser and formatter for block text: `key:: value` lines,
`#key(value)` inline properties, `[[links#with(qualifiers)]]` and
`((block refs))`.

- **The formatter checks itself by reparsing.** `format_text` writes text
  bare only if the parser reads it back unchanged; otherwise it quotes. So a
  parser bug usually just makes the formatter quote more, and round-trip
  tests still pass. Parser behaviour needs its own examples
  (`parse.rs` tests), and readable output its own (`readable_where_possible`).
- **Nested values quote `[[` and `((`** even when they read back alone:
  `#p(a [[b] c]) see x]]` would otherwise read as a link reaching past the
  `)`. Whether a value is safe can't be judged from the value alone.
- **Lists start `[ ` when the first item starts with `[`**, so `[[[x]]]`
  isn't read as a link.
- **Link escaping is context-free**: `\`, `[` and `]` always, and `#`
  before a key character or `[`. A link reads back the same wherever it
  appears.
- **Keys and link names are single lines.** They're titles, written with
  whitespace collapsed (`normalise_name`); the generators exclude `\n` and
  `\r` for that reason.
- **References come from structure.** Links and block refs in a value count
  at the top level or in a list, but not inside quoted text or qualifiers.
  `Refs` collects them the same way for inline and line properties.
- **An inline property's value is read from its own text.** Nested links can
  read past the closing `)` and be settled by what follows it, so after the
  first reading finds where the value ends, `inline_value` reads that text
  again until it reads as exactly itself. Without it, a stray `]]` after a
  property could change what the property meant (the `block` fuzz target's
  "each property can be parsed from its span alone"). A value that is
  malformed on its own ends up shorter, down to a bare `#key`.
- **`#key` alone on a line is `key::`.** `parse_property_line` (and so
  `is_properties_block`) accepts it, and `format_property_line` writes a
  property with no value that way. `parse_block` still reads it as an inline
  property, since in prose it is a tag.
- `#` starts an inline property only after whitespace, `(`, the start of
  the text, or directly after another inline property (`#a(1)#b(2)`). Code
  spans and fences are skipped; all-digit keys (`#123`) aren't keys.

Robustness against arbitrary text is the `block` fuzz target's job; the
proptests check round trips of generated values.
