use crate::parse::{Link, Value, is_bare_key, parse_block, parse_line_value};

/// Where a value is being written; each context has different delimiters.
#[derive(Debug, Clone, Copy)]
enum Context {
    /// Inside `#key(…)`.
    Paren,
    /// Inside `[…]`.
    ListItem,
    /// After `key:: `.
    Line,
}

/// Formats a property key for use after `#`, bracketing it if necessary.
pub fn format_key(key: &str) -> String {
    if is_bare_key(key) {
        key.to_owned()
    } else {
        format!("[[{key}]]")
    }
}

/// Formats a value for use inside `#key(…)`.
pub fn format_value(value: &Value) -> String {
    format_in(value, Context::Paren)
}

/// Formats `#key` or `#key(value)`.
pub fn format_inline_property(key: &str, value: Option<&Value>) -> String {
    match value {
        Some(v) => format!("#{}({})", format_key(key), format_value(v)),
        None => format!("#{}", format_key(key)),
    }
}

/// Formats a `key:: value` line. The key must be valid as a line key.
pub fn format_property_line(key: &str, value: Option<&Value>) -> String {
    match value {
        Some(v) => format!("{key}:: {}", format_in(v, Context::Line)),
        None => format!("{key}::"),
    }
}

/// Formats a link, escaping its name only where necessary.
pub fn format_link(link: &Link) -> String {
    let qualifiers: String = link
        .qualifiers
        .iter()
        .map(|q| format_inline_property(&q.key, q.value.as_ref()))
        .collect();
    let plain = format!("[[{}{qualifiers}]]", link.name);
    if parse_block(&plain).links.first() == Some(link) {
        return plain;
    }
    let mut escaped = String::new();
    for c in link.name.chars() {
        if matches!(c, '\\' | '[' | ']' | '#') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("[[{escaped}{qualifiers}]]")
}

fn format_in(value: &Value, ctx: Context) -> String {
    match value {
        Value::Text(s) => format_text(s, ctx),
        Value::Link(link) => format_link(link),
        Value::BlockRef(id) => format!("(({id}))"),
        Value::List(items) => {
            let items: Vec<_> = items
                .iter()
                .map(|v| format_in(v, Context::ListItem))
                .collect();
            format!("[{}]", items.join(", "))
        }
    }
}

/// Writes text as it is if it would parse back unchanged, otherwise quoted.
fn format_text(s: &str, ctx: Context) -> String {
    let as_is = !s.is_empty()
        && !s.contains(['\n', '\r'])
        && reparse(s, ctx).is_some_and(|v| v == Value::Text(s.to_owned()));
    if as_is {
        return s.to_owned();
    }
    let mut quoted = String::from('"');
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn reparse(raw: &str, ctx: Context) -> Option<Value> {
    match ctx {
        Context::Paren => single_inline_value(&format!("#k({raw})")),
        Context::ListItem => match single_inline_value(&format!("#k([{raw}])")) {
            Some(Value::List(mut items)) if items.len() == 1 => items.pop(),
            _ => None,
        },
        Context::Line => Some(parse_line_value(raw)),
    }
}

fn single_inline_value(src: &str) -> Option<Value> {
    match parse_block(src).properties.as_slice() {
        [p] if p.span == (0..src.len()) => p.value.clone(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{Qualifier, parse_property_line};

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn samples() -> Vec<Value> {
        let jane = Link {
            name: "Jane".into(),
            qualifiers: vec![Qualifier {
                key: "type".into(),
                value: Some(text("person")),
            }],
        };
        vec![
            text("plain"),
            text("with spaces"),
            text(""),
            text("  padded "),
            text("a, b"),
            text("a)b"),
            text("a]b"),
            text("[not a list]"),
            text("\"quoted\""),
            text(r"back\slash"),
            text("multi\nline"),
            text("[[looks like a link]]"),
            text("#hash"),
            Value::Link(jane.clone()),
            Value::Link(Link {
                name: "odd]]name#x".into(),
                qualifiers: vec![],
            }),
            Value::Link(Link {
                name: "Book".into(),
                qualifiers: vec![Qualifier {
                    key: "author".into(),
                    value: Some(Value::Link(jane)),
                }],
            }),
            Value::BlockRef("0198f0c4".into()),
            Value::List(vec![
                text("a"),
                text("b, c"),
                text(""),
                Value::BlockRef("x".into()),
            ]),
            Value::List(vec![]),
        ]
    }

    #[test]
    fn inline_round_trip() {
        for value in samples() {
            for key in ["k", "due date"] {
                let src = format_inline_property(key, Some(&value));
                let block = parse_block(&src);
                assert_eq!(block.properties.len(), 1, "{src}");
                assert_eq!(block.properties[0].key, key, "{src}");
                assert_eq!(block.properties[0].value.as_ref(), Some(&value), "{src}");
            }
        }
    }

    #[test]
    fn line_round_trip() {
        for value in samples() {
            let line = format_property_line("k", Some(&value));
            assert_eq!(
                parse_property_line(&line),
                Some(("k".into(), Some(value.clone()))),
                "{line}"
            );
        }
    }

    #[test]
    fn readable_where_possible() {
        assert_eq!(
            format_inline_property("status", Some(&text("done"))),
            "#status(done)"
        );
        assert_eq!(format_inline_property("due date", None), "#[[due date]]");
        assert_eq!(
            format_property_line("note", Some(&text("a, b"))),
            "note:: a, b"
        );
        assert_eq!(
            format_inline_property("note", Some(&text("a, b"))),
            "#note(a, b)"
        );
        assert_eq!(
            format_inline_property("tags", Some(&Value::List(vec![text("a, b"), text("c")]))),
            "#tags([\"a, b\", c])"
        );
        assert_eq!(
            format_link(&Link {
                name: "C# notes".into(),
                qualifiers: vec![]
            }),
            "[[C# notes]]"
        );
    }
}
