mod commands;
mod config;
mod editors;
mod git;
mod github;
mod park;
mod scan;
mod util;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    util::init_path();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // PELAGO_DATA_DIR lets contributors run an isolated instance (tests, demos).
            let data_dir = match std::env::var_os("PELAGO_DATA_DIR") {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Arc::new(commands::AppState::new(data_dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_settings,
            commands::complete_onboarding,
            commands::suggest_scan_folders,
            commands::list_projects,
            commands::project_size,
            commands::open_project,
            commands::reveal_in_finder,
            commands::open_url,
            commands::set_pinned,
            commands::set_project_editor,
            commands::list_editors,
            commands::gh_status,
            commands::gh_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
