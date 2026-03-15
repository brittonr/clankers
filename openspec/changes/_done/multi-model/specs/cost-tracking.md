# CostTracker — Per-Model Cost Tracking and Budget Enforcement

## Overview

The `CostTracker` records token usage for each model used in a session,
calculates cost based on per-model pricing, and enforces budget thresholds.
It emits warnings when soft limits are approached and provides aggregate
cost summaries for the TUI and agent tools.

## Data Structures

### CostTracker

```rust
pub struct CostTracker {
    /// Usage per model
    usage: Arc<RwLock<HashMap<String, ModelUsage>>>,

    /// Pricing table (model_id → ModelPricing)
    pricing: HashMap<String, ModelPricing>,

    /// Budget configuration
    config: CostTrackerConfig,

    /// Event bus for emitting budget warnings
    event_tx: mpsc::UnboundedSender<AgentEvent>,
}

pub struct CostTrackerConfig {
    /// Soft budget limit (warn but don't enforce)
    soft_limit: Option<f32>,  // USD

    /// Hard budget limit (downgrade to cheaper models)
    hard_limit: Option<f32>,  // USD

    /// Warn at regular intervals (e.g., every $1)
    warning_interval: Option<f32>,

    /// Whether to emit cost events on the agent bus
    emit_events: bool,
}

struct ModelUsage {
    model_id: String,
    input_tokens: u64,
    output_tokens: u64,
    total_turns: u64,
    cost_usd: f64,
}

struct ModelPricing {
    /// Cost per million input tokens
    input_per_mtok: f64,

    /// Cost per million output tokens
    output_per_mtok: f64,

    /// Display name for UI
    display_name: String,
}
```

### CostSummary

```rust
pub struct CostSummary {
    /// Total cost across all models
    total_cost: f64,

    /// Per-model breakdown
    by_model: Vec<ModelCostBreakdown>,

    /// Budget status
    budget_status: BudgetStatus,

    /// Most expensive model used this session
    most_expensive: Option<String>,
}

struct ModelCostBreakdown {
    model_id: String,
    display_name: String,
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: f64,
    percentage: f32,  // % of total cost
}

enum BudgetStatus {
    /// No budget configured
    NoBudget,

    /// Under soft limit
    Ok { remaining: f64 },

    /// Over soft limit, under hard limit
    Warning { over_soft_by: f64, hard_limit_remaining: f64 },

    /// Over hard limit
    Exceeded { over_hard_by: f64 },
}
```

## Pricing Table

Default pricing for Anthropic models (as of March 2025):

```rust
fn default_pricing() -> HashMap<String, ModelPricing> {
    [
        ("claude-opus-4", ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            display_name: "Claude Opus 4".into(),
        }),
        ("claude-opus-4-20250514", ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            display_name: "Claude Opus 4 (2025-05-14)".into(),
        }),
        ("claude-sonnet-4", ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            display_name: "Claude Sonnet 4".into(),
        }),
        ("claude-sonnet-3-5-20241022", ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            display_name: "Claude Sonnet 3.5".into(),
        }),
        ("claude-haiku-3-5-20241022", ModelPricing {
            input_per_mtok: 0.8,
            output_per_mtok: 4.0,
            display_name: "Claude Haiku 3.5".into(),
        }),
        ("claude-haiku-4", ModelPricing {
            input_per_mtok: 1.0,
            output_per_mtok: 5.0,
            display_name: "Claude Haiku 4".into(),
        }),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}
```

Pricing is loaded from `~/.clankers/pricing.json` if present, else defaults.

## Behavior

### Recording Usage

```rust
impl CostTracker {
    /// Record token usage from an API response
    pub fn record_usage(&self, model_id: &str, input_tokens: u64, output_tokens: u64) {
        let pricing = self.pricing.get(model_id).expect("unknown model");

        let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok;
        let total_cost = input_cost + output_cost;

        let mut usage = self.usage.write();
        let entry = usage.entry(model_id.to_string()).or_insert(ModelUsage {
            model_id: model_id.to_string(),
            input_tokens: 0,
            output_tokens: 0,
            total_turns: 0,
            cost_usd: 0.0,
        });

        entry.input_tokens += input_tokens;
        entry.output_tokens += output_tokens;
        entry.total_turns += 1;
        entry.cost_usd += total_cost;

        // Check thresholds
        self.check_thresholds(entry.cost_usd);

        // Emit event
        if self.config.emit_events {
            let _ = self.event_tx.send(AgentEvent::CostUpdate {
                model_id: model_id.to_string(),
                input_tokens,
                output_tokens,
                cost_usd: total_cost,
                total_cost: self.total_cost_locked(&usage),
            });
        }
    }

    fn total_cost_locked(&self, usage: &HashMap<String, ModelUsage>) -> f64 {
        usage.values().map(|u| u.cost_usd).sum()
    }

    pub fn total_cost(&self) -> f64 {
        let usage = self.usage.read();
        self.total_cost_locked(&usage)
    }
}
```

### Threshold Checking

