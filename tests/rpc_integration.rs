//! Integration test for iroh RPC client → daemon communication
//!
//! Prerequisites: `clanker-router serve` must be running with a valid daemon.json.
//! Skip this test in CI by checking for daemon.json first.

use clanker_router::rpc::client::RpcClient;
use clanker_router::rpc::daemon::DaemonInfo;

#[tokio::test]
async fn test_rpc_status() {
    let info_path = clanker_router::rpc::daemon::daemon_info_path();
    let info = match DaemonInfo::load(&info_path) {
        Some(i) if i.is_alive() => i,
        _ => {
            eprintln!("SKIP: no running daemon (start with `clanker-router serve`)");
            return;
        }
    };

    eprintln!("Connecting to daemon node {}...", &info.node_id[..12]);
    let client = RpcClient::connect_with_addrs(&info.node_id, &info.addrs).await.expect("connect failed");

    // Ping
    assert!(client.ping().await, "ping failed");
    eprintln!("Ping OK");

    // Status
    let status = client.status().await.expect("status failed");
    eprintln!("Status: {}", status);
    assert_eq!(status["status"], "running");
    assert!(status["model_count"].as_u64().unwrap() > 0);

    // Models
    let models = client.list_models().await.expect("list_models failed");
    eprintln!("Models: {}", models.len());
    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.provider == "anthropic"));
}
