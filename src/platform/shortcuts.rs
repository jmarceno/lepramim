/// Global shortcuts stub (would use zbus/ashpd for XDG portal).
/// Defines structures and try_register that logs and returns false.
pub struct ShortcutsAdapter {
    pub registered: bool,
}

impl ShortcutsAdapter {
    pub fn new() -> Self {
        Self { registered: false }
    }

    pub async fn try_register(&mut self) -> bool {
        tracing::info!("GlobalShortcuts portal unavailable (stub)");
        false
    }

    pub fn disconnect(&mut self) {
        tracing::info!("ShortcutsAdapter disconnect");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stub_register_false() {
        let mut a = ShortcutsAdapter::new();
        assert!(!a.try_register().await);
    }
}
