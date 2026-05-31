//! Line-delimited JSON over stdin/stdout.
//!
//! Reads `Request` objects from stdin (one per line), executes them,
//! and writes `Response` objects to stdout. Streaming methods (prompt)
//! emit intermediate notification frames before the final response.

use std::sync::Arc;

use clankers_provider::Provider;

/// Context needed to build agents for prompt execution.
pub struct RpcContext {
    pub provider: Arc<dyn Provider>,
    pub tools: Vec<Arc<dyn crate::tools::Tool>>,
    pub settings: clankers_config::settings::Settings,
    pub model: String,
    pub system_prompt: String,
}
