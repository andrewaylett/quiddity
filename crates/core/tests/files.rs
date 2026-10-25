use std::fs;

use quiddity_core::files::{export_dir, import_dir};
use quiddity_core::syntax::{Link, Value};
use quiddity_core::{Graph, Index, Resolution, Result, ensure_journal};

fn text(s: &str) -> Option<Value> {
    Some(Value::Text(s.to_owned()))
}

#[test]
fn export_import_round_trip() -> Result<()> {
    let graph = Graph::new();
    let jane = graph.create_entity("Jane", &[("type", text("person"))])?;
    graph.create_entity("Jane", &[("type", text("book"))])?;
    let notes = graph.create_entity("Notes: misc/other", &[])?;
    let lunch = graph.create_block(
        Some(notes),
        None,
        "Lunch with [[Jane#type(person)]]\nsecond line",
    )?;
    graph.create_block(Some(lunch), None, "#tags([a, b]) nested")?;
    let quote = graph.create_block(Some(jane), None, "Quotable")?;
    let id = graph.ensure_ref_id(quote)?;
    graph.create_block(Some(notes), None, &format!("See (({id}))"))?;
    let day = ensure_journal(
        &graph,
        &Index::build(&graph)?,
        "2026-10-25".parse().unwrap(),
    )?;
    graph.create_block(Some(day), None, "Wrote the exporter")?;
    graph.commit();

    let tmp = tempfile::tempdir()?;
    let first = tmp.path().join("first");
    assert_eq!(export_dir(&graph, &first)?, 4);
    assert!(first.join("entities/Jane.md").is_file());
    assert!(first.join("entities/Jane 2.md").is_file());
    assert!(first.join("entities/Notes_ misc_other.md").is_file());
    assert_eq!(
        fs::read_to_string(first.join("journals/2026-10-25.md"))?,
        "title:: 2026-10-25\ntype:: journal\ndate:: 2026-10-25\n\n- Wrote the exporter\n"
    );
    assert!(export_dir(&graph, &first).is_err(), "refuses to overwrite");

    let copy = Graph::new();
    import_dir(&copy, &first)?;
    let index = Index::build(&copy)?;
    let link = Link {
        name: "jane".into(),
        qualifiers: vec![],
    };
    assert!(matches!(index.resolve(&link), Resolution::Ambiguous(ids) if ids.len() == 2));
    let quoted = index.resolve_block_ref(&id).expect("block refs survive");
    assert_eq!(copy.text(quoted)?, format!("Quotable\nid:: {id}"));

    let second = tmp.path().join("second");
    export_dir(&copy, &second)?;
    for file in [
        "entities/Jane.md",
        "entities/Jane 2.md",
        "entities/Notes_ misc_other.md",
        "journals/2026-10-25.md",
    ] {
        let (a, b) = (
            fs::read_to_string(first.join(file))?,
            fs::read_to_string(second.join(file))?,
        );
        // Same-named entities may swap files, but the set of contents matches.
        if file.starts_with("entities/Jane") {
            let other = if file.ends_with("2.md") {
                "entities/Jane.md"
            } else {
                "entities/Jane 2.md"
            };
            assert!(
                b == a || b == fs::read_to_string(first.join(other))?,
                "{file}"
            );
        } else {
            assert_eq!(a, b, "{file}");
        }
    }
    Ok(())
}

#[test]
fn imports_a_logseq_graph() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path();
    fs::create_dir_all(dir.join("pages"))?;
    fs::create_dir_all(dir.join("journals"))?;
    fs::create_dir_all(dir.join("logseq/bak"))?;
    fs::write(
        dir.join("pages/Jane.md"),
        "alias:: JD\n\n- A person\n\t- with a child block\n",
    )?;
    fs::write(dir.join("journals/2026_10_25.md"), "- Met [[JD]]\n")?;
    fs::write(dir.join("logseq/bak/Jane.md"), "- an old backup\n")?;

    let graph = Graph::new();
    assert_eq!(import_dir(&graph, dir)?.len(), 2);
    let index = Index::build(&graph)?;
    let day = index
        .journal("2026-10-25".parse().unwrap())
        .expect("journal inferred from the file name");
    assert_eq!(graph.entity_name(day)?, "2026-10-25");
    let Resolution::Unique(jane) = index.resolve(&Link {
        name: "JD".into(),
        qualifiers: vec![],
    }) else {
        panic!("alias resolves");
    };
    assert_eq!(graph.entity_name(jane)?, "Jane");
    assert_eq!(index.backlinks(jane).len(), 1);
    Ok(())
}
