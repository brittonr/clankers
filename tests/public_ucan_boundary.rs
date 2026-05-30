#[test]
fn daemon_auth_defaults_to_public_ucan_without_legacy_verifier() {
    let session_store = include_str!("../src/modes/daemon/session_store.rs");
    let quic_bridge = include_str!("../src/modes/daemon/quic_bridge.rs");
    let handlers = include_str!("../src/modes/daemon/handlers.rs");
    let token_command = include_str!("../src/commands/token.rs");

    for (name, source) in [
        ("session_store", session_store),
        ("quic_bridge", quic_bridge),
        ("handlers", handlers),
        ("token_command", token_command),
    ] {
        assert!(!source.contains("TokenVerifier"), "{name} must not construct legacy TokenVerifier");
        assert!(
            !source.contains("Credential::from_base64"),
            "{name} must decode public UCAN envelopes, not legacy credentials"
        );
        assert!(
            !source.contains("clankers_ucan::Credential"),
            "{name} must not use legacy clanker-auth credential aliases by default"
        );
    }
}

#[test]
fn daemon_entrypoints_use_shared_session_admission_request_helpers() {
    let quic_bridge = include_str!("../src/modes/daemon/quic_bridge.rs");
    let handlers = include_str!("../src/modes/daemon/handlers.rs");
    let matrix_commands = include_str!("../src/modes/matrix_bridge/bot_commands.rs");
    let matrix_bridge = include_str!("../src/modes/matrix_bridge/mod.rs");

    assert!(quic_bridge.contains("session_create_admission_request()"));
    assert!(quic_bridge.contains("session_attach_admission_request(&session_id)"));
    assert!(handlers.contains("session_prompt_admission_request(peer_id)"));
    assert!(handlers.contains("session_prompt_admission_request(&peer_id)"));
    assert!(matrix_commands.contains("session_prompt_admission_request(&user_id)"));
    assert!(matrix_bridge.contains("session_prompt_admission_request(sender)"));

    for (name, source) in [
        ("quic_bridge", quic_bridge),
        ("handlers", handlers),
        ("matrix_commands", matrix_commands),
        ("matrix_bridge", matrix_bridge),
    ] {
        assert!(
            !source.contains("BasaltAdmissionRequest::new"),
            "{name} should not hand-roll remote session admission requests"
        );
    }
}

#[test]
fn public_ucan_dependency_uses_remote_workspace_pin() {
    let crate_manifest = include_str!("../crates/clankers-ucan/Cargo.toml");
    let workspace_manifest = include_str!("../Cargo.toml");

    assert!(!crate_manifest.contains("../../../ucan"));
    assert!(crate_manifest.contains("ucan = { workspace = true }"));
    assert!(workspace_manifest.contains("git = \"ssh://git@github.com/OnixResearch/ucan.git\""));
}
