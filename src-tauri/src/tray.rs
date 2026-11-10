//! A small menu-bar entry point. It reuses the main window rather than opening new ones.
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "pelago:show", "Open Pelago", true, None::<&str>)?;
    let search = MenuItem::with_id(
        app,
        "pelago:search",
        "Quick open project…",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "pelago:quit", "Quit Pelago", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &search, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Pelago")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "pelago:show" | "pelago:search" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    if event.id.as_ref() == "pelago:search" {
                        let _ = window.emit("pelago:quick-open", ());
                    }
                }
            }
            "pelago:quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
