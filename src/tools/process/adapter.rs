use std::time::Duration;

use clankers_runtime::process_jobs::AdoptProcessJobRequest;
use clankers_runtime::process_jobs::BackendRef;
use clankers_runtime::process_jobs::GarbageCollectProcessJobsRequest;
use clankers_runtime::process_jobs::ListProcessJobsRequest;
use clankers_runtime::process_jobs::MutateProcessJobRequest;
use clankers_runtime::process_jobs::PollProcessJobRequest;
use clankers_runtime::process_jobs::ProcessJobBackendKind;
use clankers_runtime::process_jobs::ProcessJobCallerScope;
use clankers_runtime::process_jobs::ProcessJobCapabilitySet;
use clankers_runtime::process_jobs::ProcessJobCwd;
use clankers_runtime::process_jobs::ProcessJobFilter;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobLogRange;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicy;
use clankers_runtime::process_jobs::ProcessJobOwnerScope;
use clankers_runtime::process_jobs::ProcessJobRedactionPolicy;
use clankers_runtime::process_jobs::ProcessJobStream;
use clankers_runtime::process_jobs::ProcessJobToolRequest;
use clankers_runtime::process_jobs::ReadProcessJobLogRequest;
use clankers_runtime::process_jobs::StartProcessJobRequest;
use clankers_runtime::process_jobs::WaitProcessJobRequest;
use clankers_runtime::process_jobs::WriteProcessJobStdinRequest;
use serde_json::Value;

use crate::tools::ToolResult;

pub(super) struct ProcessToolJsonAdapter;

impl ProcessToolJsonAdapter {
    pub(super) fn process_job_tool_request(params: &Value) -> Result<ProcessJobToolRequest, ToolResult> {
        let action = params.get("action").and_then(Value::as_str).unwrap_or("start");
        match action {
            "start" => {
                let backend = Self::requested_backend(params)?;
                Self::start_request(params, backend).map(ProcessJobToolRequest::Start)
            }
            "list" => Self::process_job_filter_request(params)
                .map(|filter| ProcessJobToolRequest::List(ListProcessJobsRequest { filter })),
            "poll" => Ok(ProcessJobToolRequest::Poll(PollProcessJobRequest {
                id: ProcessJobId(Self::required_session(params)?),
                cursor: None,
            })),
            "log" => Ok(ProcessJobToolRequest::Log(ReadProcessJobLogRequest {
                id: ProcessJobId(Self::required_session(params)?),
                range: Self::process_job_log_range(params),
                raw: params.get("raw").and_then(Value::as_bool).unwrap_or(false),
            })),
            "wait" => Ok(ProcessJobToolRequest::Wait(WaitProcessJobRequest {
                id: ProcessJobId(Self::required_session(params)?),
                timeout: Self::process_job_timeout(params),
            })),
            "kill" => Ok(ProcessJobToolRequest::Kill(MutateProcessJobRequest {
                id: ProcessJobId(Self::required_session(params)?),
            })),
            "restart" => Ok(ProcessJobToolRequest::Restart(MutateProcessJobRequest {
                id: ProcessJobId(Self::required_session(params)?),
            })),
            "write" => Ok(ProcessJobToolRequest::WriteStdin(WriteProcessJobStdinRequest {
                id: ProcessJobId(Self::required_session(params)?),
                data: params.get("data").and_then(Value::as_str).unwrap_or("").as_bytes().to_vec(),
                newline: false,
            })),
            "submit" => Ok(ProcessJobToolRequest::WriteStdin(WriteProcessJobStdinRequest {
                id: ProcessJobId(Self::required_session(params)?),
                data: params.get("data").and_then(Value::as_str).unwrap_or("").as_bytes().to_vec(),
                newline: true,
            })),
            "close" => Ok(ProcessJobToolRequest::CloseStdin(MutateProcessJobRequest {
                id: ProcessJobId(Self::required_session(params)?),
            })),
            "adopt" => {
                let backend = Self::requested_backend(params)?;
                Self::adopt_request(params, backend).map(ProcessJobToolRequest::Adopt)
            }
            "gc" | "garbage_collect" => Self::process_job_filter_request(params)
                .map(|filter| ProcessJobToolRequest::GarbageCollect(GarbageCollectProcessJobsRequest { filter })),
            other => Err(ToolResult::error(format!("Unknown process action: {other}"))),
        }
    }

