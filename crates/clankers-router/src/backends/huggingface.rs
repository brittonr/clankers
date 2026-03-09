//! HuggingFace provider backend
//!
//! Provides two capabilities:
//!
//! 1. **Inference API** — Cloud-hosted models via HuggingFace's OpenAI-compatible endpoint at `https://api-inference.huggingface.co/v1/chat/completions`.
//!    Works with any model available on HF Serverless Inference.
//!
//! 2. **Hub client** — Browse, search, and pull models from the HuggingFace Hub. Downloaded GGUF
//!    models can be served locally via Ollama or any compatible runtime.
//!
//! # Usage
//!
//! ```ignore
//! use clankers_router::backends::huggingface::*;
//! use clankers_router::backends::openai_compat::*;
//!
//! // Cloud inference
//! let provider = OpenAICompatProvider::new(OpenAICompatConfig::huggingface("hf_...".into()));
//!
//! // Hub operations
//! let hub = HubClient::new(Some("hf_...".into()));
//! let models = hub.search("llama", Some(10)).await?;
//! let pulled = hub.pull("bartowski/Llama-3.3-70B-Instruct-GGUF", "Q4_K_M").await?;
//! ```

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use tracing::info;
use tracing::warn;

use super::common;
use super::openai_compat::OpenAICompatConfig;
use crate::error::Result;
use crate::model::Model;

// ── Inference API (OpenAI-compatible) ───────────────────────────────────

impl OpenAICompatConfig {
    /// Create config for HuggingFace Inference API.
    ///
    /// Uses the OpenAI-compatible endpoint which accepts the model ID
    /// in the request body, just like OpenAI's API.
    pub fn huggingface(api_key: String) -> Self {
        Self {
            name: "huggingface".to_string(),
            base_url: "https://api-inference.huggingface.co/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: default_hf_models(),
            timeout: std::time::Duration::from_secs(300),
        }
    }
}

