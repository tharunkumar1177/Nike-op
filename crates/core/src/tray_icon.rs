/// Minimal System Tray Icon Manager for Runner process
///
/// This module provides a lightweight tray icon with context menu. Runner
/// draws its quick flyout itself; this module does not own that window.
use anyhow::{anyhow, Context, Result};
use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Load application icon from the favicon.ico installed beside Runner
fn load_app_icon() -> Result<Icon> {
    let installed_icon = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("favicon.ico")))
        .filter(|path| path.exists());

    if let Some(path) = installed_icon {
        let icon_data = std::fs::read(&path).context("Failed to read favicon.ico")?;
        let img = image::load_from_memory(&icon_data).context("Failed to decode icon")?;
        let img = img.resize_exact(16, 16, image::imageops::FilterType::Lanczos3);
        let rgba = img.to_rgba8();

        return Icon::from_rgba(rgba.into_raw(), 16, 16)
            .map_err(|e| anyhow!("Failed to create icon from image: {:?}", e));
    }

    // Fallback: green square
    let icon_rgba: Vec<u8> = (0..16 * 16)
        .flat_map(|_| vec![0x00, 0xAA, 0x00, 0xFF])
        .collect();
    Icon::from_rgba(icon_rgba, 16, 16)
        .map_err(|e| anyhow!("Failed to create fallback icon: {:?}", e))
}

/// Minimal tray icon manager for Runner process: icon, tooltip, and context menu
pub struct TrayIconManager {
    #[allow(dead_code)]
    tray_icon: TrayIcon,
    active_profile: Option<String>,
    pub menu_item_settings: MenuId,
    pub menu_item_docs: MenuId,
    pub menu_item_bug_report: MenuId,
    pub menu_item_exit: MenuId,
}

impl TrayIconManager {
    /// Create a new tray icon manager
    pub fn new(active_profile: Option<String>) -> Result<Self> {
        let tooltip = if let Some(ref name) = active_profile {
            format!("Edge Optimizer - {}", name)
        } else {
            "Edge Optimizer - Inactive".to_string()
        };

        tracing::info!("Creating tray icon");

        let icon = load_app_icon()?;
        tracing::debug!("Icon loaded");

        // Create context menu (appears on right-click)
        let menu = Menu::new();
        let settings_item = MenuItem::new("Open Settings", true, None);
        let docs_item = MenuItem::new("Documentation", true, None);
        let bug_item = MenuItem::new("Report Bug", true, None);
        let separator = PredefinedMenuItem::separator();
        let exit_item = MenuItem::new("Exit", true, None);

        menu.append(&settings_item)
            .map_err(|e| anyhow!("Failed to add settings item: {}", e))?;
        menu.append(&docs_item)
            .map_err(|e| anyhow!("Failed to add docs item: {}", e))?;
        menu.append(&bug_item)
            .map_err(|e| anyhow!("Failed to add bug report item: {}", e))?;
        menu.append(&separator)
            .map_err(|e| anyhow!("Failed to add separator: {}", e))?;
        menu.append(&exit_item)
            .map_err(|e| anyhow!("Failed to add exit item: {}", e))?;

        // Store menu IDs for event handling
        let menu_item_settings = settings_item.id().clone();
        let menu_item_docs = docs_item.id().clone();
        let menu_item_bug_report = bug_item.id().clone();
        let menu_item_exit = exit_item.id().clone();

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(&tooltip)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| anyhow!("Failed to create tray icon: {}", e))?;

        tracing::info!("Tray icon created successfully with context menu");

        Ok(Self {
            tray_icon,
            active_profile,
            menu_item_settings,
            menu_item_docs,
            menu_item_bug_report,
            menu_item_exit,
        })
    }

    /// Update tooltip based on active profile
    pub fn set_active_profile(&mut self, active: Option<String>) {
        self.active_profile = active;
        let tooltip = if let Some(ref name) = self.active_profile {
            format!("Edge Optimizer - {}", name)
        } else {
            "Edge Optimizer - Inactive".to_string()
        };
        let _ = self.tray_icon.set_tooltip(Some(&tooltip));
    }

    /// Get current active profile name
    pub fn active_profile(&self) -> Option<&String> {
        self.active_profile.as_ref()
    }
}
