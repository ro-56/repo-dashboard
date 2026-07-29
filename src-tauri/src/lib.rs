pub mod client;
pub mod collect;
pub mod credentials;
pub mod diff;
pub mod model;
pub mod normalize;
pub mod roster;
pub mod storage;

use tauri::Manager;

use client::RealBitbucketClient;
use collect::{collect_and_store, RunError};
use credentials::{Credentials, CredentialsState};

/// A `tokio::sync::Mutex`, not `std::sync::Mutex` — `run_now` holds this guard across
/// `.await` points inside `collect_and_store`, and only an async-aware guard is `Send`.
pub struct DbState(pub tokio::sync::Mutex<rusqlite::Connection>);

/// Stores/replaces the workspace credential in memory only. Never written to disk, never
/// returned to the frontend by this or any other command.
#[tauri::command]
fn set_credentials(
    state: tauri::State<CredentialsState>,
    username: String,
    app_password: String,
    workspace: String,
) {
    *state.0.lock().unwrap() = Some(Credentials { username, app_password, workspace });
}

/// Thin wrapper around `collect_and_store` — all meaningful logic lives there and is covered
/// by `cargo test`; this command only wires managed state to it and stringifies the error for
/// the frontend.
#[tauri::command]
async fn run_now(
    credentials: tauri::State<'_, CredentialsState>,
    db: tauri::State<'_, DbState>,
) -> Result<i64, String> {
    let creds = credentials
        .0
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "no credentials set".to_string())?;

    let client = RealBitbucketClient::new(creds.username, creds.app_password);
    let run_at = chrono::Utc::now().to_rfc3339();

    let mut conn = db.0.lock().await;
    collect_and_store(&client, &mut conn, &creds.workspace, &run_at)
        .await
        .map_err(|e| match e {
            RunError::CredentialRejected => "credential rejected".to_string(),
            RunError::DiscoveryFailed(msg) => format!("could not reach Bitbucket: {msg}"),
            RunError::StorageFailed(msg) => format!("could not save run: {msg}"),
        })
}

/// Populates the two run selectors: every Snapshot's id + run_at, newest first.
#[tauri::command]
async fn list_snapshots(db: tauri::State<'_, DbState>) -> Result<Vec<storage::SnapshotSummary>, String> {
    let conn = db.0.lock().await;
    storage::list_snapshots(&conn).map_err(|e| e.to_string())
}

/// Thin wrapper around `roster::get_roster_tree` — all grouping/diff logic lives there and is
/// covered by `cargo test`; this command only wires managed state to it.
#[tauri::command]
async fn get_roster_tree(
    db: tauri::State<'_, DbState>,
    snapshot_a_id: i64,
    snapshot_b_id: i64,
) -> Result<roster::RosterTreeResult, String> {
    let conn = db.0.lock().await;
    roster::get_roster_tree(&conn, snapshot_a_id, snapshot_b_id).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(CredentialsState::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = rusqlite::Connection::open(data_dir.join("repo-dashboard.sqlite3"))?;
            storage::init_schema(&conn)?;
            app.manage(DbState(tokio::sync::Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_credentials,
            run_now,
            list_snapshots,
            get_roster_tree
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
