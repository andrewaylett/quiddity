//! The Tauri shell. For now it only hosts the UI; the core, and the IPC that
//! carries Loro updates and queries to it, arrive in a later step.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running the Quiddity app");
}
