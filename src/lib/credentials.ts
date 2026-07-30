// Mirrors the JSON shape serialized by src-tauri/src/lib.rs's `CredentialsSummary` — never
// carries the app password (the backend doesn't return it).

export interface CredentialsSummary {
  username: string | null;
  workspace: string | null;
  hasCredentials: boolean;
}
