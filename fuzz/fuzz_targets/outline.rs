//! Arbitrary files through the outline parser, in both indentation modes.
//!
//! - Parsing never panics.
//! - Whatever is parsed, formatting it and parsing the result (as Quiddity's
//!   own format) gives the same outline, and formatting that gives the same
//!   text: one import normalises a file, and from then on it is stable.
#![no_main]

use libfuzzer_sys::fuzz_target;
use quiddity_core::outline::{Indent, format_outline, parse_outline};

fuzz_target!(|input: (bool, &str)| {
    let (auto, src) = input;
    let indent = if auto { Indent::Auto } else { Indent::Tabs };
    let outline = parse_outline(src, indent);
    let text = format_outline(&outline);
    let again = parse_outline(&text, Indent::Tabs);
    assert_eq!(again, outline, "formatted as {text:?}");
    assert_eq!(format_outline(&again), text);
});
