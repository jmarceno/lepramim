/// MPRIS stub (would use zbus for D-Bus MPRIS2).
/// Defines MprisAdapter that logs and does nothing.
pub struct MprisAdapter {
    pub connected: bool,
}

impl MprisAdapter {
    pub fn new() -> Self {
        Self { connected: false }
    }

    pub async fn connect(&mut self) -> Result<(), String> {
        tracing::info!("MPRIS2 stub connect (no dbus)");
        // Simulate success but no real connection
        self.connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        tracing::info!("MPRIS disconnect");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn mpris_stub() {
        let mut m = MprisAdapter::new();
        assert!(m.connect().await.is_ok());
        m.disconnect();
        assert!(!m.connected);
    }
}
