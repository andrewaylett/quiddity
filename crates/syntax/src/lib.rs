//! Block-level Markdown syntax for Quiddity.
//!
//! A block's text is Markdown, and it is the source of truth for the block's
//! properties. This crate finds the Quiddity-specific constructs in that text:
//!
//! | Syntax                         | Meaning                                        |
//! |--------------------------------|------------------------------------------------|
//! | `key:: value`                  | a property, on a line of its own (Logseq style) |
//! | `#tag`                         | a property with no value                       |
//! | `#prop(value)`                 | a property with a value                        |
//! | `#prop([a, b])`                | a property whose value is a list               |
//! | `#[[multi word]](value)`       | a property whose name contains spaces          |
//! | `[[Name]]`                     | a link to an entity by name                    |
//! | `[[Name#type(person)]]`        | a link narrowed by the target's properties     |
//! | `((id))`                       | a reference to a block by its `id` property    |
//!
//! Parsing never fails: anything that isn't well-formed is plain text.

mod format;
mod name;
mod parse;

pub use format::{
    format_inline_property, format_key, format_link, format_property_line, format_value,
};
pub use name::normalise_name;
pub use parse::{
    Block, Link, Property, PropertyStyle, Qualifier, Value, is_properties_block, parse_block,
    parse_line_value, parse_property_line,
};
