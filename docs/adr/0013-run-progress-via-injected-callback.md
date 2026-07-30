# Run progress reported via an injected callback, not a direct AppHandle

`collect_and_store` (`src-tauri/src/collect.rs`) is deliberately generic over `C: BitbucketClient`
so it stays free of the Tauri runtime and is fully exercised by `cargo test` against a
`FakeBitbucketClient` — its own doc comment states this is where "all meaningful logic lives,"
with `run_now` as a thin wrapper. Reporting live per-repo progress ("Fetching: repo-a (2/12)") to
the frontend needs a way to push events out of that loop, and the obvious mechanism —
`tauri::AppHandle::emit` — would require passing an `AppHandle` into `collect_and_store` directly.

We instead add a `progress: &mut impl FnMut(RunProgress)` parameter (`RunProgress { index, total,
repo }`, 1-indexed, one call per repo right before that repo's fetches start; a repo's own and its
Project's fetches are folded into that single tick, never reported separately). `run_now` passes a
closure that calls `app_handle.emit("run-progress", ...)`; `cargo test` passes a closure that
collects into a `Vec` so tests can assert the exact progress sequence, the same pattern already
used for `FakeBitbucketClient`. This keeps `collect_and_store` Tauri-free and keeps progress
reporting covered by the same unit-test suite as the rest of the Run, at the cost of one extra
parameter threaded through the function and its tests.
