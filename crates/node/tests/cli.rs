//! The `quiddity-node` binary end to end: import, serve, sync, export.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

fn node(graph: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_quiddity-node"));
    command.arg("--graph").arg(graph);
    command
}

/// Runs to completion, returning stdout; panics with stderr on failure.
fn run(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Every file under `dir`, by relative path.
fn files(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for sub in ["entities", "journals"] {
        for entry in fs::read_dir(dir.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            let name = format!("{sub}/{}", path.file_name().unwrap().to_string_lossy());
            out.insert(name, fs::read_to_string(path).unwrap());
        }
    }
    out
}

#[test]
fn import_serve_sync_export() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        tmp.path().join("a"),
        tmp.path().join("b"),
        tmp.path().join("c"),
    );

    let logseq = tmp.path().join("logseq-graph");
    fs::create_dir_all(logseq.join("pages")).unwrap();
    fs::create_dir_all(logseq.join("journals")).unwrap();
    fs::write(
        logseq.join("pages/Jane.md"),
        "alias:: JD\n\n- A person\n\t- with a child\n",
    )
    .unwrap();
    fs::write(logseq.join("journals/2026_10_25.md"), "- Met [[JD]]\n").unwrap();

    let a_id = run(node(&a).arg("init"));
    assert_eq!(
        run(node(&a).arg("import").arg(&logseq)),
        "imported 2 entities"
    );
    let b_id = run(node(&b).args(["init", "--blank"]));
    run(node(&c).args(["init", "--blank"]));

    let mut server = node(&a)
        .args(["serve", "--allow", &b_id, "--local", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line.trim().rsplit(' ').next().unwrap().to_owned();
    assert!(line.starts_with(&format!("serving {a_id} at ")), "{line}");

    let sync = |graph: &Path| {
        node(graph)
            .args(["sync", &a_id, "--addr", &addr, "--local", "127.0.0.1:0"])
            .output()
            .unwrap()
    };
    assert!(sync(&b).status.success());
    assert!(!sync(&c).status.success(), "c isn't on the allow list");
    server.kill().unwrap();
    server.wait().unwrap();

    for (graph, out) in [(&a, "a-out"), (&b, "b-out"), (&c, "c-out")] {
        run(node(graph).arg("export").arg(tmp.path().join(out)));
    }
    let exported = files(&tmp.path().join("a-out"));
    assert_eq!(exported.len(), 2);
    assert_eq!(files(&tmp.path().join("b-out")), exported);
    assert!(files(&tmp.path().join("c-out")).is_empty());
}
