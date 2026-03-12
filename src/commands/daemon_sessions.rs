//! CLI commands for interacting with a running daemon via the control socket.

use clankers_controller::transport::control_socket_path;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame;
use tokio::net::UnixStream;

use crate::cli::DaemonSessionAction;
use crate::error::Result;

pub async fn run(action: DaemonSessionAction) -> Result<()> {
    match action {
        DaemonSessionAction::List => {
            let resp = send_control(ControlCommand::ListSessions).await?;
            match resp {
                ControlResponse::Sessions(sessions) => {
                    if sessions.is_empty() {
                        println!("No active sessions.");
                    } else {
                        println!("{:<24} {:<20} {:>5} {:>8} LAST ACTIVE", "SESSION", "MODEL", "TURNS", "CLIENTS");
                        for s in &sessions {
                            println!(
                                "{:<24} {:<20} {:>5} {:>8} {}",
                                s.session_id, s.model, s.turn_count, s.client_count, s.last_active
                            );
                        }
                        println!("\n{} session(s)", sessions.len());
                    }
                }
                ControlResponse::Error { message } => {
                    eprintln!("Error: {message}");
                }
                other => {
                    eprintln!("Unexpected response: {other:?}");
                }
            }
        }

        DaemonSessionAction::Status => {
            let resp = send_control(ControlCommand::Status).await?;
            match resp {
                ControlResponse::Status(status) => {
                    println!("Daemon status:");
                    println!("  PID:      {}", status.pid);
                    println!("  Uptime:   {:.0}s", status.uptime_secs);
                    println!("  Sessions: {}", status.session_count);
                    println!("  Clients:  {}", status.total_clients);
                }
                ControlResponse::Error { message } => {
                    eprintln!("Error: {message}");
                }
                other => {
                    eprintln!("Unexpected response: {other:?}");
                }
            }
        }

        DaemonSessionAction::Create { model, system_prompt } => {
            let resp = send_control(ControlCommand::CreateSession {
                model,
                system_prompt,
                token: None,
            })
            .await?;
            match resp {
                ControlResponse::Created {
                    session_id,
                    socket_path,
                } => {
                    println!("Created session: {session_id}");
                    println!("Socket: {socket_path}");
                }
                ControlResponse::Error { message } => {
                    eprintln!("Error: {message}");
                }
                other => {
                    eprintln!("Unexpected response: {other:?}");
                }
            }
        }

        DaemonSessionAction::Kill { session_id } => {
            let resp = send_control(ControlCommand::KillSession { session_id }).await?;
            match resp {
                ControlResponse::Killed => {
                    println!("Session killed.");
                }
                ControlResponse::Error { message } => {
                    eprintln!("Error: {message}");
                }
                other => {
                    eprintln!("Unexpected response: {other:?}");
                }
            }
        }

        DaemonSessionAction::Shutdown => {
            let resp = send_control(ControlCommand::Shutdown).await?;
            match resp {
                ControlResponse::ShuttingDown => {
                    println!("Daemon shutting down.");
                }
                ControlResponse::Error { message } => {
                    eprintln!("Error: {message}");
                }
                other => {
                    eprintln!("Unexpected response: {other:?}");
                }
            }
        }
    }

    Ok(())
}

/// `clankers ps` — compact session listing (docker-ps style).
pub async fn run_ps(show_all: bool) -> Result<()> {
    let resp = send_control(ControlCommand::ListSessions).await?;
    match resp {
        ControlResponse::Sessions(sessions) => {
            if sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }
            if show_all {
                println!(
                    "{:<10} {:<28} {:>5} {:>7} {:<20} SOCKET",
                    "SESSION", "MODEL", "TURNS", "CLIENTS", "LAST ACTIVE"
                );
                for s in &sessions {
                    let sid = if s.session_id.len() > 8 {
                        &s.session_id[..8]
                    } else {
                        &s.session_id
                    };
                    let model = if s.model.len() > 26 {
                        format!("{}…", &s.model[..25])
                    } else {
                        s.model.clone()
                    };
                    println!(
                        "{:<10} {:<28} {:>5} {:>7} {:<20} {}",
                        sid, model, s.turn_count, s.client_count, s.last_active, s.socket_path
                    );
                }
            } else {
                println!(
                    "{:<10} {:<28} {:>5} {:>7} LAST ACTIVE",
                    "SESSION", "MODEL", "TURNS", "CLIENTS"
                );
                for s in &sessions {
                    let sid = if s.session_id.len() > 8 {
                        &s.session_id[..8]
                    } else {
                        &s.session_id
                    };
                    let model = if s.model.len() > 26 {
                        format!("{}…", &s.model[..25])
                    } else {
                        s.model.clone()
                    };
                    println!(
                        "{:<10} {:<28} {:>5} {:>7} {}",
                        sid, model, s.turn_count, s.client_count, s.last_active
                    );
                }
            }
            println!("{} session(s)", sessions.len());
        }
        ControlResponse::Error { message } => {
            eprintln!("Error: {message}");
        }
        other => {
            eprintln!("Unexpected response: {other:?}");
        }
    }
    Ok(())
}

/// Send a control command to the daemon and return the response.
async fn send_control(cmd: ControlCommand) -> Result<ControlResponse> {
    let path = control_socket_path();
    let stream = UnixStream::connect(&path).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!(
                "Cannot connect to daemon at {}: {e}\nIs the daemon running? Start with: clankers daemon",
                path.display()
            ),
        }
    })?;

    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(&mut writer, &cmd).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to send command: {e}"),
        }
    })?;

    let resp: ControlResponse = frame::read_frame(&mut reader).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to read response: {e}"),
        }
    })?;

    Ok(resp)
}
