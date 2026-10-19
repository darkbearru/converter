use tauri::{
    Emitter,
    menu::{Menu, MenuItem, MenuItemKind},
};

// Сохраняем стандартные команды ОС, заменяя только системное окно About.
pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let menu = Menu::default(app.handle())?;
    #[cfg(target_os = "macos")]
    let parent = menu.items()?.into_iter().next();
    #[cfg(not(target_os = "macos"))]
    let parent = menu.get(tauri::menu::HELP_SUBMENU_ID);
    if let Some(MenuItemKind::Submenu(parent)) = parent {
        parent.remove_at(0)?;
        let about = MenuItem::with_id(app, "converter-about", "О программе", true, None::<&str>)?;
        parent.insert(&about, 0)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "converter-about" {
            let _ = app.emit("app:about", ());
        }
    });
    Ok(())
}
