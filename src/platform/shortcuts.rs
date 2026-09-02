/// Global shortcut registration — handled by the Iced tray app (`lexaloud app`).
///
/// When running `lexaloud app`, hotkeys are registered via D-Bus (`org.lexaloud.App`)
/// and KGlobalAccel (Meta+R / Meta+P). This adapter remains for CLI `setup` hints only.
pub struct ShortcutsAdapter {
    pub registered: bool,
    pub message: Option<String>,
}

impl ShortcutsAdapter {
    pub fn new() -> Self {
        Self {
            registered: false,
            message: None,
        }
    }

    /// Hotkeys are owned by the in-process UI; return instructions for manual binding.
    pub async fn try_register(&mut self) -> bool {
        self.message = Some(
            "Run `lexaloud` (or `lexaloud app`) for tray + global hotkeys.\n\
             Manual bind target: org.lexaloud.App SpeakSelection / Toggle\n\
             GNOME: Settings → Keyboard → Custom Shortcuts\n\
             KDE: shortcuts are registered automatically when the app is running"
                .to_string(),
        );
        tracing::info!("Global shortcuts: start `lexaloud app` for Meta+R / Meta+P");
        false
    }

    pub fn disconnect(&mut self) {
        self.registered = false;
    }
}

impl Default for ShortcutsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_returns_bool_without_panic() {
        let mut a = ShortcutsAdapter::new();
        let _ = a.try_register().await;
        assert!(a.message.is_some());
    }
}
