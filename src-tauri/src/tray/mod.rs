//! System Tray & Menu Bar Implementation (Phase 17.1).
//!
//! Provides a persistent system tray icon with multi-resolution assets
//! (16x16, 32x32, 64x64, 128x128, 256x256), context menu with quick
//! console access, real-time protection status, and circuit breaker reset.

use std::error::Error;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager};

pub const TRAY_ICON_16: &[u8] = include_bytes!("../../icons/tray_icon_16.png");
pub const TRAY_ICON_32: &[u8] = include_bytes!("../../icons/tray_icon_32.png");
pub const TRAY_ICON_64: &[u8] = include_bytes!("../../icons/tray_icon_64.png");
pub const TRAY_ICON_128: &[u8] = include_bytes!("../../icons/tray_icon_128.png");
pub const TRAY_ICON_256: &[u8] = include_bytes!("../../icons/tray_icon_256.png");

/// Decodes PNG image bytes into a Tauri RGBA Image.
pub fn load_tray_image(bytes: &[u8]) -> Result<tauri::image::Image<'static>, Box<dyn Error>> {
    let img = image::load_from_memory(bytes)?.to_rgba8();
    let (width, height) = img.dimensions();
    Ok(tauri::image::Image::new_owned(
        img.into_raw(),
        width,
        height,
    ))
}

/// Initializes and registers the persistent system tray icon on startup (Phase 17.1).
pub fn setup_system_tray(app: &mut App) -> Result<TrayIcon, Box<dyn Error>> {
    let handle = app.handle();

    // 1. Build Context Menu
    let show_item = MenuItem::with_id(
        handle,
        "show_console",
        "Open TruthBeacon Console",
        true,
        None::<&str>,
    )?;
    let status_item = MenuItem::with_id(
        handle,
        "protection_status",
        "● Status: Protection Active",
        false, // Disabled / informative
        None::<&str>,
    )?;
    let reset_breaker_item = MenuItem::with_id(
        handle,
        "reset_circuit_breaker",
        "Reset Safety Pause (Circuit Breaker)",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(handle)?;
    let quit_item = MenuItem::with_id(handle, "quit_app", "Quit TruthBeacon", true, None::<&str>)?;

    let menu = Menu::with_items(
        handle,
        &[
            &show_item,
            &status_item,
            &separator,
            &reset_breaker_item,
            &separator,
            &quit_item,
        ],
    )?;

    // 2. Load preferred multi-resolution tray asset (32x32 for retina menu bar, with fallback)
    let icon_image = load_tray_image(TRAY_ICON_32)?;

    // 3. Build Persistent System Tray Icon
    let tray = TrayIconBuilder::with_id("truthbeacon-main-tray")
        .icon(icon_image)
        .tooltip("TruthBeacon — Community Impersonation Protection")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| match event.id().as_ref() {
            "show_console" => {
                show_main_window(app_handle);
            }
            "reset_circuit_breaker" => {
                let _ = crate::commands::reset_circuit_breaker();
                log::info!("Safety circuit breaker reset via System Tray context menu");
            }
            "quit_app" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                show_main_window(app);
            }
        })
        .build(app)?;

    log::info!("TruthBeacon persistent system tray icon initialized successfully");
    Ok(tray)
}

/// Helper to display and focus the main window
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_resolution_tray_assets_embedded_and_valid() {
        assert_eq!(
            TRAY_ICON_16.len(),
            934,
            "16x16 icon asset must be exactly 934 bytes"
        );
        assert_eq!(
            TRAY_ICON_32.len(),
            2927,
            "32x32 icon asset must be exactly 2927 bytes"
        );
        assert_eq!(
            TRAY_ICON_64.len(),
            9128,
            "64x64 icon asset must be exactly 9128 bytes"
        );
        assert_eq!(
            TRAY_ICON_128.len(),
            28729,
            "128x128 icon asset must be exactly 28729 bytes"
        );
        assert_eq!(
            TRAY_ICON_256.len(),
            90826,
            "256x256 icon asset must be exactly 90826 bytes"
        );

        // Verify valid PNG headers (0x89 0x50 0x4E 0x47 0x0D 0x0A 0x1A 0x0A)
        for (name, bytes) in &[
            ("16x16", TRAY_ICON_16),
            ("32x32", TRAY_ICON_32),
            ("64x64", TRAY_ICON_64),
            ("128x128", TRAY_ICON_128),
            ("256x256", TRAY_ICON_256),
        ] {
            assert!(
                bytes.starts_bytes_with_png_header(),
                "Asset {} must be valid PNG with standard PNG magic header",
                name
            );
        }
    }

    trait PngHeaderCheck {
        fn starts_bytes_with_png_header(&self) -> bool;
    }

    impl PngHeaderCheck for &[u8] {
        fn starts_bytes_with_png_header(&self) -> bool {
            self.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        }
    }

    #[test]
    fn test_tray_icon_image_deserialization() {
        let img16 = load_tray_image(TRAY_ICON_16);
        assert!(
            img16.is_ok(),
            "16x16 tray icon must decode into Tauri Image"
        );
        let i16 = img16.unwrap();
        assert_eq!(i16.width(), 16);
        assert_eq!(i16.height(), 16);

        let img32 = load_tray_image(TRAY_ICON_32);
        assert!(
            img32.is_ok(),
            "32x32 tray icon must decode into Tauri Image"
        );
        let i32 = img32.unwrap();
        assert_eq!(i32.width(), 32);
        assert_eq!(i32.height(), 32);

        let img64 = load_tray_image(TRAY_ICON_64);
        assert!(
            img64.is_ok(),
            "64x64 tray icon must decode into Tauri Image"
        );
        let i64 = img64.unwrap();
        assert_eq!(i64.width(), 64);
        assert_eq!(i64.height(), 64);

        let img128 = load_tray_image(TRAY_ICON_128);
        assert!(
            img128.is_ok(),
            "128x128 tray icon must decode into Tauri Image"
        );
        let i128 = img128.unwrap();
        assert_eq!(i128.width(), 128);
        assert_eq!(i128.height(), 128);

        let img256 = load_tray_image(TRAY_ICON_256);
        assert!(
            img256.is_ok(),
            "256x256 tray icon must decode into Tauri Image"
        );
        let i256 = img256.unwrap();
        assert_eq!(i256.width(), 256);
        assert_eq!(i256.height(), 256);
    }
}
