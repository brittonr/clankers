//! Proactive agent features: heartbeats and trigger pipes.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_controller::transport::DaemonState;
use clankers_protocol::SessionKey;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use super::prompt::run_proactive_prompt;
use crate::modes::daemon::socket_bridge::SessionFactory;

/// Check whether a response signals "nothing to report".
pub(crate) fn is_heartbeat_ok(response: &str) -> bool {
    let upper = response.to_uppercase();
    upper.contains("HEARTBEAT_OK") || upper.contains("HEARTBEAT OK")
}

/// Ensure a trigger pipe reader is running for a Matrix session.
///
/// Creates the pipe and spawns a reader task if one doesn't exist yet.
/// Tracks whether spawned via a flag on `DaemonState` (session_dir naming).
pub(crate) async fn ensure_trigger_pipe(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    key: &SessionKey,
    sessions_dir: &Path,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) {
    if key.matrix_room_id().is_none() {
        return;
    }

    let session_dir = sessions_dir.join(key.dir_name());
    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        warn!("failed to create session dir {}: {e}", session_dir.display());
        return;
    }

    let pipe_path = session_dir.join("trigger.pipe");
    // If the pipe already exists, a reader is likely already running
    if pipe_path.exists() {
        return;
    }

    let cancel = spawn_trigger_reader(
        &session_dir,
        key.clone(),
        state,
        registry,
        factory,
        matrix_client,
    );

    if cancel.is_none() {
        warn!("failed to spawn trigger reader for {}", key);
    }
}

/// Run the per-session heartbeat scheduler.
///
/// Iterates all active Matrix sessions, checks for HEARTBEAT.md,
/// and prompts the agent if the file is non-empty. Responses
/// containing "HEARTBEAT_OK" are suppressed.
pub(crate) async fn run_session_heartbeat(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    sessions_dir: PathBuf,
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
            let st = state.lock().await;
            st.matrix_keys()
                .into_iter()
                .filter_map(|(key, _session_id)| {
                    let room_id = key.matrix_room_id()?.to_string();
                    let hb_path = sessions_dir.join(key.dir_name()).join("HEARTBEAT.md");
                    Some((key, hb_path, room_id))
                })
                .collect()
        };

        for (key, hb_path, room_id) in targets {
            let contents = match tokio::fs::read_to_string(&hb_path).await {
                Ok(c) if !c.trim().is_empty() => c,
                _ => continue,
            };

            info!("[{}] heartbeat: found {} bytes in HEARTBEAT.md", key, contents.len());

            let prompt = format!("{}\n\n---\n\n{}", heartbeat_prompt, contents);

            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                Ok(rid) => rid.clone(),
                Err(_) => continue,
            };
            {
                let c = matrix_client.read().await;
                c.set_typing(&room_id_parsed, true).await.ok();
            }

            let response = run_proactive_prompt(
                Arc::clone(&state),
                registry.clone(),
                Arc::clone(&factory),
                key.clone(),
                prompt,
            ).await;

            {
                let c = matrix_client.read().await;
                c.set_typing(&room_id_parsed, false).await.ok();
            }

            if is_heartbeat_ok(&response) {
                info!("[{}] heartbeat: OK (suppressed)", key);
                continue;
            }

            if response.trim().is_empty() {
                info!("[{}] heartbeat: empty response (suppressed)", key);
                continue;
            }

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
pub(crate) fn create_fifo(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    let c_path = std::ffi::CString::new(path.to_string_lossy().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

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
pub(crate) fn spawn_trigger_reader(
    session_dir: &Path,
    key: SessionKey,
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
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

                        {
                            let c = matrix_client.read().await;
                            c.set_typing(&room_id_parsed, true).await.ok();
                        }

                        let response = run_proactive_prompt(
                            Arc::clone(&state),
                            registry.clone(),
                            Arc::clone(&factory),
                            key.clone(),
                            text,
                        ).await;

                        {
                            let c = matrix_client.read().await;
                            c.set_typing(&room_id_parsed, false).await.ok();
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
                    Ok(Some(_)) => {}
                    Ok(None) => break,
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
