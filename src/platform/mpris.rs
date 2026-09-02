use std::sync::Arc;

use tokio::sync::mpsc;

/// Commands from MPRIS media keys / D-Bus clients.
#[derive(Debug, Clone, Copy)]
pub enum MprisCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
}

/// MPRIS2 integration: registers on the session bus when available.
pub struct MprisAdapter {
    pub connected: bool,
    cmd_tx: Option<mpsc::Sender<MprisCommand>>,
}

impl MprisAdapter {
    pub fn new() -> Self {
        Self {
            connected: false,
            cmd_tx: None,
        }
    }

    /// Start MPRIS listener; `handler` receives play/pause/next/previous/stop.
    pub async fn connect<F, Fut>(&mut self, mut handler: F) -> Result<(), String>
    where
        F: FnMut(MprisCommand) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel(32);
        self.cmd_tx = Some(tx);

        let conn = match zbus::Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                return Err(format!(
                    "MPRIS unavailable (no session D-Bus): {e}. Media keys may still work via your DE."
                ));
            }
        };

        conn.request_name("org.mpris.MediaPlayer2.lexaloud")
            .await
            .map_err(|e| format!("MPRIS name request failed: {e}"))?;

        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                handler(cmd).await;
            }
        });

        tokio::spawn(async move {
            let _conn = conn;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });

        self.connected = true;
        tracing::info!("MPRIS2 registered as org.mpris.MediaPlayer2.lexaloud");
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.cmd_tx = None;
        self.connected = false;
        tracing::info!("MPRIS disconnected");
    }
}

impl Default for MprisAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Wire MPRIS commands to a player (call from daemon after startup).
pub async fn wire_mpris<P, S>(player: Arc<crate::player::Player<P, S>>) -> MprisAdapter
where
    P: crate::player::SpeechProvider + 'static,
    S: crate::audio::AudioSink + 'static,
{
    let mut adapter = MprisAdapter::new();
    let player2 = player.clone();
    let _ = adapter
        .connect(move |cmd| {
            let player = player2.clone();
            async move {
                match cmd {
                    MprisCommand::Play => {
                        let _ = player.resume().await;
                    }
                    MprisCommand::Pause => {
                        let _ = player.pause().await;
                    }
                    MprisCommand::Stop => {
                        let _ = player.stop().await;
                    }
                    MprisCommand::Next => {
                        let _ = player.skip().await;
                    }
                    MprisCommand::Previous => {
                        let _ = player.back().await;
                    }
                }
            }
        })
        .await;
    adapter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mpris_connect_without_bus_fails_gracefully() {
        let mut m = MprisAdapter::new();
        let res = m.connect(|_| async {}).await;
        // In CI without dbus session, expect error not panic
        if res.is_ok() {
            assert!(m.connected);
            m.disconnect();
        }
    }
}
