//! Provider backends
//!
//! Each backend implements the [`Provider`](crate::provider::Provider) trait
//! for a specific LLM API.

pub mod anthropic;
pub mod common;
pub mod huggingface;
pub mod openai_compat;
