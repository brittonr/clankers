use super::*;

#[async_trait]
pub(super) trait PueueRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> Result<String, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct PueueCliRunner;

#[async_trait]
impl PueueRunner for PueueCliRunner {
    async fn run(&self, args: &[String]) -> Result<String, RuntimeError> {
        let output = Command::new("pueue")
            .args(args)
            .output()
            .await
            .map_err(|e| RuntimeError::InvalidTool(format!("failed to execute pueue: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(RuntimeError::InvalidTool(format!("pueue {:?} failed: {message}", args)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Clone)]
pub(super) struct PueueProcessJobService<R = PueueCliRunner> {
    runner: Arc<R>,
    enabled: bool,
}

impl Default for PueueProcessJobService<PueueCliRunner> {
    fn default() -> Self {
        Self::new(PueueCliRunner)
    }
}

impl<R> PueueProcessJobService<R> {
    pub(super) fn new(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: true,
        }
    }

    #[cfg(test)]
    pub(super) fn disabled(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: false,
        }
    }
}

impl<R: PueueRunner> PueueProcessJobService<R> {
    async fn ensure_available(
        &self,
        operation: ProcessJobOperation,
    ) -> Result<Option<ProcessJobReceipt>, RuntimeError> {
        if !self.enabled {
            return Ok(Some(pueue_backend_unavailable(operation, "pueue backend is disabled by configuration")));
        }
        match self.runner.run(&["--version".to_string()]).await {
            Ok(_) => Ok(None),
            Err(error) => Ok(Some(pueue_backend_unavailable(operation, error.to_string()))),
        }
    }

    async fn pueue_tasks(&self) -> Result<Vec<PueueTaskProjection>, RuntimeError> {
        let json = self.runner.run(&["status".to_string(), "--json".to_string()]).await?;
        Ok(parse_pueue_tasks(&json))
    }

    async fn pueue_task(&self, id: &ProcessJobId) -> Result<PueueTaskProjection, RuntimeError> {
        let task_id = pueue_task_id(id)?;
        self.pueue_tasks()
            .await?
            .into_iter()
            .find(|task| task.task_id == task_id)
            .ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown pueue process session_id: {}", id.0)))
    }
}

#[async_trait]
impl<R: PueueRunner> ProcessJobService for PueueProcessJobService<R> {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Pueue {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Start,
                None,
                request.backend,
                "pueue process service only supports pueue backend requests",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Start).await? {
            return Ok(receipt);
        }
        let command = pueue_command_from_request(&request)?;
        let mut args = vec!["add".to_string(), "--print-task-id".to_string()];
        if let ProcessJobCwd::Explicit(cwd) = &request.cwd {
            args.push("--working-directory".to_string());
            args.push(cwd.display().to_string());
        }
        if let Some(group) = request.metadata.get("pueue_group").or_else(|| request.metadata.get("group")) {
            args.push("--group".to_string());
            args.push(group.clone());
        }
        if let Some(label) = request.metadata.get("label") {
            args.push("--label".to_string());
            args.push(label.clone());
        }
        args.push(command);
        let output = self.runner.run(&args).await?;
        let task_id = output
            .lines()
            .find_map(|line| line.trim().parse::<u64>().ok())
            .ok_or_else(|| RuntimeError::InvalidTool(format!("pueue add did not return a task id: {output}")))?;
        let id = ProcessJobId(format!("pueue_{task_id}"));
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Pending),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: pueue_log_refs(&id),
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
            summary: format!("Started pueue task {task_id} as {}", id.0),
            error: None,
        })
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        if self.ensure_available(ProcessJobOperation::List).await?.is_some() {
            return Ok(Vec::new());
        }
        let summaries = self
            .pueue_tasks()
            .await?
            .into_iter()
            .map(|task| task.summary())
            .filter(|summary| filter.backend.is_none_or(|backend| backend == summary.backend))
            .filter(|summary| filter.include_terminal || !summary.status.is_terminal())
            .collect();
        Ok(summaries)
    }

    async fn poll(
        &self,
        id: ProcessJobId,
        _cursor: Option<ProcessJobLogCursor>,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        let task = self.pueue_task(&id).await?;
        Ok(task.receipt(ProcessJobOperation::Poll, format!("{} status: {}", id.0, task.status_label)))
    }

    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        let lines = range.limit_bytes.clamp(1, DEFAULT_LOG_LIMIT as u64).to_string();
        let output = self
            .runner
            .run(&[
                "log".to_string(),
                "--json".to_string(),
                "--lines".to_string(),
                lines,
                task_id.to_string(),
            ])
            .await?;
        let text = parse_pueue_log_text(&output, task_id);
        let start = range.offset.unwrap_or(0);
        let len = u64::try_from(text.lines().count()).unwrap_or(u64::MAX);
        Ok(ProcessJobLogChunk {
            id,
            backend: ProcessJobBackendKind::Pueue,
            stream: range.stream,
            cursor: ProcessJobLogCursor {
                stream: range.stream,
                offset: start,
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: range.stream,
                offset: start.saturating_add(len),
            }),
            text,
            truncated: false,
        })
    }

    async fn wait(&self, id: ProcessJobId, timeout: Option<Duration>) -> Result<ProcessJobReceipt, RuntimeError> {
        let deadline = Instant::now() + timeout.unwrap_or(Duration::from_secs(30));
        loop {
            let task = self.pueue_task(&id).await?;
            if task.status.is_terminal() {
                return Ok(task.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} finished with status: {}", id.0, task.status_label),
                ));
            }
            if Instant::now() >= deadline {
                return Ok(task.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} still running as pueue task {}", id.0, task.task_id),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        self.runner.run(&["kill".to_string(), task_id.to_string()]).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Kill,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Killed),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: Vec::new(),
            profile: None,
            summary: format!("Kill requested for pueue task {task_id}"),
            error: None,
        })
    }

    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        self.runner.run(&["restart".to_string(), "--in-place".to_string(), task_id.to_string()]).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Restart,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Pending),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: Vec::new(),
            profile: None,
            summary: format!("Restart requested for pueue task {task_id}"),
            error: None,
        })
    }

    async fn write_stdin(
        &self,
        id: ProcessJobId,
        _data: Vec<u8>,
        _newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "pueue backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "pueue backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Pueue {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
                "pueue process service only supports pueue adoption requests",
            ));
        }
        if !request.is_authorized() {
            return Ok(ProcessJobReceipt::permission_denied(
                ProcessJobOperation::Adopt,
                ProcessJobBackendKind::Pueue,
                "adopt",
                "pueue task adoption denied by caller identity, capability scope, or backend-selection grant",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Adopt).await? {
            return Ok(receipt);
        }
        let task_id = pueue_task_id_from_backend_ref(&request.backend_ref)?;
        let task = self
            .pueue_tasks()
            .await?
            .into_iter()
            .find(|task| task.task_id == task_id)
            .ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown pueue task id for adoption: {task_id}")))?;
        Ok(
            task.receipt(
                ProcessJobOperation::Adopt,
                format!("Adopted pueue task {task_id} as {}", task.process_id().0),
            ),
        )
    }

    async fn garbage_collect(
        &self,
        _filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError> {
        Ok(unsupported_gc_receipt(
            ProcessJobBackendKind::Pueue,
            "pueue retention is owned by pueue cleanup policies for now",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PueueTaskProjection {
    task_id: u64,
    command: String,
    group: Option<String>,
    status: ProcessJobStatus,
    status_label: String,
    started_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

impl PueueTaskProjection {
    fn process_id(&self) -> ProcessJobId {
        ProcessJobId(format!("pueue_{}", self.task_id))
    }

    fn backend_ref(&self) -> BackendRef {
        BackendRef(format!("pueue:{}", self.task_id))
    }

    fn summary(&self) -> ProcessJobSummary {
        let id = self.process_id();
        let mut metadata = self.command.clone();
        if let Some(group) = &self.group {
            metadata = format!("[{group}] {metadata}");
        }
        ProcessJobSummary {
            id: id.clone(),
            backend: ProcessJobBackendKind::Pueue,
            backend_ref: Some(self.backend_ref()),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: self.status.clone(),
            command_preview: metadata.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
            cwd: ProcessJobCwd::Inherited,
            started_at: self.started_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            log_refs: pueue_log_refs(&id),
            profile: None,
        }
    }

    fn receipt(&self, operation: ProcessJobOperation, summary: String) -> ProcessJobReceipt {
        let id = self.process_id();
        ProcessJobReceipt {
            operation,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(self.status.clone()),
            backend_ref: Some(self.backend_ref()),
            log_refs: pueue_log_refs(&id),
            profile: None,
            summary,
            error: None,
        }
    }
}

fn pueue_command_from_request(request: &StartProcessJobRequest) -> Result<String, RuntimeError> {
    match (&request.shell_command, &request.program) {
        (Some(command), None) => Ok(command.clone()),
        (None, Some(program)) => Ok(format_direct_command(program, &request.args)),
        (Some(_), Some(_)) => Err(RuntimeError::InvalidTool(
            "pueue start requires either shell_command or program, not both".to_string(),
        )),
        (None, None) => Err(RuntimeError::InvalidTool("pueue start requires shell_command or program".to_string())),
    }
}

fn pueue_task_id(id: &ProcessJobId) -> Result<u64, RuntimeError> {
    id.0.strip_prefix("pueue_")
        .and_then(|raw| raw.parse::<u64>().ok())
        .ok_or_else(|| RuntimeError::InvalidTool(format!("{} is not a pueue process id", id.0)))
}

fn pueue_task_id_from_backend_ref(backend_ref: &BackendRef) -> Result<u64, RuntimeError> {
    let raw = backend_ref.0.strip_prefix("pueue:").unwrap_or(backend_ref.0.as_str());
    raw.parse::<u64>()
        .map_err(|_| RuntimeError::InvalidTool(format!("invalid pueue task backend_ref: {}", backend_ref.0)))
}

fn pueue_log_refs(id: &ProcessJobId) -> Vec<ProcessJobLogRef> {
    vec![ProcessJobLogRef {
        stream: ProcessJobStream::Combined,
        reference: format!("pueue:{}:log", id.0.trim_start_matches("pueue_")),
        retained_until: None,
        max_bytes: None,
    }]
}

fn pueue_backend_unavailable(operation: ProcessJobOperation, reason: impl Into<String>) -> ProcessJobReceipt {
    ProcessJobReceipt::backend_unavailable(operation, ProcessJobBackendKind::Pueue, reason)
}

fn parse_pueue_tasks(raw: &str) -> Vec<PueueTaskProjection> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(tasks) = value.get("tasks").and_then(Value::as_object) else {
        return Vec::new();
    };
    tasks.values().filter_map(parse_pueue_task).collect()
}

fn parse_pueue_task(value: &Value) -> Option<PueueTaskProjection> {
    let task_id = value.get("id")?.as_u64()?;
    let command = value
        .get("original_command")
        .or_else(|| value.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let group = value.get("group").and_then(Value::as_str).map(str::to_string);
    let (status, status_label, terminal_time) = parse_pueue_status(value.get("status"));
    let created_at = value.get("created_at").and_then(Value::as_str).and_then(parse_pueue_time);
    let started_at = value
        .get("start")
        .or_else(|| value.get("started_at"))
        .and_then(Value::as_str)
        .and_then(parse_pueue_time)
        .or(created_at);
    let updated_at = terminal_time.or(started_at).unwrap_or_else(Utc::now);
    Some(PueueTaskProjection {
        task_id,
        command,
        group,
        status,
        status_label,
        started_at,
        updated_at,
        completed_at: terminal_time,
    })
}

fn parse_pueue_status(status: Option<&Value>) -> (ProcessJobStatus, String, Option<DateTime<Utc>>) {
    let Some(status) = status else {
        return (
            ProcessJobStatus::Unknown {
                raw: "missing".to_string(),
            },
            "missing".to_string(),
            None,
        );
    };
    let Some((name, detail)) = status.as_object().and_then(|object| object.iter().next()) else {
        return (
            ProcessJobStatus::Unknown {
                raw: status.to_string(),
            },
            status.to_string(),
            None,
        );
    };
    let lower = name.to_ascii_lowercase();
    let completed_at = detail
        .get("finished_at")
        .or_else(|| detail.get("end"))
        .or_else(|| detail.get("done_at"))
        .and_then(Value::as_str)
        .and_then(parse_pueue_time);
    let exit_code = detail
        .get("exit_code")
        .or_else(|| detail.get("code"))
        .and_then(Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    let projected = match lower.as_str() {
        "running" => ProcessJobStatus::Running,
        "queued" | "stashed" | "paused" | "locked" => ProcessJobStatus::Pending,
        "done" | "success" | "succeeded" => ProcessJobStatus::Succeeded {
            exit_code: exit_code.or(Some(0)),
        },
        "failed" => ProcessJobStatus::Failed {
            exit_code,
            reason: detail.get("reason").and_then(Value::as_str).unwrap_or("pueue task failed").to_string(),
        },
        "killed" => ProcessJobStatus::Killed,
        _ if lower.contains("failed") => ProcessJobStatus::Failed {
            exit_code,
            reason: name.clone(),
        },
        _ => ProcessJobStatus::Unknown { raw: name.clone() },
    };
    (projected, lower, completed_at)
}

fn parse_pueue_log_text(raw: &str, task_id: u64) -> String {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    let task = value
        .get("tasks")
        .and_then(Value::as_object)
        .and_then(|tasks| tasks.get(&task_id.to_string()))
        .or_else(|| value.get(task_id.to_string()))
        .or_else(|| value.get("output"));
    let Some(task) = task else {
        return String::new();
    };
    for key in ["output", "log", "stdout", "stderr"] {
        if let Some(text) = task.get(key).and_then(Value::as_str) {
            return text.to_string();
        }
    }
    if let Some(lines) = task.get("lines").and_then(Value::as_array) {
        return lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    }
    task.as_str().unwrap_or_default().to_string()
}

fn parse_pueue_time(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw).ok().map(|time| time.with_timezone(&Utc))
}