/// Default set of popular models available on HuggingFace Serverless Inference.
///
/// These are models that are commonly available without a dedicated endpoint.
/// The user can discover more via `clankers-router hf search`.
fn default_hf_models() -> Vec<Model> {
    vec![
        Model {
            id: "meta-llama/Llama-3.3-70B-Instruct".into(),
            name: "Llama 3.3 70B Instruct (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "Qwen/Qwen2.5-72B-Instruct".into(),
            name: "Qwen 2.5 72B Instruct (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "Qwen/Qwen2.5-Coder-32B-Instruct".into(),
            name: "Qwen 2.5 Coder 32B (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 32_768,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "mistralai/Mistral-Small-24B-Instruct-2501".into(),
            name: "Mistral Small 24B (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 32_768,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "microsoft/Phi-4".into(),
            name: "Phi-4 (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 16_384,
            max_output_tokens: 4_096,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "google/gemma-2-27b-it".into(),
            name: "Gemma 2 27B (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 8_192,
            max_output_tokens: 4_096,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "deepseek-ai/DeepSeek-R1-Distill-Llama-70B".into(),
            name: "DeepSeek R1 Distill 70B (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "NousResearch/Hermes-3-Llama-3.1-8B".into(),
            name: "Hermes 3 Llama 8B (HF)".into(),
            provider: "huggingface".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
    ]
}

// ── HuggingFace Hub Client ──────────────────────────────────────────────

/// Client for the HuggingFace Hub API.
///
/// Supports:
/// - Searching models by name/tag
/// - Getting model metadata and file listings
/// - Downloading GGUF model files for local serving
pub struct HubClient {
    client: reqwest::Client,
    api_key: Option<String>,
    cache_dir: PathBuf,
}

impl HubClient {
    const HUB_API: &'static str = "https://huggingface.co/api";

    /// Create a new Hub client.
    ///
    /// `api_key` is optional — public models don't require auth, but gated
    /// models (like Llama) need an HF token.
    pub fn new(api_key: Option<String>) -> Self {
        let cache_dir = default_cache_dir();
        Self {
            client: common::build_http_client(std::time::Duration::from_secs(300))
                .expect("failed to build HTTP client"),
            api_key,
            cache_dir,
        }
    }

    /// Create a Hub client with a custom cache directory.
    pub fn with_cache_dir(api_key: Option<String>, cache_dir: PathBuf) -> Self {
        Self {
            client: common::build_http_client(std::time::Duration::from_secs(300))
                .expect("failed to build HTTP client"),
            api_key,
            cache_dir,
        }
    }

    /// Get the model cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    // ── Search & Discovery ──────────────────────────────────────────

    /// Search for text-generation models on the Hub.
    ///
    /// Returns models sorted by downloads, filtered to chat/instruct models.
    pub async fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<HubModelInfo>> {
        let limit = limit.unwrap_or(20);
        let url = format!(
            "{}/models?search={}&filter=text-generation&sort=downloads&direction=-1&limit={}",
            Self::HUB_API,
            urlencoding::encode(query),
            limit,
        );
        let resp = self.authed_get(&url).await?;
        let body = resp.text().await?;
        let models: Vec<HubModelInfo> = serde_json::from_str(&body).map_err(|e| crate::Error::Provider {
            message: format!("failed to parse HF Hub response: {e}"),
            status: None,
        })?;
        Ok(models)
    }

    /// Get detailed information about a specific model.
    pub async fn model_info(&self, model_id: &str) -> Result<HubModelDetail> {
        let url = format!("{}/models/{}", Self::HUB_API, model_id);
        let resp = self.authed_get(&url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(crate::Error::provider_with_status(status.as_u16(), format!("HF Hub: {body}")));
        }
        let body = resp.text().await?;
        let info: HubModelDetail = serde_json::from_str(&body).map_err(|e| crate::Error::Provider {
            message: format!("failed to parse model info: {e}"),
            status: None,
        })?;
        Ok(info)
    }

    /// List files in a model repository.
    ///
    /// Optionally filter by extension (e.g., ".gguf").
    pub async fn list_files(&self, model_id: &str, extension: Option<&str>) -> Result<Vec<HubFile>> {
        let url = format!("{}/models/{}/tree/main", Self::HUB_API, model_id);
        let resp = self.authed_get(&url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(crate::Error::provider_with_status(status.as_u16(), format!("HF Hub: {body}")));
        }
        let body = resp.text().await?;
        let files: Vec<HubFile> = serde_json::from_str(&body).map_err(|e| crate::Error::Provider {
            message: format!("failed to parse file listing: {e}"),
            status: None,
        })?;

        if let Some(ext) = extension {
            Ok(files.into_iter().filter(|f| f.path.ends_with(ext)).collect())
        } else {
            Ok(files)
        }
    }

    /// List available GGUF files for a model, grouped by quantization.
    ///
    /// Looks for GGUF files in the repo and parses quantization info from
    /// filenames (e.g., `Q4_K_M`, `Q5_K_S`, `Q8_0`).
    pub async fn list_gguf_files(&self, model_id: &str) -> Result<Vec<GgufFileInfo>> {
        let files = self.list_files(model_id, Some(".gguf")).await?;
        let mut gguf_files: Vec<GgufFileInfo> = files
            .into_iter()
            .map(|f| {
                let quant = parse_quantization(&f.path);
                GgufFileInfo {
                    filename: f.path,
                    size_bytes: f.size,
                    quantization: quant,
                }
            })
            .collect();
        gguf_files.sort_by_key(|a| a.size_bytes);
        Ok(gguf_files)
    }

    // ── Model Pull (Download) ───────────────────────────────────────

    /// Pull (download) a GGUF model file from the Hub.
    ///
    /// If `quantization` is specified, selects the file matching that quant
    /// level (e.g., "Q4_K_M"). Otherwise downloads the smallest available
    /// GGUF file.
    ///
    /// Returns the local file path of the downloaded model.
    pub async fn pull(
        &self,
        model_id: &str,
        quantization: Option<&str>,
        progress: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> Result<PulledModel> {
        info!("pulling model {model_id} from HuggingFace Hub");

        // List available GGUF files
        let gguf_files = self.list_gguf_files(model_id).await?;
        if gguf_files.is_empty() {
            return Err(crate::Error::Provider {
                message: format!("no GGUF files found in {model_id}"),
                status: None,
            });
        }

        // Select file based on quantization preference
        let selected = if let Some(quant) = quantization {
            let quant_upper = quant.to_uppercase();
            gguf_files
                .iter()
                .find(|f| f.quantization.as_ref().map(|q| q.to_uppercase() == quant_upper).unwrap_or(false))
                .or_else(|| {
                    // Fuzzy match: try contains
                    gguf_files.iter().find(|f| f.filename.to_uppercase().contains(&quant_upper))
                })
                .ok_or_else(|| crate::Error::Provider {
                    message: format!(
                        "no GGUF file matching quantization '{quant}' in {model_id}. Available: {}",
                        gguf_files
                            .iter()
                            .filter_map(|f| f.quantization.as_ref())
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    status: None,
                })?
        } else {
            // Default: pick the first (smallest) file
            &gguf_files[0]
        };

        info!(
            "selected: {} ({}, {})",
            selected.filename,
            selected.quantization.as_deref().unwrap_or("unknown"),
            common::format_bytes(selected.size_bytes),
        );

        // Create local cache path: {cache_dir}/{org}/{repo}/{filename}
        let local_path = self.cache_dir.join(model_id).join(&selected.filename);

        // Check if already cached
        if local_path.exists() {
            let metadata = std::fs::metadata(&local_path)?;
            if metadata.len() == selected.size_bytes {
                info!("already cached at {}", local_path.display());
                return Ok(PulledModel {
                    model_id: model_id.to_string(),
                    filename: selected.filename.clone(),
                    local_path,
                    size_bytes: selected.size_bytes,
                    quantization: selected.quantization.clone(),
                });
            }
            // Size mismatch — re-download
            warn!("cached file size mismatch, re-downloading");
        }

        // Ensure parent directory exists
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Download
        let download_url = format!("https://huggingface.co/{}/resolve/main/{}", model_id, selected.filename);
        let resp = self.authed_get(&download_url).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(crate::Error::provider_with_status(status.as_u16(), format!("download failed: {body}")));
        }

        let total_size = resp.content_length().unwrap_or(selected.size_bytes);

        // Stream to file with progress
        let mut file = tokio::fs::File::create(&local_path).await?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;

        use futures_lite::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| crate::Error::Streaming {
                message: format!("download stream error: {e}"),
            })?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(ref cb) = progress {
                cb(downloaded, total_size);
            }
        }
        file.flush().await?;

        info!("downloaded {} to {}", common::format_bytes(downloaded), local_path.display());

        Ok(PulledModel {
            model_id: model_id.to_string(),
            filename: selected.filename.clone(),
            local_path,
            size_bytes: downloaded,
            quantization: selected.quantization.clone(),
        })
    }

    /// List locally cached (pulled) models.
    pub fn list_cached(&self) -> Vec<PulledModel> {
        let mut models = Vec::new();
        if !self.cache_dir.exists() {
            return models;
        }

        // Walk cache_dir looking for .gguf files
        if let Ok(entries) = walkdir(&self.cache_dir) {
            for entry in entries {
                if entry.extension().is_some_and(|e| e == "gguf") {
                    let rel = entry.strip_prefix(&self.cache_dir).unwrap_or(&entry);
                    // The model_id is the first two path components (org/repo)
                    let parts: Vec<_> = rel.components().collect();
                    if parts.len() >= 3 {
                        let model_id = format!(
                            "{}/{}",
                            parts[0].as_os_str().to_string_lossy(),
                            parts[1].as_os_str().to_string_lossy(),
                        );
                        let filename =
                            parts[2..].iter().map(|p| p.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                        let size = std::fs::metadata(&entry).map(|m| m.len()).unwrap_or(0);
                        let quant = parse_quantization(&filename);
                        models.push(PulledModel {
                            model_id,
                            filename,
                            local_path: entry,
                            size_bytes: size,
                            quantization: quant,
                        });
                    }
                }
            }
        }
        models
    }

    /// Remove a cached model file.
    pub fn remove_cached(&self, model_id: &str) -> Result<Vec<PathBuf>> {
        let model_dir = self.cache_dir.join(model_id);
        if !model_dir.exists() {
            return Ok(Vec::new());
        }
        let mut removed = Vec::new();
        if let Ok(entries) = walkdir(&model_dir) {
            for entry in entries {
                if entry.is_file() {
                    removed.push(entry.clone());
                    std::fs::remove_file(&entry)?;
                }
            }
        }
        // Clean up empty directories
        let _ = std::fs::remove_dir_all(&model_dir);
        Ok(removed)
    }

    // ── Ollama Integration ──────────────────────────────────────────

    /// Register a pulled GGUF model with Ollama.
    ///
    /// Creates a Modelfile and runs `ollama create` to make the model
    /// available for local inference.
    pub async fn register_with_ollama(&self, pulled: &PulledModel, ollama_name: Option<&str>) -> Result<String> {
        // Check if ollama is available
        let ollama_check = tokio::process::Command::new("ollama").arg("--version").output().await;

        if ollama_check.is_err() {
            return Err(crate::Error::Config {
                message: "ollama not found in PATH. Install from https://ollama.com".into(),
            });
        }

        // Generate a nice model name for ollama
        let name = ollama_name
            .map(String::from)
            .unwrap_or_else(|| generate_ollama_name(&pulled.model_id, pulled.quantization.as_deref()));

        // Create a Modelfile
        let modelfile_content = format!("FROM {}\n", pulled.local_path.display());
        let modelfile_path = pulled.local_path.with_extension("Modelfile");
        std::fs::write(&modelfile_path, &modelfile_content)?;

        info!("creating ollama model '{}' from {}", name, pulled.local_path.display());

        // Run ollama create
        let output = tokio::process::Command::new("ollama")
            .arg("create")
            .arg(&name)
            .arg("-f")
            .arg(&modelfile_path)
            .output()
            .await
            .map_err(|e| crate::Error::Provider {
                message: format!("ollama create failed: {e}"),
                status: None,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::Provider {
                message: format!("ollama create failed: {stderr}"),
                status: None,
            });
        }

        info!("registered ollama model: {}", name);
        Ok(name)
    }

    // ── Internal ────────────────────────────────────────────────────

    async fn authed_get(&self, url: &str) -> std::result::Result<reqwest::Response, reqwest::Error> {
        let mut builder = self.client.get(url);
        if let Some(ref key) = self.api_key {
            builder = builder.header("authorization", format!("Bearer {key}"));
        }
        builder.send().await
    }
}

// ── Hub API types ───────────────────────────────────────────────────────

/// Model info returned from Hub search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelInfo {
    /// Full model ID (e.g., "meta-llama/Llama-3.3-70B-Instruct")
    #[serde(rename = "modelId", alias = "id")]
    pub model_id: String,

    /// Author/organization (e.g., "meta-llama")
    #[serde(default)]
    pub author: Option<String>,

    /// Number of downloads
    #[serde(default)]
    pub downloads: u64,

    /// Number of likes
    #[serde(default)]
    pub likes: u64,

    /// Tags (e.g., ["text-generation", "pytorch", "llama"])
    #[serde(default)]
    pub tags: Vec<String>,

    /// Pipeline tag (e.g., "text-generation")
    #[serde(default, rename = "pipeline_tag")]
    pub pipeline_tag: Option<String>,

    /// Last modified timestamp
    #[serde(default, rename = "lastModified")]
    pub last_modified: Option<String>,

    /// Whether the model is private
    #[serde(default)]
    pub private: bool,

    /// Whether the model is gated (requires accepting terms)
    #[serde(default)]
    pub gated: Option<serde_json::Value>,
}

