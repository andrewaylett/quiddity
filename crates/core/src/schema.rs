//! Property schemas: how a property's values should be read.
//!
//! A property's schema comes from its property entity (an entity of type
//! `property` with the property's name), falling back to a built-in schema
//! for Quiddity's own properties. Schemas are advisory: values that don't
//! match are kept and reported, never discarded.

use jiff::Timestamp;
use jiff::civil::Date;
use quiddity_syntax::{Link, Value, format_link, normalise_name};

use crate::Index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Text,
    Number,
    Boolean,
    Date,
    DateTime,
    /// A link to an entity, or a reference to a block.
    Ref,
}

/// Whether each instance of a property holds one value or an ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Scalar,
    List,
}

/// Whether a block may have more than one instance of a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Many,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySchema {
    pub value_type: ValueType,
    pub form: Form,
    pub cardinality: Cardinality,
}

impl Default for PropertySchema {
    /// The schema for a property nobody has described: anything goes.
    fn default() -> Self {
        Self::new(ValueType::Text, Form::Scalar, Cardinality::Many)
    }
}

impl PropertySchema {
    pub const fn new(value_type: ValueType, form: Form, cardinality: Cardinality) -> Self {
        Self {
            value_type,
            form,
            cardinality,
        }
    }

    /// Schemas for the properties Quiddity itself relies on.
    pub fn builtin(key: &str) -> Option<Self> {
        use {Cardinality::*, Form::*, ValueType::*};
        Some(match normalise_name(key).as_str() {
            "type" => Self::new(Ref, Scalar, Many),
            "alias" => Self::new(Text, Scalar, Many),
            "date" => Self::new(Date, Scalar, One),
            "id" | "title" | "value-type" | "form" | "cardinality" => Self::new(Text, Scalar, One),
            _ => return None,
        })
    }
}

impl Index {
    /// The schema for the property `key`.
    pub fn schema(&self, key: &str) -> PropertySchema {
        let mut schema = PropertySchema::builtin(key).unwrap_or_default();
        let Some(info) = self.property_entity(key).and_then(|id| self.entity(id)) else {
            return schema;
        };
        let setting = |name| info.values(name).find_map(text).map(|s| normalise_name(&s));
        if let Some(t) = setting("value-type") {
            schema.value_type = match t.as_str() {
                "number" => ValueType::Number,
                "boolean" => ValueType::Boolean,
                "date" => ValueType::Date,
                "datetime" => ValueType::DateTime,
                "ref" => ValueType::Ref,
                _ => ValueType::Text,
            };
        }
        match setting("form").as_deref() {
            Some("list") => schema.form = Form::List,
            Some("scalar") => schema.form = Form::Scalar,
            _ => {}
        }
        match setting("cardinality").as_deref() {
            Some("one") => schema.cardinality = Cardinality::One,
            Some("many") => schema.cardinality = Cardinality::Many,
            _ => {}
        }
        schema
    }
}

/// A value read according to its schema.
#[derive(Debug, Clone, PartialEq)]
pub enum Typed {
    Text(String),
    Number(f64),
    Boolean(bool),
    Date(Date),
    DateTime(Timestamp),
    Link(Link),
    BlockRef(String),
    List(Vec<Typed>),
}

/// A value that doesn't fit its schema.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("expected {expected}, found {found:?}")]
pub struct Mismatch {
    pub expected: String,
    pub found: Value,
}

/// Reads a value according to a schema.
pub fn coerce(value: &Value, schema: &PropertySchema) -> Result<Typed, Mismatch> {
    match (schema.form, value) {
        (Form::List, Value::List(items)) => items
            .iter()
            .map(|v| coerce_scalar(v, schema.value_type))
            .collect::<Result<_, _>>()
            .map(Typed::List),
        (Form::List, _) => Err(mismatch("a list", value)),
        (Form::Scalar, Value::List(_)) => Err(mismatch("a single value", value)),
        (Form::Scalar, _) => coerce_scalar(value, schema.value_type),
    }
}

