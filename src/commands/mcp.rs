use std::io::BufRead;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use clankers_controller::client::ClientAdapter;
use clankers_protocol::ControlCommand;
use clankers_protocol::ControlResponse;
use clankers_protocol::SessionCommand;
use tokio::net::UnixStream;

use crate::cli::McpAction;
use crate::commands::CommandContext;
use crate::error::Result;

pub async fn run(_ctx: &CommandContext, action: McpAction) -> Result<()> {
    match action {
        McpAction::Serve { session } => run_serve(session).await,
    }
}

async fn run_serve(session: String) -> Result<()> {
    let mut client = connect_session(&session).await?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|source| crate::error::Error::Io { source })?;
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_json_line_for_client(&line, Some(session.as_str()), &mut client)?;
        tracing::info!(
            source = "mcp_session_control",
            transport = "stdio",
            session_id = session.as_str(),
            "processed MCP session-control request"
        );
        writeln!(stdout, "{response}").map_err(|source| crate::error::Error::Io { source })?;
        stdout.flush().map_err(|source| crate::error::Error::Io { source })?;
    }

    Ok(())
}

pub fn handle_json_line_for_client(line: &str, session_id: Option<&str>, client: &mut ClientAdapter) -> Result<String> {
    let mut dispatch = |cmd: SessionCommand| {
        let submitted = client.send(cmd);
        let events = drain_session_events(client);
        let disconnected = client.is_disconnected();
        crate::modes::mcp_control::McpDispatchEvidence {
            submitted,
            events,
            disconnected,
        }
    };
    crate::modes::mcp_control::handle_json_line_with_evidence_dispatch(line, session_id, &mut dispatch)
        .map_err(|source| crate::error::Error::Json { source })
}

fn drain_session_events(client: &mut ClientAdapter) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + Duration::from_millis(25);
    let mut events = Vec::new();
    loop {
        while let Some(event) = client.try_recv() {
            events.push(crate::modes::mcp_control::summarize_daemon_event(&event));
        }
        if !events.is_empty() || client.is_disconnected() || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    events
}

async fn connect_session(session_id: &str) -> Result<ClientAdapter> {
    let response = crate::modes::attach::send_control(ControlCommand::AttachSession {
        session_id: session_id.to_string(),
    })
    .await?;
    let socket_path = match response {
        ControlResponse::Attached { socket_path } => socket_path,
        ControlResponse::Error { message } => {
            return Err(crate::error::Error::Provider {
                message: format!("Failed to attach MCP session-control bridge to session {session_id}: {message}"),
            });
        }
        other => {
            return Err(crate::error::Error::Provider {
                message: format!("Unexpected daemon attach response for MCP session-control bridge: {other:?}"),
            });
        }
    };
    let stream = UnixStream::connect(&socket_path).await.map_err(|source| crate::error::Error::Io { source })?;
    ClientAdapter::connect(stream, "clankers-mcp-session-control", None, Some(session_id.to_string()))
        .await
        .map_err(|error| crate::error::Error::Provider {
            message: format!("Failed to connect MCP session-control bridge to session socket: {error}"),
        })
}
