/// Global shortcut registration via desktop portal / DE-specific paths.
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

    /// Try to register Meta+R → lexaloud speak-selection.
    pub async fn try_register(&mut self) -> bool {
        if std::env::var("XDG_SESSION_TYPE").ok().as_deref() == Some("wayland") {
            match ashpd::desktop::global_shortcuts::GlobalShortcuts::new().await {
                Ok(portal) => {
                    tracing::debug!(
                        "Global Shortcuts portal available; bind Meta+R to `lexaloud speak-selection` in your DE settings"
                    );
                    let _ = portal;
                }
                Err(e) => {
                    tracing::debug!("Global Shortcuts portal unavailable: {e}");
                }
            }
        }

        self.message = Some(
            "Bind Meta+R manually to: lexaloud speak-selection\n\
             GNOME: Settings → Keyboard → Custom Shortcuts\n\
             KDE: System Settings → Shortcuts → Custom Shortcuts\n\
             XFCE: Settings → Keyboard → Application Shortcuts"
                .to_string(),
        );
        tracing::info!("Shortcut setup: see `lexaloud setup` output for DE-specific instructions");
        false
    }

    pub fn disconnect(&mut self) {
        self.registered = false;
        tracing::info!("ShortcutsAdapter disconnect");
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
