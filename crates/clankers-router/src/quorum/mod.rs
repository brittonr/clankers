//! Quorum dispatch — send the same prompt to multiple models (or replicas of
//! the same model) and determine a consensus result.
//!
//! This builds on top of [`multi`](crate::multi) fan-out dispatch and adds a
//! consensus layer that decides which response "wins".
//!
//! # Strategies
//!
//! - **`Unanimous`** — all successful responses must agree (text similarity ≥ threshold)
//! - **`Majority`** — cluster by text similarity, pick the largest cluster
//! - **`Judge`** — fan out a second LLM call that evaluates all responses and picks the best one,
//!   returning structured reasoning
//! - **`Collect`** — no consensus; just return all responses for the caller to handle
//!
//! # Targets
//!
//! A [`QuorumTarget`] can specify:
//! - A **different model** for each slot (cross-model quorum)
//! - The **same model** repeated N times (replica quorum) with optional per-slot temperature jitter
//!   to encourage diversity
//!
//! # Example
//!
//! ```ignore
//! use clankers_router::quorum::*;
//!
//! let quorum_req = QuorumRequest {
//!     request: base_request,
//!     targets: QuorumTarget::replicas("claude-sonnet-4-5", 3)
//!         .with_temperature_spread(0.3, 1.0),
//!     consensus: ConsensusStrategy::Majority {
//!         similarity_threshold: 0.7,
//!     },
//!     min_agree: 2,
//! };
//! let result = router.complete_quorum(quorum_req).await?;
//! if result.quorum_met {
//!     println!("consensus: {}", result.winner.text());
//! }
//! ```

use serde::Deserialize;
use serde::Serialize;

use crate::multi::MultiResponse;
use crate::provider::Usage;

// ── Targets ─────────────────────────────────────────────────────────────

/// A single slot in a quorum request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumSlot {
    /// Model ID (resolved through the registry; aliases work).
    pub model: String,
    /// Optional temperature override for this slot.
    /// When `None` the base request's temperature is used.
    pub temperature: Option<f64>,
    /// Human-readable label (e.g. "replica-1", "openai-gpt4o").
    /// Defaults to the model ID if not set.
    pub label: Option<String>,
}

/// Builder for the set of models/replicas to query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumTarget {
    pub slots: Vec<QuorumSlot>,
}

impl QuorumTarget {
    /// Query N different models.
    pub fn models(model_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            slots: model_ids
                .into_iter()
                .map(|id| {
                    let id = id.into();
                    QuorumSlot {
                        label: None,
                        model: id,
                        temperature: None,
                    }
                })
                .collect(),
        }
    }

    /// Query the same model `n` times (replica quorum).
    pub fn replicas(model: impl Into<String>, n: usize) -> Self {
        let model = model.into();
        Self {
            slots: (0..n)
                .map(|i| QuorumSlot {
                    model: model.clone(),
                    temperature: None,
                    label: Some(format!("replica-{}", i + 1)),
                })
                .collect(),
        }
    }

    /// Spread temperatures linearly across slots from `lo` to `hi`.
    ///
    /// Useful for replica quorum — low temperature gives deterministic answers,
    /// high gives creative ones; clustering then reveals the "stable" answer.
    pub fn with_temperature_spread(mut self, lo: f64, hi: f64) -> Self {
        let n = self.slots.len();
        if n <= 1 {
            if let Some(slot) = self.slots.first_mut() {
                slot.temperature = Some(lo);
            }
            return self;
        }
        for (i, slot) in self.slots.iter_mut().enumerate() {
            slot.temperature = Some(lo + (hi - lo) * (i as f64 / (n - 1) as f64));
        }
        self
    }

    /// Set the same temperature on every slot.
    pub fn with_temperature(mut self, temp: f64) -> Self {
        for slot in &mut self.slots {
            slot.temperature = Some(temp);
        }
        self
    }

    /// Number of slots.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

// ── Consensus strategy ──────────────────────────────────────────────────

/// How the quorum determines the winning response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusStrategy {
    /// Every successful response must agree (similarity ≥ threshold).
    Unanimous {
        /// Minimum pair-wise similarity to count as "agreeing" (0.0–1.0).
        similarity_threshold: f64,
    },
    /// Cluster responses by similarity, pick the largest cluster's
    /// representative (the one closest to the cluster centroid).
    Majority {
        /// Minimum pair-wise similarity to group two responses (0.0–1.0).
        similarity_threshold: f64,
    },
    /// Use a judge model to evaluate all candidate responses.
    Judge {
        /// Model to use as the judge (resolved through registry).
        judge_model: String,
        /// Evaluation criteria (e.g. "correctness", "completeness and accuracy").
        criteria: String,
    },
    /// No consensus — return all responses and let the caller decide.
    Collect,
}

