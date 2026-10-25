use quiddity_core::outline::export_entity;
use quiddity_core::{Graph, Result};

fn sync(a: &Graph, b: &Graph) -> Result<()> {
    a.commit();
    b.commit();
    b.import(&a.updates_since(&b.version())?)?;
    a.import(&b.updates_since(&a.version())?)?;
    Ok(())
}

fn render(graph: &Graph) -> Result<String> {
    graph
        .roots()
        .into_iter()
        .map(|e| export_entity(graph, e))
        .collect()
}

#[test]
fn replicas_converge() -> Result<()> {
    let laptop = Graph::new();
    let phone = Graph::blank();
    assert_eq!(phone.schema_version(), None);

    let page = laptop.create_entity("Plans", &[])?;
    let a = laptop.create_block(Some(page), None, "Book the train")?;
    let b = laptop.create_block(Some(page), None, "Pack")?;
    sync(&laptop, &phone)?;
    assert_eq!(phone.schema_version(), laptop.schema_version());

    // Concurrent edits to the same block's text merge character by character.
    laptop.set_text(a, "Book the early train")?;
    phone.set_text(a, "Book the train #status(done)")?;
    // Concurrent moves that would make a cycle: Loro keeps one of them.
    laptop.move_block(b, Some(a), None)?;
    phone.move_block(a, Some(b), None)?;
    sync(&laptop, &phone)?;

    assert_eq!(laptop.text(a)?, "Book the early train #status(done)");
    assert_eq!(render(&laptop)?, render(&phone)?);
    let a_under_b = laptop.parent(a) == Some(b);
    let b_under_a = laptop.parent(b) == Some(a);
    assert!(a_under_b != b_under_a, "exactly one move wins");
    Ok(())
}

#[test]
fn late_joiner_catches_up_from_a_snapshot() -> Result<()> {
    let node = Graph::new();
    let page = node.create_entity("Page", &[])?;
    node.create_block(Some(page), None, "hello")?;
    node.commit();

    let tablet = Graph::from_snapshot(&node.snapshot()?)?;
    tablet.create_block(Some(page), None, "from the tablet")?;
    sync(&node, &tablet)?;
    assert_eq!(render(&node)?, render(&tablet)?);
    assert_eq!(node.children(page).len(), 2);
    Ok(())
}
