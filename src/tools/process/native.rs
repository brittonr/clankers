use clankers_runtime::RuntimeError;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobReceipt;
use clankers_runtime::process_jobs::StartProcessJobRequest;

pub(super) struct NativeProcessJobBackendAdapter {
    db: Option<clankers_db::Db>,
    process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
    call_id: Option<String>,
}

impl NativeProcessJobBackendAdapter {
    pub(super) fn for_invocation(
        db: Option<clankers_db::Db>,
        process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
        call_id: String,
    ) -> Self {
        Self {
            db,
            process_monitor,
            call_id: Some(call_id),
        }
    }

    pub(super) async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        super::start_native_process_job(
            request,
            self.db.clone(),
            self.process_monitor.as_ref(),
            self.call_id.as_deref(),
        )
        .await
    }

    pub(super) async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        super::restart_native_process_job(
            id,
            self.db.clone(),
            self.process_monitor.as_ref(),
            self.call_id.as_deref(),
        )
        .await
    }
}
