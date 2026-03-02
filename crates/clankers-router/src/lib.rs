//! clankers-router — Model router and auth gateway for LLM providers
//!
//! This crate provides:
//!
//! - **Unified provider trait** — Common interface for all LLM providers
//! - **Auth framework** — Multi-provider credential storage, OAuth PKCE flows, automatic token
//!   refresh with file-locking, proactive background refresh
//! - **Model registry** — Catalog of models with capabilities, pricing, aliases
//! - **Router** — Routes completion requests to the right provider based on model ID, aliases, and
//!   credential availability
//! - **Fallback chains** — Configurable per-model fallback sequences for automatic failover when a
//!   provider is rate-limited or down
//! - **Circuit breaker** — Per-provider/model health tracking in [`db::rate_limits`] with Closed →
//!   Open → HalfOpen state machine, exponential backoff, and automatic probe-on-cooldown-expiry
//! - **Retry & resilience** — Exponential backoff with full jitter to decorrelate retry storms,
//!   Retry-After header support, retryable status detection (429/5xx), structured error status
//!   codes
//! - **Response cache** — SHA-256 keyed cache with TTL, LRU eviction, hit counting, and automatic
//!   background eviction of expired entries
//! - **Persistent database** — redb-backed storage for usage tracking, rate-limit state, request
//!   audit log, and response cache
//! - **OpenAI-compatible proxy** — HTTP server that exposes the router as an OpenAI API for use
//!   with Cursor, aider, Continue, etc.
//! - **iroh p2p tunnel** — QUIC-based remote access to the proxy without port forwarding, plus an
//!   RPC protocol for clankers ↔ router communication
//!
//! # Supported Providers
//!
//! - **Anthropic** — Native Messages API with OAuth + API key auth, prompt caching
//! - **OpenAI** — GPT-4o, o3, o3-mini with reasoning token support
//! - **Google/Gemini** — Via OpenAI-compatible endpoint
//! - **DeepSeek** — V3 and R1 (reasoning)
//! - **Groq** — Llama inference
//! - **Mistral** — Large, Small, Codestral
//! - **OpenRouter** — Dynamic model catalog
//! - **Together AI** — Llama, DeepSeek, Qwen
//! - **Fireworks AI** — Llama, DeepSeek
//! - **Perplexity** — Sonar search-augmented models
//! - **xAI** — Grok 3, Grok 3 Mini
//! - **Local** — Ollama, LM Studio, vLLM (any OpenAI-compatible server)
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                      Router                           │
//! │  ┌──────────┐ ┌──────────┐ ┌────────────────────┐   │
//! │  │ Fallback  │ │ Registry │ │  Circuit Breaker   │   │
//! │  │  chains   │ │ (models) │ │ (per-provider/model│   │
//! │  └─────┬─────┘ └────┬─────┘ └────────┬───────────┘   │
//! │        └─────────────┼────────────────┘               │
//! │                      ▼                                │
//! │  ┌────────────────────────────────────────────────┐  │
//! │  │              Provider Backends                  │  │
//! │  │  ┌──────────┐ ┌────────┐ ┌────────┐ ┌───────┐ │  │
//! │  │  │Anthropic │ │ OpenAI │ │ Google │ │DeepSeek│ │  │
//! │  │  └──────────┘ └────────┘ └────────┘ └───────┘ │  │
//! │  │  ┌──────────┐ ┌────────┐ ┌────────┐ ┌───────┐ │  │
//! │  │  │  Groq    │ │Mistral │ │Together│ │  xAI  │ │  │
//! │  │  └──────────┘ └────────┘ └────────┘ └───────┘ │  │
//! │  └────────────────────────────────────────────────┘  │
//! │                      │                                │
//! │  ┌───────────┐ ┌────┴─────┐ ┌─────────────────────┐ │
//! │  │ Resp Cache │ │ Usage DB │ │  Request Audit Log  │ │
//! │  └───────────┘ └──────────┘ └─────────────────────┘ │
//! └──────────────────────────────────────────────────────┘
//! ```

pub mod auth;
pub mod backends;
pub mod credential;
pub mod db;
pub mod error;
pub mod model;
pub mod model_switch;
pub mod multi;
pub mod oauth;
pub mod provider;
#[cfg(feature = "proxy")]
pub mod proxy;
pub mod quorum;
pub mod registry;
pub mod retry;
pub mod router;
#[cfg(feature = "rpc")]
pub mod rpc;
pub mod streaming;

// Re-exports for convenience
pub use db::RouterDb;
pub use error::Error;
pub use error::Result;
pub use model::Model;
pub use model_switch::ModelSwitchReason;
pub use model_switch::ModelSwitchRecord;
pub use model_switch::ModelSwitchTracker;
pub use multi::MultiRequest;
pub use multi::MultiResponse;
pub use multi::MultiResult;
pub use multi::MultiStrategy;
pub use provider::CompletionRequest;
pub use provider::Provider;
pub use provider::ThinkingConfig;
pub use provider::Usage;
pub use registry::ModelRegistry;
pub use router::FallbackConfig;
pub use router::Router;
pub use quorum::ConsensusStrategy;
pub use quorum::QuorumRequest;
pub use quorum::QuorumResult;
pub use quorum::QuorumTarget;
pub use streaming::TaggedStreamEvent;