impl std::fmt::Display for ConsensusStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusStrategy::Unanimous { similarity_threshold } => {
                write!(f, "unanimous(≥{similarity_threshold:.0}%)")
            }
            ConsensusStrategy::Majority { similarity_threshold } => {
                write!(f, "majority(≥{similarity_threshold:.0}%)")
            }
            ConsensusStrategy::Judge { judge_model, .. } => write!(f, "judge({judge_model})"),
            ConsensusStrategy::Collect => write!(f, "collect"),
        }
    }
}

// ── Request ─────────────────────────────────────────────────────────────

/// A request that fans out to a quorum of models and determines consensus.
#[derive(Debug, Clone)]
pub struct QuorumRequest {
    /// The base completion request (model + temperature are overridden per slot).
    pub request: crate::provider::CompletionRequest,
    /// Which models/replicas to query.
    pub targets: QuorumTarget,
    /// How to pick the winning response.
    pub consensus: ConsensusStrategy,
    /// Minimum number of responses that must agree for the quorum to be "met".
    /// Set to 0 to always accept the best available answer.
    pub min_agree: usize,
}

// ── Result ──────────────────────────────────────────────────────────────

/// The outcome of a quorum request.
#[derive(Debug)]
pub struct QuorumResult {
    /// The selected winning response.
    pub winner: MultiResponse,
    /// Index of the winner in `all_responses`.
    pub winner_index: usize,
    /// Every individual response (successful and failed).
    pub all_responses: Vec<MultiResponse>,
    /// How many successful responses agreed with the winner.
    pub agreeing_count: usize,
    /// Agreement ratio (agreeing / total_successful), 0.0–1.0.
    pub agreement: f64,
    /// Whether `agreeing_count >= min_agree`.
    pub quorum_met: bool,
    /// The consensus strategy that was used.
    pub consensus: ConsensusStrategy,
    /// Judge's reasoning (only populated for `Judge` strategy).
    pub judge_reasoning: Option<String>,
    /// Aggregated token usage across *all* responses (including judge call).
    pub total_usage: Usage,
}

// ── Text similarity ─────────────────────────────────────────────────────

/// Normalise text for comparison: trim, collapse whitespace, lowercase.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Compute a simple similarity ratio between two strings (0.0–1.0).
///
/// Uses the Sørensen–Dice coefficient on character bigrams — fast,
/// no allocations beyond two small vecs, and a good enough proxy for
/// "do these two free-text answers say roughly the same thing".
pub fn text_similarity(a: &str, b: &str) -> f64 {
    let a = normalise(a);
    let b = normalise(b);

    if a == b {
        return 1.0;
    }
    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }

    fn bigrams(s: &str) -> Vec<(char, char)> {
        let chars: Vec<char> = s.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    }

    let a_bi = bigrams(&a);
    let b_bi = bigrams(&b);

    if a_bi.is_empty() && b_bi.is_empty() {
        return 1.0;
    }

    let mut matches = 0usize;
    let mut b_used = vec![false; b_bi.len()];
    for a_pair in &a_bi {
        for (j, b_pair) in b_bi.iter().enumerate() {
            if !b_used[j] && a_pair == b_pair {
                matches += 1;
                b_used[j] = true;
                break;
            }
        }
    }

    (2.0 * matches as f64) / (a_bi.len() + b_bi.len()) as f64
}

// ── Clustering ──────────────────────────────────────────────────────────

/// Cluster indices by pair-wise text similarity.
///
/// Returns a list of clusters (each cluster is a vec of indices into `texts`).
/// Clusters are sorted largest-first.
pub fn cluster_by_similarity(texts: &[&str], threshold: f64) -> Vec<Vec<usize>> {
    let n = texts.len();
    if n == 0 {
        return vec![];
    }

    // Single-linkage clustering: assign each text to the first cluster
    // that has a member similar enough.
    let mut clusters: Vec<Vec<usize>> = Vec::new();

    for i in 0..n {
        let mut assigned = false;
        for cluster in &mut clusters {
            // Check similarity against every member of this cluster
            let similar = cluster.iter().all(|&j| text_similarity(texts[i], texts[j]) >= threshold);
            if similar {
                cluster.push(i);
                assigned = true;
                break;
            }
        }
        if !assigned {
            clusters.push(vec![i]);
        }
    }

    // Sort largest cluster first
    clusters.sort_by_key(|c| std::cmp::Reverse(c.len()));
    clusters
}

