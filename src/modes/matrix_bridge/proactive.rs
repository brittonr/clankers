//! Proactive agent features: heartbeats and trigger pipes.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use super::prompt::run_proactive_prompt;
use crate::modes::daemon::SessionKey;
use crate::modes::daemon::SessionStore;

/// Check whether a response signals "nothing to report".
pub(crate) fn is_heartbeat_ok(response: &str) -> bool {
    let upper = response.to_uppercase();
    upper.contains("HEARTBEAT_OK") || upper.contains("HEARTBEAT OK")
}

/// Ensure a trigger pipe reader is running for a Matrix session.
/// No-op if the session already has one or the key is not Matrix.
pub(crate) async fn ensure_trigger_pipe(
    store: Arc<RwLock<SessionStore>>,
    key: &SessionKey,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) {
    if key.matrix_room_id().is_none() {
        return;
    }
    let needs_spawn = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.trigger_cancel.is_none(),
            None => false,
        }
    };

    if !needs_spawn {
        return;
    }

    let session_dir = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.session_dir.clone(),
            None => return,
        }
    };

    let cancel = spawn_trigger_reader(&session_dir, key.clone(), Arc::clone(&store), matrix_client);

    if let Some(cancel) = cancel {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(key) {
            session.trigger_cancel = Some(cancel);
        }
    }
}

/// Run the per-session heartbeat scheduler.
///
/// Iterates all active Matrix sessions, checks for HEARTBEAT.md,
/// and prompts the agent if the file is non-empty. Responses
/// containing "HEARTBEAT_OK" are suppressed.
pub(crate) async fn run_session_heartbeat(
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
    interval: std::time::Duration,
    heartbeat_prompt: String,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await; // skip the immediate first tick

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            () = cancel.cancelled() => break,
        }

        // Snapshot all Matrix sessions that have a HEARTBEAT.md
        let targets: Vec<(SessionKey, PathBuf, String)> = {
            let store = store.read().await;
            store
                .sessions
                .iter()
                .filter_map(|(key, session)| {
                    let room_id = key.matrix_room_id()?.to_string();
                    let hb_path = session.session_dir.join("HEARTBEAT.md");
                    Some((key.clone(), hb_path, room_id))
                })
                .collect()
        };

        for (key, hb_path, room_id) in targets {
            // Read heartbeat file
            let contents = match tokio::fs::read_to_string(&hb_path).await {
                Ok(c) if !c.trim().is_empty() => c,
                _ => continue, // missing or empty — skip
            };

            info!("[{}] heartbeat: found {} bytes in HEARTBEAT.md", key, contents.len());

            let prompt = format!("{}\n\n---\n\n{}", heartbeat_prompt, contents);

            // Start typing
            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                Ok(rid) => rid.clone(),
                Err(_) => continue,
            };
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, true).await;
            }

            // Prompt the agent (without updating last_active)
            let response = run_proactive_prompt(Arc::clone(&store), key.clone(), prompt).await;

            // Stop typing
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, false).await;
            }

            // Suppress HEARTBEAT_OK responses
            if is_heartbeat_ok(&response) {
                info!("[{}] heartbeat: OK (suppressed)", key);
                continue;
            }

            if response.trim().is_empty() {
                info!("[{}] heartbeat: empty response (suppressed)", key);
                continue;
            }

            // Send response to Matrix
            let c = matrix_client.read().await;
            if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                error!("[{}] heartbeat send failed: {e}", key);
            } else {
                info!("[{}] heartbeat: sent response ({} bytes)", key, response.len());
            }
        }
    }
}

/// Create a named pipe (FIFO) at the given path.
/// Returns Ok(()) if created or already exists, Err on failure.
pub(crate) fn create_fifo(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    let c_path = std::ffi::CString::new(path.to_string_lossy().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // Mode 0o660: owner+group read/write
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o660) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Spawn a trigger pipe reader task for a session.
///
/// Creates a FIFO at `{session_dir}/trigger.pipe` and reads lines from it.
/// Each line becomes a prompt to the agent; responses go to the Matrix room.
/// Returns the cancellation token used to stop the reader.
pub(crate) fn spawn_trigger_reader(
    session_dir: &Path,
    key: SessionKey,
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) -> Option<CancellationToken> {
    let room_id = key.matrix_room_id()?.to_string();
    let pipe_path = session_dir.join("trigger.pipe");

    if let Err(e) = create_fifo(&pipe_path) {
        error!("[{}] failed to create trigger pipe {}: {e}", key, pipe_path.display());
        return None;
    }

    info!("[{}] trigger pipe: {}", key, pipe_path.display());

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;

        loop {
            // Open the FIFO — this blocks until a writer opens the other end.
            // When the writer closes, we get EOF and re-open.
            let file = tokio::select! {
                f = tokio::fs::File::open(&pipe_path) => {
                    match f {
                        Ok(f) => f,
                        Err(e) => {
                            warn!("[{}] trigger pipe open failed: {e}", key);
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                () = cancel_clone.cancelled() => break,
            };

            let reader = tokio::io::BufReader::new(file);
            let mut lines = reader.lines();

            loop {
                let line = tokio::select! {
                    l = lines.next_line() => l,
                    () = cancel_clone.cancelled() => break,
                };

                match line {
                    Ok(Some(text)) if !text.trim().is_empty() => {
                        info!("[{}] trigger: {}", key, &text[..80.min(text.len())]);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        // Typing indicator
                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, true).await;
                        }

                        let response = run_proactive_prompt(Arc::clone(&store), key.clone(), text).await;

                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        if is_heartbeat_ok(&response) || response.trim().is_empty() {
                            info!("[{}] trigger: suppressed (ok/empty)", key);
                            continue;
                        }

                        let c = matrix_client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("[{}] trigger send failed: {e}", key);
                        }
                    }
                    Ok(Some(_)) => {}  // empty line, skip
                    Ok(None) => break, // EOF — writer closed, re-open
                    Err(e) => {
                        warn!("[{}] trigger pipe read error: {e}", key);
                        break;
                    }
                }
            }
        }
    });

    Some(cancel)
}
