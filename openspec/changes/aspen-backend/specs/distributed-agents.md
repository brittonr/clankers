# Distributed Agents — Jobs & Coordination

## Summary

When running in cluster mode, subagent delegation and multi-agent
coordination use aspen's job queue and coordination primitives instead of
local tokio tasks and in-process mutexes.  This enables cross-node work
distribution, automatic retry, and concurrent session locking across a
multi-node clankers deployment.

## Subagent Jobs

### Job Type

```rust
const JOB_TYPE_AGENT_PROMPT: &str = "clankers-agent-prompt";

#[derive(Serialize, Deserialize)]
struct AgentPromptPayload {
    /// The prompt to execute
    prompt: String,
    /// Which tools the subagent can use
    tools: Vec<String>,
    /// Model to use
    model: String,
    /// System prompt
    system_prompt: String,
    /// Context from the parent agent (file paths, prior output, etc.)
    context: Option<String>,
    /// Maximum turns before giving up
    max_turns: u32,
    /// Parent session ID (for audit trail)
    parent_session_id: String,
}

#[derive(Serialize, Deserialize)]
struct AgentPromptResult {
    /// The agent's final response text
    response: String,
    /// Tools that were called
    tool_calls: Vec<ToolCallSummary>,
    /// Token usage
    tokens: TokenUsage,
    /// Which node executed this
    node_id: String,
}
```

### Submission

```rust
// In the delegate tool (src/tools/delegate.rs)
async fn execute_cluster(&self, ctx: &ToolContext, params: DelegateParams) -> ToolResult {
    let job = Job::new(JOB_TYPE_AGENT_PROMPT)
        .payload(AgentPromptPayload {
            prompt: params.task,
            tools: params.tools.unwrap_or_default(),
            model: params.model.unwrap_or(self.default_model.clone()),
            system_prompt: self.system_prompt.clone(),
            context: params.context,
            max_turns: 20,
            parent_session_id: ctx.session_id.clone(),
        })
        .priority(JobPriority::Normal)
        .retry(RetryPolicy::exponential(3, Duration::from_secs(5)))
        .timeout(Duration::from_secs(600));

    let job_id = self.job_manager.submit(job).await?;

    // Stream progress updates while waiting
    let mut watcher = self.job_manager.watch(job_id).await?;
    while let Some(event) = watcher.next().await {
        match event {
            JobEvent::Progress(msg) => ctx.emit_progress(&msg),
            JobEvent::Completed(result) => {
                let result: AgentPromptResult = serde_json::from_str(&result)?;
                return ToolResult::text(&result.response);
            }
            JobEvent::Failed(err) => return ToolResult::error(&err),
        }
    }

    ToolResult::error("Job watcher closed unexpectedly")
}
```

### Worker

```rust
struct ClankerAgentWorker {
    provider: Arc<dyn Provider>,
    available_tools: Vec<Arc<dyn Tool>>,
    settings: Settings,
}

#[async_trait]
impl JobWorker for ClankerAgentWorker {
    fn job_types(&self) -> &[&str] {
        &[JOB_TYPE_AGENT_PROMPT]
    }

    async fn execute(&self, job: &Job, progress: &ProgressReporter) -> Result<String> {
        let payload: AgentPromptPayload = job.payload()?;

        // Filter tools to only those requested
        let tools = self.available_tools.iter()
            .filter(|t| payload.tools.is_empty() || payload.tools.contains(&t.definition().name))
            .cloned()
            .collect();

        // Create ephemeral agent
        let agent = Agent::new(
            Arc::clone(&self.provider),
            tools,
            self.settings.clone(),
            payload.model,
            payload.system_prompt,
        );

        // Run the prompt
        progress.report("Starting agent...").await?;
        let response = agent.prompt(&payload.prompt, payload.max_turns).await?;

        progress.report("Agent completed").await?;
        Ok(serde_json::to_string(&AgentPromptResult {
            response: response.text,
            tool_calls: response.tool_calls,
            tokens: response.tokens,
            node_id: self.node_id.clone(),
        })?)
    }
}
```

### Worker Registration

Each clankers node registers its worker with capabilities:

```rust
// During startup in cluster mode
let worker = ClankerAgentWorker::new(provider, tools, settings);
node_handle.worker.register(worker).await?;

// Worker advertises its capabilities via gossip
// Other nodes can see which models/tools are available where
```

## Coordination Primitives

### Session Locks

Prevent concurrent prompts to the same session across nodes:

```rust
async fn acquire_session_lock(
    coordination: &AspenCoordination,
    session_id: &str,
) -> Result<DistributedLock> {
    let lock = coordination.lock(
        &format!("clankers:locks:session:{}", session_id),
        LockOptions {
            ttl: Duration::from_secs(300),   // 5 min lease
            retry: RetryPolicy::linear(10, Duration::from_millis(200)),
        },
    ).await?;
    Ok(lock)
}

// Usage in prompt handler:
let lock = acquire_session_lock(&coordination, &session_id).await?;
let _guard = lock.guard(); // auto-releases on drop
// ... execute prompt ...
```

### Rate Limiting

Per-user rate limiting across all nodes:

```rust
let limiter = coordination.rate_limiter(
    &format!("clankers:ratelimit:user:{}", user_id),
    RateLimitConfig {
        tokens_per_second: 2.0,   // 2 prompts/sec
        burst: 5,                  // burst of 5
    },
).await?;

if !limiter.try_acquire(1).await? {
    return Err(Error::RateLimited);
}
```

### Agent Semaphore

Bound total concurrent agent executions across the cluster:

```rust
let semaphore = coordination.semaphore(
    "clankers:semaphore:agents",
    SemaphoreConfig {
        max_permits: 16,           // max 16 concurrent agents cluster-wide
        ttl: Duration::from_secs(600),
    },
).await?;

let permit = semaphore.acquire().await?;
// ... run agent ...
drop(permit); // release
```

## Standalone Fallback

In standalone mode, all coordination is in-process:

```rust
enum CoordinationMode {
    /// Local tokio mutexes and semaphores (current behavior)
    Standalone {
        session_locks: DashMap<String, Arc<Mutex<()>>>,
        agent_semaphore: tokio::sync::Semaphore,
    },
    /// Aspen distributed primitives
    Cluster {
        coordination: AspenCoordination,
    },
}
```

The `delegate` tool checks the mode:

```rust
match &self.coordination {
    CoordinationMode::Standalone { .. } => {
        // Spawn local tokio task (current behavior)
        self.execute_local(ctx, params).await
    }
    CoordinationMode::Cluster { .. } => {
        // Submit aspen job
        self.execute_cluster(ctx, params).await
    }
}
```

## Observability

Agent jobs emit standard aspen job metrics:

```
clankers:agents:{node-id}:status         → { "active_jobs": 3, "total_completed": 142 }
clankers:agents:{node-id}:jobs:active    → list of running job IDs
```

These are queryable via `clankers status --cluster` to see all active
agents across all nodes.
