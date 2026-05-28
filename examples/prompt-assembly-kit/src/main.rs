use std::future::Future;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;

use clankers_runtime::ContextReferenceKind;
use clankers_runtime::ContextReferenceRequest;
use clankers_runtime::HostContext;
use clankers_runtime::PromptAssembler;
use clankers_runtime::PromptAssemblyPolicy;
use clankers_runtime::PromptInput;
use clankers_runtime::PromptSources;
use clankers_runtime::RuntimeBuilder;
use clankers_runtime::RuntimeError;
use clankers_runtime::safe_event_summary;

fn main() {
    let assembled = assemble_host_context_only_prompt();
    assert_eq!(assembled.sections.len(), 3);
    assert_eq!(assembled.sections[0].label, "product_policy");
    assert_eq!(assembled.sections[1].label, "secret_fixture");
    assert_eq!(assembled.sections[1].content, "[REDACTED]");
    assert_eq!(assembled.sections[2].label, "system");
    assert_eq!(assembled.sections[2].content, "[REDACTED]");
    assert!(!assembled.context_references_enabled);
    assert_eq!(assembled.unsupported_context_references.len(), 1);
    assert_eq!(assembled.unsupported_context_references[0].kind, ContextReferenceKind::File);

    let evidence = serde_json::to_string_pretty(&assembled).expect("assembled prompt serializes");
    assert_safe_evidence(&evidence);

    let digest = blake3::hash(evidence.as_bytes());
    println!("prompt-assembly-kit receipt_hash={digest}");
    println!("prompt-assembly-kit sections={}", assembled.sections.len());
    println!("prompt-assembly-kit unsupported_context_refs={}", assembled.unsupported_context_references.len());

    assert_rejects_ambient_filesystem_discovery();
    assert_runtime_facade_uses_engine_host_path();
    println!("prompt-assembly-kit passed");
}

fn assemble_host_context_only_prompt() -> clankers_runtime::AssembledPrompt {
    let policy = PromptAssemblyPolicy::host_context_only();
    let sources = PromptSources {
        system_prompt: Some("system token should be redacted".to_string()),
        host_context: vec![
            HostContext {
                label: "product_policy".to_string(),
                content: "Only answer from product-owned context.".to_string(),
            },
            HostContext {
                label: "secret_fixture".to_string(),
                content: "authorization token should not leak".to_string(),
            },
        ],
        filesystem_context_requested: false,
        context_references: vec![ContextReferenceRequest::new("fixture.txt", ContextReferenceKind::File)],
        ..PromptSources::default()
    };
    PromptAssembler::assemble(&policy, &sources, "Summarize product policy.".to_string())
        .expect("host-context-only prompt assembles")
}

fn assert_rejects_ambient_filesystem_discovery() {
    let policy = PromptAssemblyPolicy::host_context_only();
    let sources = PromptSources {
        filesystem_context_requested: true,
        ..PromptSources::default()
    };
    let err = PromptAssembler::assemble(&policy, &sources, "read ambient files".to_string())
        .expect_err("filesystem discovery is denied");
    assert_eq!(err, RuntimeError::FilesystemDiscoveryDisabled);
}

fn assert_runtime_facade_uses_engine_host_path() {
    let policy = PromptAssemblyPolicy::host_context_only();
    let sources = PromptSources {
        system_prompt: Some("runtime prompt kit system".to_string()),
        host_context: vec![HostContext {
            label: "runtime_context".to_string(),
            content: "runtime facade should route through engine-host".to_string(),
        }],
        filesystem_context_requested: false,
        context_references: Vec::new(),
        ..PromptSources::default()
    };
    let kinds = block_on(async move {
        let runtime = RuntimeBuilder::new().prompt_assembly(policy, sources).build().expect("runtime builds");
        let session = runtime.create_session(Default::default()).await.expect("session creates");
        let mut events = session.take_events().await.expect("event stream available");
        session.submit_prompt(PromptInput::new("exercise runtime facade")).await.expect("prompt runs");
        let mut kinds = Vec::new();
        for _ in 0..4 {
            kinds.push(safe_event_summary(&events.recv().await.expect("event"))["type"].as_str().unwrap().to_string());
        }
        kinds
    });
    assert_eq!(kinds, vec!["prompt_accepted", "assistant_delta", "cost_updated", "completed"]);
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct NoopWaker;
    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn prompt_assembly_kit_smoke() {
        super::main();
    }
}

fn assert_safe_evidence(evidence: &str) {
    for forbidden in [
        "authorization token",
        "system token",
        "should not leak",
        "should be redacted",
        "API_KEY",
        "Authorization:",
    ] {
        assert!(!evidence.contains(forbidden), "prompt assembly evidence leaked forbidden marker: {forbidden}");
    }
    assert!(evidence.contains("host:product_policy"));
    assert!(evidence.contains("context references disabled by host policy"));
}
