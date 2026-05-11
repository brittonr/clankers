use crate::RuntimeError;

pub(crate) fn validate_public_runtime_boundary() -> Result<(), RuntimeError> {
    // Runtime check complements compile-level tests and documents the stable deny list.
    let denied = ["DaemonEvent", "SessionCommand", "Tui", "Acp", "Mcp", "Cli"];
    for item in denied {
        if public_type_names().iter().any(|name| name.contains(item)) {
            return Err(RuntimeError::PublicBoundaryLeak(item.to_string()));
        }
    }
    Ok(())
}

pub(crate) fn public_type_names() -> Vec<&'static str> {
    vec![
        "RuntimeBuilder",
        "Runtime",
        "SessionHandle",
        "SessionEvent",
        "PromptInput",
        "PromptReceipt",
        "EventMetadata",
        "RuntimeServices",
        "PromptAssembler",
        "PromptAssemblyPolicy",
        "ToolCatalog",
        "ToolDescriptor",
        "ConfirmationBroker",
        "ConfirmationRequest",
        "ConfirmationDecision",
    ]
}
