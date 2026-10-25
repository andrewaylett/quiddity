use quiddity_syntax::{Property, Value, format_property_line, is_properties_block, parse_block};

use crate::{BlockId, Graph, Result};

impl Graph {
    /// Creates an entity: a root block named `name`, with an initial
    /// properties block if `properties` isn't empty.
    pub fn create_entity(
        &self,
        name: &str,
        properties: &[(&str, Option<Value>)],
    ) -> Result<BlockId> {
        let entity = self.create_block(None, None, name)?;
        if !properties.is_empty() {
            let lines: Vec<_> = properties
                .iter()
                .map(|(key, value)| format_property_line(key, value.as_ref()))
                .collect();
            self.create_block(Some(entity), None, &lines.join("\n"))?;
        }
        Ok(entity)
    }

    /// An entity's name: the first line of its root block.
    pub fn entity_name(&self, entity: BlockId) -> Result<String> {
        Ok(self
            .text(entity)?
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_owned())
    }

    /// The entity's initial properties block, if its first child is one.
    pub fn properties_block(&self, entity: BlockId) -> Result<Option<BlockId>> {
        match self.children(entity).first() {
            Some(&first) if is_properties_block(&self.text(first)?) => Ok(Some(first)),
            _ => Ok(None),
        }
    }

    /// The entity's own properties, from its initial properties block.
    pub fn entity_properties(&self, entity: BlockId) -> Result<Vec<Property>> {
        match self.properties_block(entity)? {
            Some(block) => self.block_properties(block),
            None => Ok(Vec::new()),
        }
    }

    /// The properties written in a block's text.
    pub fn block_properties(&self, block: BlockId) -> Result<Vec<Property>> {
        Ok(parse_block(&self.text(block)?).properties)
    }

    /// Returns the block's `id` property, adding a new one if it has none, so
    /// that it can be referred to as `((id))`.
    pub fn ensure_ref_id(&self, block: BlockId) -> Result<String> {
        let text = self.text(block)?;
        let existing = parse_block(&text)
            .properties
            .into_iter()
            .find_map(|p| match p.value {
                Some(Value::Text(id)) if p.key.eq_ignore_ascii_case("id") => Some(id),
                _ => None,
            });
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let line = format_property_line("id", Some(&Value::Text(id.clone())));
        let sep = if text.is_empty() || text.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        self.set_text(block, &format!("{text}{sep}{line}"))?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Option<Value> {
        Some(Value::Text(s.to_owned()))
    }

    #[test]
    fn entity_properties() -> Result<()> {
        let graph = Graph::new();
        let jane = graph.create_entity(
            "Jane",
            &[("type", text("person")), ("born", text("1970-01-01"))],
        )?;
        graph.create_block(Some(jane), None, "Met at #event(summit)")?;
        assert_eq!(graph.entity_name(jane)?, "Jane");
        let props = graph.entity_properties(jane)?;
        assert_eq!(
            props.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(),
            ["type", "born"]
        );

        let plain = graph.create_entity("Plain", &[])?;
        graph.create_block(Some(plain), None, "not:: only\nproperties")?;
        assert_eq!(graph.properties_block(plain)?, None);
        Ok(())
    }

    #[test]
    fn ref_ids_are_stable() -> Result<()> {
        let graph = Graph::new();
        let page = graph.create_entity("Page", &[])?;
        let block = graph.create_block(Some(page), None, "Quotable")?;
        let id = graph.ensure_ref_id(block)?;
        assert_eq!(graph.ensure_ref_id(block)?, id);
        assert_eq!(graph.text(block)?, format!("Quotable\nid:: {id}"));
        Ok(())
    }
}
