use crate::error::{AppError, AppResult};
use std::sync::Mutex;
use tauri::{App, AppHandle, Manager, menu::MenuBuilder, tray::TrayIconBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct ShortcutRegistration(pub Mutex<Option<String>>);

pub fn setup(app: &mut App, default_shortcut: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let handle = app.handle().clone();
    let menu = MenuBuilder::new(app)
        .text("open", "打开 DualTranslation")
        .text("settings", "设置")
        .separator()
        .text("quit", "退出")
        .build()?;
    let mut tray_builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("DualTranslation · 二元编译")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let _ = show_main_window(app);
            }
            "settings" => {
                let _ = show_main_window(app);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.eval(
                        "window.dispatchEvent(new CustomEvent('dualtranslation:navigate-settings'))",
                    );
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon).icon_as_template(false);
    }
    tray_builder.build(app)?;

    if let Err(error) = replace_shortcut(&handle, default_shortcut) {
        eprintln!("DualTranslation shortcut unavailable: {}", error.code);
    }
    Ok(())
}

pub fn replace_shortcut(app: &AppHandle, shortcut: &str) -> AppResult<()> {
    let state = app.state::<ShortcutRegistration>();
    let previous = state.0.lock().map_err(AppError::internal)?.clone();

    if let Some(previous) = &previous {
        app.global_shortcut()
            .unregister(previous.as_str())
            .map_err(|_| shortcut_error())?;
    }

    let candidate = shortcut.to_owned();
    let registration = app
        .global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = toggle_main_window(app);
            }
        });

    if registration.is_err() {
        if let Some(previous) = previous {
            let _ = app.global_shortcut().on_shortcut(
                previous.as_str(),
                move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = toggle_main_window(app);
                    }
                },
            );
        }
        return Err(shortcut_error());
    }

    *state.0.lock().map_err(AppError::internal)? = Some(candidate);
    Ok(())
}

pub fn unregister_shortcut(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<ShortcutRegistration>();
    if let Some(shortcut) = state.0.lock().map_err(AppError::internal)?.take() {
        app.global_shortcut()
            .unregister(shortcut.as_str())
            .map_err(|_| shortcut_error())?;
    }
    Ok(())
}

pub fn show_main_window(app: &AppHandle) -> AppResult<()> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        AppError::new(
            "INTERNAL_ERROR",
            "主窗口不可用。",
            "请从托盘退出后重新启动应用。",
        )
    })?;
    window.unminimize().map_err(AppError::internal)?;
    window.show().map_err(AppError::internal)?;
    window.set_focus().map_err(AppError::internal)?;
    let _ = window.eval("window.dispatchEvent(new CustomEvent('dualtranslation:focus-input'))");
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> AppResult<()> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        AppError::new(
            "INTERNAL_ERROR",
            "主窗口不可用。",
            "请从托盘退出后重新启动应用。",
        )
    })?;
    window.hide().map_err(AppError::internal)
}

pub fn toggle_main_window(app: &AppHandle) -> AppResult<()> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        AppError::new(
            "INTERNAL_ERROR",
            "主窗口不可用。",
            "请从托盘退出后重新启动应用。",
        )
    })?;
    if window.is_visible().map_err(AppError::internal)? {
        window.hide().map_err(AppError::internal)
    } else {
        show_main_window(app)
    }
}

fn shortcut_error() -> AppError {
    AppError::new(
        "SHORTCUT_CONFLICT",
        "全局快捷键注册失败，可能已被其他应用占用。",
        "请换一个快捷键；托盘入口仍可使用。",
    )
}