/// Pick the "representative" of a cluster: the response whose text
/// has the highest average similarity to all other members.
pub fn cluster_representative(texts: &[&str], cluster: &[usize]) -> usize {
    if cluster.len() <= 1 {
        return cluster[0];
    }

    let mut best_idx = cluster[0];
    let mut best_avg = 0.0f64;

    for &i in cluster {
        let avg: f64 = cluster.iter().filter(|&&j| j != i).map(|&j| text_similarity(texts[i], texts[j])).sum::<f64>()
            / (cluster.len() - 1) as f64;
        if avg > best_avg {
            best_avg = avg;
            best_idx = i;
        }
    }

    best_idx
}

// ── Consensus evaluation ────────────────────────────────────────────────

/// Apply the `Unanimous` strategy: all successful responses must agree.
pub(crate) fn evaluate_unanimous(
    responses: &[MultiResponse],
    threshold: f64,
    _min_agree: usize,
) -> (usize, usize, f64) {
    let ok: Vec<usize> = responses.iter().enumerate().filter(|(_, r)| r.is_ok()).map(|(i, _)| i).collect();
    if ok.is_empty() {
        return (0, 0, 0.0);
    }

    let texts: Vec<String> = ok.iter().map(|&i| responses[i].text()).collect();
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

    // Check all pairs
    let mut all_agree = true;
    for i in 0..text_refs.len() {
        for j in (i + 1)..text_refs.len() {
            if text_similarity(text_refs[i], text_refs[j]) < threshold {
                all_agree = false;
                break;
            }
        }
        if !all_agree {
            break;
        }
    }

    let agreeing = if all_agree { ok.len() } else { 1 };
    let agreement = if all_agree { 1.0 } else { 1.0 / ok.len() as f64 };

    // Pick the representative (shortest response that is still complete,
    // or first if unanimous)
    let winner = if all_agree {
        cluster_representative(&text_refs, &(0..ok.len()).collect::<Vec<_>>())
    } else {
        0
    };

    (ok[winner], agreeing, agreement)
}

/// Apply the `Majority` strategy: cluster by similarity, pick the largest cluster.
pub(crate) fn evaluate_majority(responses: &[MultiResponse], threshold: f64, _min_agree: usize) -> (usize, usize, f64) {
    let ok: Vec<usize> = responses.iter().enumerate().filter(|(_, r)| r.is_ok()).map(|(i, _)| i).collect();
    if ok.is_empty() {
        return (0, 0, 0.0);
    }

    let texts: Vec<String> = ok.iter().map(|&i| responses[i].text()).collect();
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

    let clusters = cluster_by_similarity(&text_refs, threshold);
    let largest = &clusters[0];

    let agreeing = largest.len();
    let agreement = agreeing as f64 / ok.len() as f64;
    let rep = cluster_representative(&text_refs, largest);

    (ok[rep], agreeing, agreement)
}

// ── Judge prompt construction ───────────────────────────────────────────

/// Build the prompt sent to the judge model.
pub fn build_judge_prompt(
    original_prompt: &str,
    responses: &[(usize, &str, &str)], // (index, model_name, text)
    criteria: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are evaluating multiple model responses to the same prompt.\n\
         Your job is to select the best response based on the given criteria.\n\n",
    );

    prompt.push_str("## Original prompt\n\n");
    prompt.push_str(original_prompt);
    prompt.push_str("\n\n## Candidate responses\n\n");

    for (idx, model, text) in responses {
        prompt.push_str(&format!("### Response {} (from {})\n\n{}\n\n", idx + 1, model, text));
    }

    prompt.push_str(&format!(
        "## Evaluation criteria\n\n{}\n\n\
         ## Instructions\n\n\
         Compare the responses and select the best one.\n\
         Respond with ONLY a JSON object (no markdown fencing):\n\n\
         {{\n  \
           \"winner\": <1-based response number>,\n  \
           \"reasoning\": \"<brief explanation of why this response is best>\",\n  \
           \"agreement\": <0.0 to 1.0 — how much the responses agree with each other>\n\
         }}",
        criteria
    ));

    prompt
}

/// Parse the judge's JSON response.
pub fn parse_judge_response(text: &str) -> Option<(usize, String, f64)> {
    // Try to find JSON in the response (may be wrapped in markdown fences)
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            return None;
        }
    } else {
        return None;
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let winner = parsed.get("winner")?.as_u64()? as usize;
    let reasoning = parsed.get("reasoning")?.as_str().unwrap_or("").to_string();
    let agreement = parsed.get("agreement").and_then(|v| v.as_f64()).unwrap_or(0.0);

    // Convert from 1-based to 0-based
    Some((winner.saturating_sub(1), reasoning, agreement))
}


#[cfg(test)]
mod tests;
