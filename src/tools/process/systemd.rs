use super::*;

#[async_trait]
pub(super) trait SystemdRunner: Send + Sync {
    async fn run(&self, program: &str, args: &[String]) -> Result<String, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct SystemdCliRunner;

#[async_trait]
impl SystemdRunner for SystemdCliRunner {
    async fn run(&self, program: &str, args: &[String]) -> Result<String, RuntimeError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .map_err(|e| RuntimeError::InvalidTool(format!("failed to execute {program}: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(RuntimeError::InvalidTool(format!("{program} {:?} failed: {message}", args)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Clone)]
pub(super) struct SystemdProcessJobService<R = SystemdCliRunner> {
    runner: Arc<R>,
    enabled: bool,
    user_mode: bool,
}

impl Default for SystemdProcessJobService<SystemdCliRunner> {
    fn default() -> Self {
        Self::new(SystemdCliRunner)
    }
}

impl<R> SystemdProcessJobService<R> {
    pub(super) fn new(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: true,
            user_mode: true,
        }
    }

    #[cfg(test)]
    pub(super) fn disabled(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: false,
            user_mode: true,
        }
    }

    fn manager_args(&self) -> Vec<String> {
        if self.user_mode {
            vec!["--user".to_string()]
        } else {
            Vec::new()
        }
    }
}

impl<R: SystemdRunner> SystemdProcessJobService<R> {
    async fn ensure_available(
        &self,
        operation: ProcessJobOperation,
    ) -> Result<Option<ProcessJobReceipt>, RuntimeError> {
        if !self.enabled {
            return Ok(Some(systemd_backend_unavailable(operation, "systemd backend is disabled by configuration")));
        }
        match self.runner.run("systemctl", &["--version".to_string()]).await {
            Ok(_) => Ok(None),
            Err(error) => Ok(Some(systemd_backend_unavailable(operation, error.to_string()))),
        }
    }

    async fn systemd_unit(&self, id: &ProcessJobId) -> Result<SystemdUnitProjection, RuntimeError> {
        let unit = systemd_unit_name(id)?;
        let mut args = self.manager_args();
        args.extend(["show".to_string(), unit.clone()]);
        args.extend([
            "--property=Id".to_string(),
            "--property=Description".to_string(),
            "--property=ActiveState".to_string(),
            "--property=SubState".to_string(),
            "--property=Result".to_string(),
            "--property=ExecMainStatus".to_string(),
            "--property=ExecMainPID".to_string(),
        ]);
        let output = self.runner.run("systemctl", &args).await?;
        Ok(parse_systemd_show(&output, &unit))
    }
}

#[async_trait]
impl<R: SystemdRunner> ProcessJobService for SystemdProcessJobService<R> {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Systemd {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Start,
                None,
                request.backend,
                "systemd process service only supports systemd backend requests",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Start).await? {
            return Ok(receipt);
        }
        let unit = systemd_unit_from_request(&request);
        let mut args = self.manager_args();
        args.extend(["--unit".to_string(), unit.clone(), "--collect".to_string()]);
        if request.metadata.get("systemd_scope").is_some_and(|value| value == "true") {
            args.push("--scope".to_string());
        }
        if let ProcessJobCwd::Explicit(cwd) = &request.cwd {
            args.push("--working-directory".to_string());
            args.push(cwd.display().to_string());
        }
        match (&request.shell_command, &request.program) {
            (Some(command), None) => args.extend(["sh".to_string(), "-lc".to_string(), command.clone()]),
            (None, Some(program)) => {
                args.push(program.clone());
                args.extend(request.args.clone());
            }
            (Some(_), Some(_)) => {
                return Err(RuntimeError::InvalidTool(
                    "systemd start requires either shell_command or program, not both".to_string(),
                ));
            }
            (None, None) => {
                return Err(RuntimeError::InvalidTool("systemd start requires shell_command or program".to_string()));
            }
        }
        self.runner.run("systemd-run", &args).await?;
        let id = systemd_process_id(&unit);
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
            summary: format!("Started systemd transient unit {unit} as {}", id.0),
            error: None,
        })
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        if self.ensure_available(ProcessJobOperation::List).await?.is_some() {
            return Ok(Vec::new());
        }
        let mut args = self.manager_args();
        args.extend([
            "list-units".to_string(),
            "--type=service".to_string(),
            "--type=scope".to_string(),
            "--all".to_string(),
            "--no-legend".to_string(),
            "--plain".to_string(),
        ]);
        let output = self.runner.run("systemctl", &args).await?;
        Ok(parse_systemd_list_units(&output)
            .into_iter()
            .filter(|summary| filter.backend.is_none_or(|backend| backend == summary.backend))
            .filter(|summary| filter.include_terminal || !summary.status.is_terminal())
            .collect())
    }