    fn required_session(params: &Value) -> Result<String, ToolResult> {
        params
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolResult::error("Missing required parameter: session_id"))
    }

    fn parse_args(params: &Value) -> Result<Vec<String>, ToolResult> {
        let Some(value) = params.get("args") else {
            return Ok(Vec::new());
        };
        let Some(values) = value.as_array() else {
            return Err(ToolResult::error("Parameter 'args' must be an array of strings."));
        };
        let mut args = Vec::with_capacity(values.len());
        for value in values {
            let Some(arg) = value.as_str() else {
                return Err(ToolResult::error("Parameter 'args' must be an array of strings."));
            };
            args.push(arg.to_string());
        }
        Ok(args)
    }

    fn notification_policy(params: &Value) -> Result<ProcessJobNotificationPolicy, ToolResult> {
        let notify_on_complete = params.get("notify_on_complete").and_then(Value::as_bool).unwrap_or(false);
        let watch_patterns = match params.get("watch_patterns") {
            Some(value) => {
                let Some(values) = value.as_array() else {
                    return Err(ToolResult::error("Parameter 'watch_patterns' must be an array of strings."));
                };
                let mut patterns = Vec::with_capacity(values.len());
                for value in values {
                    let Some(pattern) = value.as_str() else {
                        return Err(ToolResult::error("Parameter 'watch_patterns' must be an array of strings."));
                    };
                    patterns.push(pattern.to_string());
                }
                patterns
            }
            None => Vec::new(),
        };
        Ok(ProcessJobNotificationPolicy {
            notify_on_complete,
            watch_patterns,
        })
    }

    fn requested_backend(params: &Value) -> Result<ProcessJobBackendKind, ToolResult> {
        match params.get("backend").and_then(Value::as_str).unwrap_or("native") {
            "native" => Ok(ProcessJobBackendKind::Native),
            "pueue" => Ok(ProcessJobBackendKind::Pueue),
            "systemd" => Ok(ProcessJobBackendKind::Systemd),
            other => Err(ToolResult::error(format!("Unsupported process backend: {other}"))),
        }
    }

    fn caller_scope_for_owner(
        owner: &ProcessJobOwnerScope,
        capabilities: ProcessJobCapabilitySet,
    ) -> ProcessJobCallerScope {
        let mut caller = ProcessJobCallerScope {
            capabilities,
            ..ProcessJobCallerScope::default()
        };
        match owner {
            ProcessJobOwnerScope::Session(session) => caller.session_id = Some(session.clone()),
            ProcessJobOwnerScope::Workspace(workspace) => caller.workspace_id = Some(workspace.clone()),
            ProcessJobOwnerScope::User(user) => caller.user_id = Some(user.clone()),
            ProcessJobOwnerScope::DaemonGlobal => caller.daemon_global = true,
        }
        caller
    }

    fn adopt_request(params: &Value, backend: ProcessJobBackendKind) -> Result<AdoptProcessJobRequest, ToolResult> {
        let backend_ref = params
            .get("backend_ref")
            .or_else(|| params.get("pid"))
            .or_else(|| params.get("pueue_task_id"))
            .or_else(|| params.get("systemd_unit"))
            .and_then(|value| value.as_str().map(str::to_string).or_else(|| value.as_u64().map(|id| id.to_string())))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ToolResult::error(
                    "Missing required parameter for adopt: backend_ref, pid, pueue_task_id, or systemd_unit",
                )
            })?;
        let owner = ProcessJobOwnerScope::DaemonGlobal;
        let caller = Self::caller_scope_for_owner(&owner, ProcessJobCapabilitySet::full_control());
        Ok(AdoptProcessJobRequest {
            backend,
            backend_ref: BackendRef(backend_ref),
            owner,
            caller,
        })
    }

    fn start_request(params: &Value, backend: ProcessJobBackendKind) -> Result<StartProcessJobRequest, ToolResult> {
        let command = params.get("command").and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        let program = params.get("program").and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        if command.is_some() && program.is_some() {
            return Err(ToolResult::error("Provide either 'command' or 'program', not both."));
        }
        let args = Self::parse_args(params)?;
        let command_preview = match (command, program) {
            (Some(command), None) => command.to_string(),
            (None, Some(program)) => super::format_direct_command(program, &args),
            (None, None) => return Err(ToolResult::error("Missing required parameter: command or program")),
            (Some(_), Some(_)) => unreachable!(),
        };
        let redaction = ProcessJobRedactionPolicy::default();
        let command_preview = redaction.safe_command_preview(&command_preview);
        let mut metadata = std::collections::BTreeMap::new();
        for key in ["group", "label", "systemd_unit", "systemd_scope"] {
            if let Some(value) = params.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()) {
                metadata.insert(key.to_string(), redaction.safe_metadata_value(key, value));
            }
        }
        Ok(StartProcessJobRequest {
            backend,
            command_preview,
            program: program.map(str::to_string),
            args,
            shell_command: command.map(str::to_string),
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            resource_policy: clankers_runtime::process_jobs::ProcessJobResourcePolicy::default(),
            notification_policy: Self::notification_policy(params)?,
            metadata,
        })
    }

    fn process_job_filter_request(params: &Value) -> Result<ProcessJobFilter, ToolResult> {
        let backend = match params.get("backend") {
            Some(_) => Some(Self::requested_backend(params)?),
            None => None,
        };
        Ok(ProcessJobFilter {
            backend,
            include_terminal: params.get("include_terminal").and_then(Value::as_bool).unwrap_or(true),
            ..ProcessJobFilter::default()
        })
    }

    fn process_job_log_range(params: &Value) -> ProcessJobLogRange {
        ProcessJobLogRange {
            stream: ProcessJobStream::Combined,
            offset: params.get("offset").and_then(Value::as_u64),
            limit_bytes: params.get("limit").and_then(Value::as_u64).unwrap_or(super::DEFAULT_LOG_LIMIT as u64),
        }
    }

    fn process_job_timeout(params: &Value) -> Option<Duration> {
        params.get("timeout").and_then(Value::as_u64).map(Duration::from_secs)
    }
}