impl HubModelInfo {
    /// Whether the model requires accepting a license before access
    pub fn is_gated(&self) -> bool {
        self.gated.as_ref().map(|v| !v.is_null() && v.as_bool() != Some(false)).unwrap_or(false)
    }

    /// Format download count for display
    pub fn downloads_display(&self) -> String {
        if self.downloads >= 1_000_000 {
            format!("{:.1}M", self.downloads as f64 / 1_000_000.0)
        } else if self.downloads >= 1_000 {
            format!("{:.1}K", self.downloads as f64 / 1_000.0)
        } else {
            self.downloads.to_string()
        }
    }
}

/// Detailed model information from the Hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelDetail {
    #[serde(rename = "modelId", alias = "id")]
    pub model_id: String,

    #[serde(default)]
    pub author: Option<String>,

    #[serde(default)]
    pub downloads: u64,

    #[serde(default)]
    pub likes: u64,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default, rename = "pipeline_tag")]
    pub pipeline_tag: Option<String>,

    #[serde(default)]
    pub library_name: Option<String>,

    #[serde(default)]
    pub siblings: Vec<HubSibling>,

    #[serde(default)]
    pub card_data: Option<serde_json::Value>,
}

/// A file in a model repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSibling {
    #[serde(rename = "rfilename")]
    pub filename: String,
}

