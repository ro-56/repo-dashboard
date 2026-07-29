//! In-memory-only credential holding (PD-5's deliberate, temporary boundary — full
//! OS-keychain-backed encryption at rest is a separate, later ticket). Held in Tauri-managed
//! state behind a `Mutex`; never written to disk, never read back out to the frontend.

use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub app_password: String,
    pub workspace: String,
}

#[derive(Default)]
pub struct CredentialsState(pub Mutex<Option<Credentials>>);
