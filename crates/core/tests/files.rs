use std::fs;

use quiddity_core::files::{export_dir, import_dir};
use quiddity_core::syntax::{Link, Value};
use quiddity_core::{Graph, Index, Resolution, Result, ensure_journal};

fn text(s: &str) -> Option<Value> {
    Some(Value::Text(s.to_owned()))
}

// Round trips are property-tested in tests/system.rs. These pin down the
// directory layout and how other tools' graphs are read.

#[test]
fn export_layout() -> Result<()> {
    let graph = Graph::new();
    graph.create_entity("Jane", &[("type", text("person"))])?;
    graph.create_entity("jane", &[("type", text("book"))])?;
    graph.create_entity("Notes: misc/other", &[])?;
    let day = ensure_journal(
        &graph,
        &Index::build(&graph)?,
        "2026-10-25".parse().unwrap(),
    )?;
    graph.create_block(Some(day), None, "Wrote the exporter")?;
    graph.commit();

    let tmp = tempfile::tempdir()?;
    let out = tmp.path().join("out");
    assert_eq!(export_dir(&graph, &out)?, 4);
    assert!(out.join("quiddity.json").is_file());
    // Names differing only in case get distinct files, for
    // case-insensitive file systems.
    assert!(out.join("entities/Jane.md").is_file());
    assert!(out.join("entities/jane 2.md").is_file());
    assert!(out.join("entities/Notes_ misc_other.md").is_file());
    assert_eq!(
        fs::read_to_string(out.join("journals/2026-10-25.md"))?,
        "title:: 2026-10-25\ntype:: journal\ndate:: 2026-10-25\n\n- Wrote the exporter\n"
    );
    assert!(export_dir(&graph, &out).is_err(), "refuses to overwrite");
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
