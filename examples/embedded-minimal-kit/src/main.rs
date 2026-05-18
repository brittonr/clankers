use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use clanker_message::Content;
use clankers_adapters::{AtomicCancellationSource, CollectingUsageObserver, MemoryEventSink, NoopRetrySleeper, ScriptedModelHost, ScriptedToolExecutor};
use clankers_engine::{EngineInput, EngineMessage, EngineMessageRole, EnginePromptSubmission, EngineState, reduce};
use clankers_engine_host::{EngineRunSeed, HostAdapters, run_engine_turn};

const MODEL: &str = "embedded-minimal";
const SESSION_ID: &str = "embedded-minimal-session";

fn main() {
    let seed = seed("hello");
    let mut model = ScriptedModelHost::new([ScriptedModelHost::completed_text("hello from minimal kit")]);
    let mut tools = ScriptedToolExecutor::default();
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    let report = block_on(run_engine_turn(seed, HostAdapters {
        model: &mut model,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }));
    assert!(report.terminal_failure().is_none());
    assert!(!model.requests().is_empty());
    assert!(!events.events().is_empty());
    println!("embedded-minimal-kit passed");
}

trait ReportExt { fn terminal_failure(&self) -> Option<&clankers_engine::EngineTerminalFailure>; }
impl ReportExt for clankers_engine_host::EngineRunReport {
    fn terminal_failure(&self) -> Option<&clankers_engine::EngineTerminalFailure> { self.last_outcome.terminal_failure.as_ref() }
}

fn seed(text: &str) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages: vec![EngineMessage { role: EngineMessageRole::User, content: vec![Content::Text { text: text.to_string() }] }],
        model: MODEL.to_string(),
        system_prompt: "minimal embedded kit".to_string(),
        max_tokens: None,
        temperature: None,
        thinking: None,
        tools: vec![],
        no_cache: true,
        cache_ttl: None,
        session_id: SESSION_ID.to_string(),
        model_request_slot_budget: 1,
    };
    let initial_state = EngineState::new();
    let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(initial_state, first_outcome)
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct NoopWaker;
    impl Wake for NoopWaker { fn wake(self: Arc<Self>) {} }
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
