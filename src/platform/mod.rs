pub mod mpris;
pub mod notifications;
pub mod selection;
pub mod service;
pub mod shortcuts;

#[allow(unused_imports)]
pub use selection::{CaptureResult, SelectionError, read_clipboard, read_primary};
#[allow(unused_imports)]
pub use service::{generate_systemd_unit, socket_path_valid, stale_socket_cleanup};