/// A file entry from the tree listing API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubFile {
    /// Relative file path (e.g., "model-Q4_K_M.gguf")
    #[serde(rename = "path")]
    pub path: String,

    /// File type ("file" or "directory")
    #[serde(rename = "type")]
    pub file_type: String,

    /// File size in bytes
    #[serde(default)]
    pub size: u64,

    /// OID/hash
    #[serde(default)]
    pub oid: Option<String>,
}

/// Information about a GGUF file in a model repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufFileInfo {
    /// Filename (e.g., "model-Q4_K_M.gguf")
    pub filename: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// Parsed quantization level (e.g., "Q4_K_M", "Q8_0")
    pub quantization: Option<String>,
}

/// A locally pulled (downloaded) model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulledModel {
    /// HuggingFace model ID (e.g., "bartowski/Llama-3.3-70B-Instruct-GGUF")
    pub model_id: String,
    /// Filename of the downloaded file
    pub filename: String,
    /// Local file path
    pub local_path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Quantization level, if detected
    pub quantization: Option<String>,
}

/// Convert a pulled model to a router Model definition for local serving.
impl PulledModel {
    pub fn to_local_model(&self, ollama_name: Option<&str>) -> Model {
        let name = ollama_name
            .map(String::from)
            .unwrap_or_else(|| generate_ollama_name(&self.model_id, self.quantization.as_deref()));
        Model {
            id: name.clone(),
            name: format!("{} (local)", self.model_id),
            provider: "local".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Default cache directory for downloaded models.
///
/// Uses `$XDG_CACHE_HOME/clankers-router/models` or `~/.cache/clankers-router/models`.
fn default_cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache")).join("clankers-router").join("models")
}

/// Parse quantization level from a GGUF filename.
///
/// Looks for patterns like: Q4_K_M, Q5_K_S, Q8_0, IQ4_XS, etc.
fn parse_quantization(filename: &str) -> Option<String> {
    // Common GGUF quantization patterns
    let patterns = [
        "IQ1_M", "IQ1_S", "IQ2_M", "IQ2_S", "IQ2_XS", "IQ2_XXS", "IQ3_M", "IQ3_S", "IQ3_XS", "IQ3_XXS", "IQ4_NL",
        "IQ4_XS", "Q2_K", "Q2_K_S", "Q3_K_L", "Q3_K_M", "Q3_K_S", "Q4_0", "Q4_1", "Q4_K_L", "Q4_K_M", "Q4_K_S", "Q5_0",
        "Q5_1", "Q5_K_L", "Q5_K_M", "Q5_K_S", "Q6_K", "Q6_K_L", "Q8_0", "F16", "F32", "BF16",
    ];

    let upper = filename.to_uppercase();
    // Try longest patterns first
    let mut sorted_patterns = patterns.to_vec();
    sorted_patterns.sort_by_key(|p| std::cmp::Reverse(p.len()));

    for pattern in sorted_patterns {
        if upper.contains(pattern) {
            return Some(pattern.to_string());
        }
    }
    None
}

/// Generate a nice Ollama model name from a HF model ID.
fn generate_ollama_name(model_id: &str, quantization: Option<&str>) -> String {
    // "bartowski/Llama-3.3-70B-Instruct-GGUF" → "llama-3.3-70b-instruct"
    let name = model_id
        .split('/')
        .next_back()
        .unwrap_or(model_id)
        .to_lowercase()
        .replace("-gguf", "")
        .replace("_gguf", "");

    if let Some(quant) = quantization {
        format!("{}:{}", name, quant.to_lowercase())
    } else {
        name
    }
}

/// Simple recursive directory walker (avoids pulling in the `walkdir` crate).
fn walkdir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

// ── Model catalog helpers ───────────────────────────────────────────────

/// Convert Hub search results into router Model definitions.
///
/// This lets you dynamically add models discovered from the Hub to the
/// router's registry.
pub fn hub_models_to_catalog(hub_models: &[HubModelInfo]) -> Vec<Model> {
    hub_models
        .iter()
        .filter(|m| m.pipeline_tag.as_deref() == Some("text-generation"))
        .map(|m| Model {
            id: m.model_id.clone(),
            name: format!("{} (HF, {} downloads)", m.model_id, m.downloads_display()),
            provider: "huggingface".into(),
            max_input_tokens: 128_000, // conservative default
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hf_config() {
        let config = OpenAICompatConfig::huggingface("hf_test123".into());
        assert_eq!(config.name, "huggingface");
        assert!(config.base_url.contains("huggingface.co"));
        assert!(!config.models.is_empty());
    }

    #[test]
    fn test_default_models() {
        let models = default_hf_models();
        assert!(!models.is_empty());
        for m in &models {
            assert_eq!(m.provider, "huggingface");
            assert!(m.id.contains('/'), "model ID should be org/name format: {}", m.id);
        }
    }

    #[test]
    fn test_parse_quantization() {
        assert_eq!(parse_quantization("model-Q4_K_M.gguf"), Some("Q4_K_M".into()));
        assert_eq!(parse_quantization("model-Q8_0.gguf"), Some("Q8_0".into()));
        assert_eq!(parse_quantization("model-IQ4_XS.gguf"), Some("IQ4_XS".into()));
        assert_eq!(parse_quantization("model-F16.gguf"), Some("F16".into()));
        assert_eq!(parse_quantization("model.gguf"), None);
        // Longer patterns should match first
        assert_eq!(parse_quantization("model-Q4_K_M-extra.gguf"), Some("Q4_K_M".into()));
    }

    #[test]
    fn test_generate_ollama_name() {
        assert_eq!(
            generate_ollama_name("bartowski/Llama-3.3-70B-Instruct-GGUF", Some("Q4_K_M")),
            "llama-3.3-70b-instruct:q4_k_m"
        );
        assert_eq!(generate_ollama_name("TheBloke/Mistral-7B-v0.1-GGUF", None), "mistral-7b-v0.1");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(common::format_bytes(512), "512 B");
        assert_eq!(common::format_bytes(1_048_576), "1.0 MB");
        assert_eq!(common::format_bytes(4_294_967_296), "4.0 GB");
    }

    #[test]
    fn test_hub_model_info_gated() {
        let info = HubModelInfo {
            model_id: "test/model".into(),
            author: None,
            downloads: 0,
            likes: 0,
            tags: vec![],
            pipeline_tag: None,
            last_modified: None,
            private: false,
            gated: Some(serde_json::json!(true)),
        };
        assert!(info.is_gated());

        let info2 = HubModelInfo {
            gated: None,
            ..info.clone()
        };
        assert!(!info2.is_gated());

        let info3 = HubModelInfo {
            gated: Some(serde_json::json!(false)),
            ..info
        };
        assert!(!info3.is_gated());
    }

    #[test]
    fn test_downloads_display() {
        let info = HubModelInfo {
            model_id: "test".into(),
            author: None,
            downloads: 1_500_000,
            likes: 0,
            tags: vec![],
            pipeline_tag: None,
            last_modified: None,
            private: false,
            gated: None,
        };
        assert_eq!(info.downloads_display(), "1.5M");

        let info2 = HubModelInfo {
            downloads: 42_000,
            ..info.clone()
        };
        assert_eq!(info2.downloads_display(), "42.0K");

        let info3 = HubModelInfo { downloads: 500, ..info };
        assert_eq!(info3.downloads_display(), "500");
    }

    #[test]
    fn test_hub_models_to_catalog() {
        let hub_models = vec![
            HubModelInfo {
                model_id: "org/text-gen-model".into(),
                author: Some("org".into()),
                downloads: 1000,
                likes: 10,
                tags: vec![],
                pipeline_tag: Some("text-generation".into()),
                last_modified: None,
                private: false,
                gated: None,
            },
            HubModelInfo {
                model_id: "org/image-model".into(),
                author: Some("org".into()),
                downloads: 500,
                likes: 5,
                tags: vec![],
                pipeline_tag: Some("image-classification".into()),
                last_modified: None,
                private: false,
                gated: None,
            },
        ];
        let catalog = hub_models_to_catalog(&hub_models);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].id, "org/text-gen-model");
        assert_eq!(catalog[0].provider, "huggingface");
    }

    #[test]
    fn test_pulled_model_to_local() {
        let pulled = PulledModel {
            model_id: "bartowski/Llama-3.3-70B-Instruct-GGUF".into(),
            filename: "Llama-3.3-70B-Instruct-Q4_K_M.gguf".into(),
            local_path: PathBuf::from("/tmp/model.gguf"),
            size_bytes: 4_000_000_000,
            quantization: Some("Q4_K_M".into()),
        };
        let model = pulled.to_local_model(None);
        assert_eq!(model.id, "llama-3.3-70b-instruct:q4_k_m");
        assert_eq!(model.provider, "local");

        let model2 = pulled.to_local_model(Some("my-llama"));
        assert_eq!(model2.id, "my-llama");
    }
}
