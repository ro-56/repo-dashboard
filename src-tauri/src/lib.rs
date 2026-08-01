pub mod apply;
pub mod client;
pub mod collect;
pub mod credentials;
pub mod diff;
pub mod model;
pub mod normalize;
pub mod refresh;
pub mod roster;
pub mod storage;

use tauri::{Emitter, Manager};

use apply::{ApplyResult, PendingEditRequest};
use client::RealBitbucketClient;
use collect::{collect_and_store, RunError};
use credentials::{Credentials, CredentialsState};
use refresh::RefreshError;

/// A `tokio::sync::Mutex`, not `std::sync::Mutex` — `run_now` holds this guard across
/// `.await` points inside `collect_and_store`, and only an async-aware guard is `Send`.
pub struct DbState(pub tokio::sync::Mutex<rusqlite::Connection>);

/// Stores/replaces the workspace credential in the OS keychain. Never returned to the
/// frontend by this or any other command.
#[tauri::command]
fn set_credentials(
    state: tauri::State<CredentialsState>,
    username: String,
    app_password: String,
    workspace: String,
) -> Result<(), String> {
    state.0.set(&Credentials { username, app_password, workspace })
}

/// Reports whether credentials are set, and the username/workspace if so — never the app
/// password. Lets the frontend render a "connected as X" state instead of a blank form.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsSummary {
    username: Option<String>,
    workspace: Option<String>,
    has_credentials: bool,
}

#[tauri::command]
fn get_credentials(state: tauri::State<CredentialsState>) -> Result<CredentialsSummary, String> {
    Ok(match state.0.get()? {
        Some(creds) => CredentialsSummary {
            username: Some(creds.username),
            workspace: Some(creds.workspace),
            has_credentials: true,
        },
        None => CredentialsSummary { username: None, workspace: None, has_credentials: false },
    })
}

/// Thin wrapper around `collect_and_store` — all meaningful logic lives there and is covered
/// by `cargo test`; this command only wires managed state to it and stringifies the error for
/// the frontend.
#[tauri::command]
async fn run_now(
    app_handle: tauri::AppHandle,
    credentials: tauri::State<'_, CredentialsState>,
    db: tauri::State<'_, DbState>,
) -> Result<i64, String> {
    let creds = credentials.0.get()?.ok_or_else(|| "no credentials set".to_string())?;

    let client = RealBitbucketClient::new(creds.username, creds.app_password);
    let run_at = chrono::Utc::now().to_rfc3339();

    let mut conn = db.0.lock().await;
    collect_and_store(&client, &mut conn, &creds.workspace, &run_at, &mut |progress| {
        let _ = app_handle.emit("run-progress", progress);
    })
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

/// Permanently deletes a Snapshot and its cascaded rows (ADR-0011). Thin wrapper around
/// `storage::delete_snapshot` — all logic lives there and is covered by `cargo test`.
#[tauri::command]
async fn delete_snapshot(db: tauri::State<'_, DbState>, id: i64) -> Result<(), String> {
    let mut conn = db.0.lock().await;
    storage::delete_snapshot(&mut conn, id).map_err(|e| match e {
        storage::DeleteSnapshotError::NotFound(id) => format!("no snapshot with id {id}"),
        storage::DeleteSnapshotError::Storage(msg) => format!("could not delete snapshot: {msg}"),
    })
}

/// Thin wrapper around `apply::apply_pending_edits` — all routing logic lives there and is
/// covered by `cargo test`; this command only wires managed credential state to it. Never
/// fails the whole batch on a per-item error (ADR-0022) — a missing credential is the only
/// thing that rejects the call outright, since nothing can be applied without one.
#[tauri::command]
async fn apply_pending_edits(
    credentials: tauri::State<'_, CredentialsState>,
    edits: Vec<PendingEditRequest>,
) -> Result<Vec<ApplyResult>, String> {
    let creds = credentials.0.get()?.ok_or_else(|| "no credentials set".to_string())?;
    let client = RealBitbucketClient::new(creds.username, creds.app_password);
    Ok(apply::apply_pending_edits(&client, &creds.workspace, edits).await)
}

/// Thin wrapper around `refresh::refresh_and_store` — all meaningful logic lives there and is
/// covered by `cargo test`; this command only wires managed state to it and stringifies the
/// error for the frontend.
#[tauri::command]
async fn refresh_snapshot(
    credentials: tauri::State<'_, CredentialsState>,
    db: tauri::State<'_, DbState>,
    source_snapshot_id: i64,
    results: Vec<ApplyResult>,
) -> Result<i64, String> {
    let creds = credentials.0.get()?.ok_or_else(|| "no credentials set".to_string())?;
    let client = RealBitbucketClient::new(creds.username, creds.app_password);
    let run_at = chrono::Utc::now().to_rfc3339();

    let mut conn = db.0.lock().await;
    refresh::refresh_and_store(&client, &mut conn, &creds.workspace, source_snapshot_id, &run_at, &results)
        .await
        .map_err(|e| match e {
            RefreshError::CredentialRejected => "credential rejected".to_string(),
            RefreshError::StorageFailed(msg) => format!("could not save refresh: {msg}"),
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
            get_credentials,
            run_now,
            list_snapshots,
            get_roster_tree,
            delete_snapshot,
            apply_pending_edits,
            refresh_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
