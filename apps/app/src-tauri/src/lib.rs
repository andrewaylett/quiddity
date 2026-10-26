//! The Tauri shell: opens the graph, hands it to the UI, and keeps the UI's
//! edits safe.
//!
//! The UI holds its own replica of the graph (the core compiled to WASM), so
//! typing never waits on IPC. These commands carry the graph in as a snapshot
//! and the edits back as updates, both as raw bytes.

mod session;

use std::path::PathBuf;
use std::sync::Mutex;

use session::{Error, Session};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Manager, State};

type Shared<'a> = State<'a, Mutex<Session>>;

/// The whole graph, for the UI to start from.
///
/// Commands are `async` so they run off the main thread. The lock is never
/// held across an await point, so a poisoned lock is the only way to fail
/// here, and that is a bug worth crashing for.
#[tauri::command]
async fn load_graph(state: Shared<'_>) -> Result<Response, Error> {
    let session = state.lock().expect("session lock poisoned");
    Ok(Response::new(session.snapshot()?))
}

/// Updates from the UI's edits, as the request body.
#[tauri::command]
async fn push_updates(state: Shared<'_>, request: Request<'_>) -> Result<(), Error> {
    let InvokeBody::Raw(updates) = request.body() else {
        return Err(Error::NotBytes);
    };
    state.lock().expect("session lock poisoned").apply(updates)
}

/// `QUIDDITY_GRAPH` names the graph's directory, for working on a graph made
/// with `quiddity-node`; otherwise there is one graph in the app's data
/// directory.
fn graph_dir(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match std::env::var_os("QUIDDITY_GRAPH") {
        Some(dir) => Ok(dir.into()),
        None => Ok(app.path().app_data_dir()?.join("graph")),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Opens links from the text of a block in the system's browser.
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(Mutex::new(Session::open(&graph_dir(app)?)?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![load_graph, push_updates])
        .run(tauri::generate_context!())
        .expect("error while running the Quiddity app");
}
