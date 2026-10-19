use tauri::{
    Emitter,
    menu::{Menu, MenuItem, MenuItemKind},
};

// Сохраняем стандартные команды ОС, заменяя только системное окно About.
pub fn install(app: &tauri::App, locale: crate::preferences::Locale) -> tauri::Result<()> {
    let menu = Menu::default(app.handle())?;
    #[cfg(target_os = "macos")]
    let parent = menu.items()?.into_iter().next();
    #[cfg(not(target_os = "macos"))]
    let parent = menu.get(tauri::menu::HELP_SUBMENU_ID);
    if let Some(MenuItemKind::Submenu(parent)) = parent {
        parent.remove_at(0)?;
        let about = MenuItem::with_id(
            app,
            "converter-about",
            crate::localization::native_text(locale, "native.about"),
            true,
            None::<&str>,
        )?;
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

pub fn set_language(app: &tauri::AppHandle, locale: crate::preferences::Locale) {
    if let Some(menu) = app.menu()
        && let Some(item) = menu.get("converter-about")
        && let Some(item) = item.as_menuitem()
        && item
            .set_text(crate::localization::native_text(locale, "native.about"))
            .is_err()
    {
        eprintln!("Could not update the About menu label");
    }
}