    async fn poll(
        &self,
        id: ProcessJobId,
        _cursor: Option<ProcessJobLogCursor>,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = self.systemd_unit(&id).await?;
        Ok(unit.receipt(ProcessJobOperation::Poll, format!("{} status: {}", id.0, unit.status_label)))
    }

    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend([
            "-u".to_string(),
            unit.clone(),
            "--no-pager".to_string(),
            "--output=short-iso".to_string(),
            "-n".to_string(),
            range.limit_bytes.clamp(1, DEFAULT_LOG_LIMIT as u64).to_string(),
        ]);
        let text = self.runner.run("journalctl", &args).await?;
        let start = range.offset.unwrap_or(0);
        let len = u64::try_from(text.lines().count()).unwrap_or(u64::MAX);
        Ok(ProcessJobLogChunk {
            id,
            backend: ProcessJobBackendKind::Systemd,
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
            let unit = self.systemd_unit(&id).await?;
            if unit.status.is_terminal() {
                return Ok(unit.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} finished with status: {}", id.0, unit.status_label),
                ));
            }
            if Instant::now() >= deadline {
                return Ok(unit.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} still running as systemd unit {}", id.0, unit.unit),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend(["kill".to_string(), "--kill-whom=all".to_string(), unit.clone()]);
        self.runner.run("systemctl", &args).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Kill,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Killed),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            profile: None,
            summary: format!("Cgroup kill requested for systemd unit {unit}"),
            error: None,
        })
    }

    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend(["restart".to_string(), unit.clone()]);
        self.runner.run("systemctl", &args).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Restart,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            profile: None,
            summary: format!("Restart requested for systemd unit {unit}"),
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
            ProcessJobBackendKind::Systemd,
            "systemd backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Systemd,
            "systemd backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Systemd {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
                "systemd process service only supports systemd adoption requests",
            ));
        }
        if !request.is_authorized() {
            return Ok(ProcessJobReceipt::permission_denied(
                ProcessJobOperation::Adopt,
                ProcessJobBackendKind::Systemd,
                "adopt",
                "systemd unit adoption denied by caller identity, capability scope, or backend-selection grant",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Adopt).await? {
            return Ok(receipt);
        }
        let unit_name = systemd_unit_name_from_backend_ref(&request.backend_ref)?;
        let unit = self.systemd_unit(&systemd_process_id(&unit_name)).await?;
        Ok(unit.receipt(
            ProcessJobOperation::Adopt,
            format!("Adopted systemd unit {} as {}", unit.unit, unit.process_id().0),
        ))
    }

    async fn garbage_collect(
        &self,
        _filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError> {
        Ok(unsupported_gc_receipt(
            ProcessJobBackendKind::Systemd,
            "systemd transient-unit retention is delegated to systemd --collect for now",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemdUnitProjection {
    unit: String,
    description: String,
    status: ProcessJobStatus,
    status_label: String,
    updated_at: DateTime<Utc>,
}

impl SystemdUnitProjection {
    fn process_id(&self) -> ProcessJobId {
        systemd_process_id(&self.unit)
    }

    fn backend_ref(&self) -> BackendRef {
        BackendRef(format!("systemd:{}", self.unit))
    }

    fn summary(&self) -> ProcessJobSummary {
        let id = self.process_id();
        ProcessJobSummary {
            id: id.clone(),
            backend: ProcessJobBackendKind::Systemd,
            backend_ref: Some(self.backend_ref()),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: self.status.clone(),
            command_preview: self.description.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
            cwd: ProcessJobCwd::Inherited,
            started_at: None,
            updated_at: self.updated_at,
            completed_at: self.status.is_terminal().then_some(self.updated_at),
            log_refs: systemd_log_refs(&self.unit),
            profile: None,
        }
    }

    fn receipt(&self, operation: ProcessJobOperation, summary: String) -> ProcessJobReceipt {
        let id = self.process_id();
        ProcessJobReceipt {
            operation,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(self.status.clone()),
            backend_ref: Some(self.backend_ref()),
            log_refs: systemd_log_refs(&self.unit),
            profile: None,
            summary,
            error: None,
        }
    }
}

fn systemd_process_id(unit: &str) -> ProcessJobId {
    ProcessJobId(format!("systemd_{unit}"))
}

fn systemd_unit_name(id: &ProcessJobId) -> Result<String, RuntimeError> {
    id.0.strip_prefix("systemd_")
        .filter(|unit| unit.ends_with(".service") || unit.ends_with(".scope"))
        .map(str::to_string)
        .ok_or_else(|| RuntimeError::InvalidTool(format!("{} is not a systemd process id", id.0)))
}

fn systemd_unit_from_request(request: &StartProcessJobRequest) -> String {
    if let Some(unit) = request.metadata.get("systemd_unit").filter(|unit| !unit.is_empty()) {
        return normalize_systemd_unit_name(unit);
    }
    let suffix = if request.metadata.get("systemd_scope").is_some_and(|value| value == "true") {
        "scope"
    } else {
        "service"
    };
    let label = request.metadata.get("label").map(String::as_str).unwrap_or(request.command_preview.as_str());
    let safe = sanitize_systemd_unit_component(label);
    format!("clankers-{safe}-{}.{}", Utc::now().timestamp_millis(), suffix)
}

fn normalize_systemd_unit_name(unit: &str) -> String {
    if unit.ends_with(".service") || unit.ends_with(".scope") {
        unit.to_string()
    } else {
        format!("{unit}.service")
    }
}

fn sanitize_systemd_unit_component(raw: &str) -> String {
    let mut safe = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    safe.truncate(48);
    if safe.is_empty() { "job".to_string() } else { safe }
}

fn systemd_log_refs(unit: &str) -> Vec<ProcessJobLogRef> {
    vec![ProcessJobLogRef {
        stream: ProcessJobStream::Combined,
        reference: format!("journalctl:{unit}"),
        retained_until: None,
        max_bytes: None,
    }]
}

fn systemd_backend_unavailable(operation: ProcessJobOperation, reason: impl Into<String>) -> ProcessJobReceipt {
    ProcessJobReceipt::backend_unavailable(operation, ProcessJobBackendKind::Systemd, reason)
}

fn parse_systemd_show(raw: &str, fallback_unit: &str) -> SystemdUnitProjection {
    let mut values = std::collections::BTreeMap::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key, value);
        }
    }
    let unit = values.get("Id").copied().filter(|value| !value.is_empty()).unwrap_or(fallback_unit).to_string();
    let description = values
        .get("Description")
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or(unit.as_str())
        .to_string();
    let active = values.get("ActiveState").copied().unwrap_or("unknown");
    let sub = values.get("SubState").copied().unwrap_or("unknown");
    let result = values.get("Result").copied().unwrap_or("unknown");
    let exit_code = values.get("ExecMainStatus").and_then(|value| value.parse::<i32>().ok());
    let (status, status_label) = systemd_status_from_parts(active, sub, result, exit_code);
    SystemdUnitProjection {
        unit,
        description,
        status,
        status_label,
        updated_at: Utc::now(),
    }
}

fn parse_systemd_list_units(raw: &str) -> Vec<ProcessJobSummary> {
    raw.lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 4 {
                return None;
            }
            let unit = fields[0];
            if !(unit.ends_with(".service") || unit.ends_with(".scope")) {
                return None;
            }
            if !unit.starts_with("clankers-") {
                return None;
            }
            let description = if fields.len() > 4 {
                fields[4..].join(" ")
            } else {
                unit.to_string()
            };
            let (status, status_label) = systemd_status_from_parts(fields[2], fields[3], "", None);
            Some(
                SystemdUnitProjection {
                    unit: unit.to_string(),
                    description,
                    status,
                    status_label,
                    updated_at: Utc::now(),
                }
                .summary(),
            )
        })
        .collect()
}

