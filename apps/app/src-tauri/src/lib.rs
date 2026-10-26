//! The Tauri shell: opens the graph and answers the UI's queries.
//!
//! The UI reads through `apps/ui/lib/tauri-store.ts`, which calls the
//! commands here. They are read-only for now (today's journal aside); editing
//! arrives with the UI's own replica.

mod session;

use std::path::PathBuf;
use std::sync::Mutex;

use session::{Entity, EntityRef, Error, Session};
use tauri::{Manager, State};

type Shared<'a> = State<'a, Mutex<Session>>;

/// Commands are `async` so they run off the main thread. The lock is never
/// held across an await point, so a poisoned lock is the only way to fail
/// here, and that is a bug worth crashing for.
fn with<T>(
    state: Shared<'_>,
    f: impl FnOnce(&mut Session) -> Result<T, Error>,
) -> Result<T, Error> {
    f(&mut state.lock().expect("session lock poisoned"))
}

#[tauri::command]
async fn favourites(state: Shared<'_>) -> Result<Vec<EntityRef>, Error> {
    with(state, |s| s.favourites())
}

#[tauri::command]
async fn entities(state: Shared<'_>) -> Result<Vec<EntityRef>, Error> {
    with(state, |s| s.entities())
}

#[tauri::command]
async fn today(state: Shared<'_>) -> Result<EntityRef, Error> {
    with(state, |s| s.today())
}

#[tauri::command]
async fn entity(state: Shared<'_>, id: String) -> Result<Entity, Error> {
    with(state, |s| s.entity(&id))
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
        .setup(|app| {
            app.manage(Mutex::new(Session::open(&graph_dir(app)?)?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            favourites, entities, today, entity
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Quiddity app");
}
