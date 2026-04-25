//! Autonomous experiment loop — init/run/log workflow with JSONL persistence,
//! metric extraction, confidence scoring, and git integration.

pub mod confidence;
pub mod git;
pub mod jsonl;
pub mod metrics;
pub mod session;

pub use confidence::ConfidenceResult;
pub use jsonl::ExperimentConfig;
pub use jsonl::ExperimentResult;
pub use jsonl::ResultStatus;
pub use session::ExperimentSession;