fn systemd_status_from_parts(
    active: &str,
    sub: &str,
    result: &str,
    exit_code: Option<i32>,
) -> (ProcessJobStatus, String) {
    let label = if result.is_empty() || result == "success" {
        format!("{active}/{sub}")
    } else {
        format!("{active}/{sub}/{result}")
    };
    let status = match active {
        "active" | "reloading" | "refreshing" => ProcessJobStatus::Running,
        "activating" => ProcessJobStatus::Pending,
        "failed" => ProcessJobStatus::Failed {
            exit_code,
            reason: if result.is_empty() {
                sub.to_string()
            } else {
                result.to_string()
            },
        },
        "inactive" | "deactivating" if matches!(result, "signal" | "core-dump") => ProcessJobStatus::Killed,
        "inactive" | "deactivating" if result == "success" || exit_code == Some(0) => ProcessJobStatus::Succeeded {
            exit_code: exit_code.or(Some(0)),
        },
        "inactive" | "deactivating" if !result.is_empty() && result != "success" => ProcessJobStatus::Failed {
            exit_code,
            reason: result.to_string(),
        },
        _ => ProcessJobStatus::Unknown { raw: label.clone() },
    };
    (status, label)
}

fn systemd_unit_name_from_backend_ref(backend_ref: &BackendRef) -> Result<String, RuntimeError> {
    let unit = backend_ref.0.strip_prefix("systemd:").unwrap_or(backend_ref.0.as_str()).trim();
    if unit.is_empty() || unit.contains('/') || unit.contains("..") {
        return Err(RuntimeError::InvalidTool(format!("invalid systemd unit backend_ref: {}", backend_ref.0)));
    }
    if !(unit.ends_with(".service") || unit.ends_with(".scope")) {
        return Err(RuntimeError::InvalidTool(format!(
            "systemd adoption requires a .service or .scope unit name: {}",
            backend_ref.0
        )));
    }
    Ok(unit.to_string())
}