```rust
impl CostTracker {
    fn check_thresholds(&self, new_total: f64) {
        // Hard limit check
        if let Some(hard_limit) = self.config.hard_limit {
            if new_total >= hard_limit as f64 {
                let _ = self.event_tx.send(AgentEvent::BudgetExceeded {
                    limit: hard_limit,
                    current: new_total as f32,
                });
                return;  // Don't double-warn
            }
        }

        // Soft limit check
        if let Some(soft_limit) = self.config.soft_limit {
            if new_total >= soft_limit as f64 {
                let _ = self.event_tx.send(AgentEvent::BudgetWarning {
                    threshold: soft_limit,
                    current: new_total as f32,
                });
            }
        }

        // Interval warning (e.g., every $1)
        if let Some(interval) = self.config.warning_interval {
            let prev_milestone = ((new_total - 0.01) / interval as f64).floor() as u32;
            let curr_milestone = (new_total / interval as f64).floor() as u32;
            if curr_milestone > prev_milestone {
                let _ = self.event_tx.send(AgentEvent::CostMilestone {
                    milestone: curr_milestone as f32 * interval,
                    total: new_total as f32,
                });
            }
        }
    }
}
```

### Summary Generation

```rust
impl CostTracker {
    pub fn summary(&self) -> CostSummary {
        let usage = self.usage.read();
        let total_cost = self.total_cost_locked(&usage);

        let by_model: Vec<ModelCostBreakdown> = usage
            .values()
            .map(|u| {
                let pricing = self.pricing.get(&u.model_id).unwrap();
                ModelCostBreakdown {
                    model_id: u.model_id.clone(),
                    display_name: pricing.display_name.clone(),
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cost_usd: u.cost_usd,
                    percentage: (u.cost_usd / total_cost * 100.0) as f32,
                }
            })
            .collect();

        let budget_status = self.compute_budget_status(total_cost);

        let most_expensive = by_model
            .iter()
            .max_by(|a, b| a.cost_usd.partial_cmp(&b.cost_usd).unwrap())
            .map(|m| m.model_id.clone());

        CostSummary {
            total_cost,
            by_model,
            budget_status,
            most_expensive,
        }
    }

    fn compute_budget_status(&self, total: f64) -> BudgetStatus {
        match (self.config.soft_limit, self.config.hard_limit) {
            (None, None) => BudgetStatus::NoBudget,
            (Some(soft), None) => {
                if total < soft as f64 {
                    BudgetStatus::Ok { remaining: soft as f64 - total }
                } else {
                    BudgetStatus::Warning {
                        over_soft_by: total - soft as f64,
                        hard_limit_remaining: f64::INFINITY,
                    }
                }
            }
            (Some(soft), Some(hard)) => {
                if total < soft as f64 {
                    BudgetStatus::Ok { remaining: soft as f64 - total }
                } else if total < hard as f64 {
                    BudgetStatus::Warning {
                        over_soft_by: total - soft as f64,
                        hard_limit_remaining: hard as f64 - total,
                    }
                } else {
                    BudgetStatus::Exceeded {
                        over_hard_by: total - hard as f64,
                    }
                }
            }
            (None, Some(hard)) => {
                if total < hard as f64 {
                    BudgetStatus::Ok { remaining: hard as f64 - total }
                } else {
                    BudgetStatus::Exceeded {
                        over_hard_by: total - hard as f64,
                    }
                }
            }
        }
    }
}
```

## Agent Events

New events added to `AgentEvent`:

```rust
pub enum AgentEvent {
    // ... existing variants ...

    /// Token usage recorded for a model
    CostUpdate {
        model_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        total_cost: f64,
    },

    /// Soft budget threshold reached
    BudgetWarning {
        threshold: f32,
        current: f32,
    },

    /// Hard budget limit exceeded
    BudgetExceeded {
        limit: f32,
        current: f32,
    },

    /// Cost milestone hit (e.g., every $1)
    CostMilestone {
        milestone: f32,
        total: f32,
    },
}
```

## TUI Integration

The status bar displays cost summary:

```
[sonnet] 3.2k in / 1.8k out | $0.17 | Budget: $4.83 / $5.00
```

Color-coded by budget status:
- Green: Under soft limit
- Yellow: Over soft, under hard
- Red: Over hard limit

## Tool Integration

New `cost` tool the agent can call:

```rust
/// Agent tool to inspect cost and budget status
pub struct CostTool {
    tracker: Arc<CostTracker>,
}

impl Tool for CostTool {
    fn name(&self) -> &str { "cost" }

    fn description(&self) -> &str {
        "Check token usage, costs, and budget status for this session"
    }

    fn execute(&self, args: &ToolArgs) -> Result<ToolOutput> {
        let summary = self.tracker.summary();

        let mut output = String::new();
        output.push_str(&format!("Total cost: ${:.3}\n\n", summary.total_cost));

        output.push_str("By model:\n");
        for m in &summary.by_model {
            output.push_str(&format!(
                "  {} — {}k in, {}k out — ${:.3} ({:.1}%)\n",
                m.display_name,
                m.input_tokens / 1000,
                m.output_tokens / 1000,
                m.cost_usd,
                m.percentage
            ));
        }

        output.push_str(&format!("\nBudget status: {}", format_budget_status(&summary.budget_status)));

        Ok(ToolOutput::success(output))
    }
}
```

## File Location

`src/routing/cost_tracker.rs` — new module under `src/routing/`.