fn coerce_scalar(value: &Value, value_type: ValueType) -> Result<Typed, Mismatch> {
    let parsed = match (value_type, value) {
        (ValueType::Text, Value::Text(s)) => Some(Typed::Text(s.clone())),
        (ValueType::Text, Value::Link(link)) => Some(Typed::Text(format_link(link))),
        (ValueType::Text, Value::BlockRef(id)) => Some(Typed::Text(format!("(({id}))"))),
        (ValueType::Number, Value::Text(s)) => s
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Typed::Number),
        (ValueType::Boolean, Value::Text(s)) => match normalise_name(s).as_str() {
            "true" | "yes" => Some(Typed::Boolean(true)),
            "false" | "no" => Some(Typed::Boolean(false)),
            _ => None,
        },
        (ValueType::Date, Value::Text(s)) => s.trim().parse().ok().map(Typed::Date),
        (ValueType::DateTime, Value::Text(s)) => s.trim().parse().ok().map(Typed::DateTime),
        (ValueType::Ref, Value::Link(link)) => Some(Typed::Link(link.clone())),
        (ValueType::Ref, Value::BlockRef(id)) => Some(Typed::BlockRef(id.clone())),
        // `type:: person` means the same as `type:: [[person]]`.
        (ValueType::Ref, Value::Text(s)) => Some(Typed::Link(Link {
            name: s.clone(),
            qualifiers: Vec::new(),
        })),
        _ => None,
    };
    parsed.ok_or_else(|| mismatch(&format!("{value_type:?}").to_lowercase(), value))
}

fn mismatch(expected: &str, found: &Value) -> Mismatch {
    Mismatch {
        expected: expected.to_owned(),
        found: found.clone(),
    }
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Link(link) => Some(link.name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph, Result};

    fn t(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    #[test]
    fn schema_from_property_entity() -> Result<()> {
        let graph = Graph::new();
        graph.create_entity(
            "Rating",
            &[
                ("type", Some(t("property"))),
                ("value-type", Some(t("number"))),
                ("cardinality", Some(t("one"))),
            ],
        )?;
        graph.create_entity(
            "Steps",
            &[("type", Some(t("property"))), ("form", Some(t("list")))],
        )?;
        let index = Index::build(&graph)?;

        let rating = index.schema("rating");
        assert_eq!(
            rating,
            PropertySchema::new(ValueType::Number, Form::Scalar, Cardinality::One)
        );
        assert_eq!(coerce(&t("4.5"), &rating), Ok(Typed::Number(4.5)));
        assert!(coerce(&t("lots"), &rating).is_err());
        assert!(coerce(&t("NaN"), &rating).is_err());

        let steps = index.schema("Steps");
        assert_eq!(steps.form, Form::List);
        assert_eq!(
            coerce(&Value::List(vec![t("a"), t("b")]), &steps),
            Ok(Typed::List(vec![
                Typed::Text("a".into()),
                Typed::Text("b".into())
            ]))
        );
        assert!(coerce(&t("a"), &steps).is_err());

        assert_eq!(index.schema("unknown"), PropertySchema::default());
        Ok(())
    }

    #[test]
    fn builtin_types() {
        let date = PropertySchema::builtin("date").unwrap();
        assert_eq!(
            coerce(&t("2026-10-25"), &date),
            Ok(Typed::Date("2026-10-25".parse().unwrap()))
        );
        assert!(coerce(&t("2026-02-30"), &date).is_err());

        let at = PropertySchema::new(ValueType::DateTime, Form::Scalar, Cardinality::One);
        assert!(coerce(&t("2026-10-25T21:04:00+01:00"), &at).is_ok());
        assert!(
            coerce(&t("2026-10-25T21:04:00"), &at).is_err(),
            "an offset is required"
        );

        let ty = PropertySchema::builtin("type").unwrap();
        assert!(matches!(coerce(&t("person"), &ty), Ok(Typed::Link(l)) if l.name == "person"));

        let flag = PropertySchema::new(ValueType::Boolean, Form::Scalar, Cardinality::One);
        assert_eq!(coerce(&t("Yes"), &flag), Ok(Typed::Boolean(true)));
    }
}
