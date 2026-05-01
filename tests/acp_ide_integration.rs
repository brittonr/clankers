use serde_json::Value;

#[test]
fn acp_initialize_primary_path_reports_prompt_capability() {
    let line = r#"{"id":"init-1","method":"initialize","params":{}}"#;
    let (response, metadata) = clankers::modes::acp::handle_json_line_with_metadata(line).unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["id"], "init-1");
    assert_eq!(value["result"]["server"], "clankers");
    assert_eq!(value["result"]["capabilities"]["sessions"], true);
    assert_eq!(value["result"]["capabilities"]["prompts"], true);
    assert_eq!(value["result"]["capabilities"]["terminals"], false);
    assert_eq!(metadata["source"], "acp_ide_integration");
    assert_eq!(metadata["transport"], "stdio");
    assert_eq!(metadata["method"], "initialize");
    assert_eq!(metadata["status"], "ok");
}

#[test]
fn acp_unsupported_method_returns_structured_failure() {
    let line = r#"{"id":"bad-1","method":"workspace/openRemote","params":{"token":"should-not-log"}}"#;
    let (response, metadata) = clankers::modes::acp::handle_json_line_with_metadata(line).unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["id"], "bad-1");
    assert_eq!(value["error"]["code"], -32004);
    assert_eq!(value["error"]["data"]["source"], "acp_ide_integration");
    assert_eq!(value["error"]["data"]["method"], "workspace/openRemote");
    assert_eq!(value["error"]["data"]["status"], "unsupported");
    assert_eq!(metadata["method"], "workspace/openRemote");
    assert_eq!(metadata["status"], "error");
    assert!(!metadata.to_string().contains("should-not-log"));
}
