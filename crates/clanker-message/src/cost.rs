//! Neutral cost tracking contracts shared by model-selection and display edges.

/// Number of fixed-point micro-units in one US dollar.
pub const COST_MICROS_PER_UNIT: u64 = 1_000_000;

/// Fixed-point non-negative cost stored as millionths of one US dollar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CostMicros {
    micros: u64,
}

impl CostMicros {
    pub const ZERO: Self = Self { micros: 0 };

    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self { micros }
    }

    #[must_use]
    pub const fn micros(self) -> u64 {
        self.micros
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.micros == 0
    }

    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self {
            micros: self.micros.saturating_add(other.micros),
        }
    }

    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self {
            micros: self.micros.saturating_sub(other.micros),
        }
    }

    #[must_use]
    pub fn format_major_units(self, precision: u32) -> String {
        assert!(precision <= 6, "cost precision is bounded by micro-units");
        let fractional_units = fractional_units_for_precision(precision);
        let rounding_divisor = rounding_divisor_for_precision(precision);
        let rounded_minor_units = divide_by_nonzero(
            self.micros.saturating_add(rounding_divisor.half()),
            rounding_divisor,
        );
        let whole_units = divide_by_nonzero(rounded_minor_units, fractional_units);
        let fractional_part = remainder_by_nonzero(rounded_minor_units, fractional_units);

        if precision == 0 {
            return whole_units.to_string();
        }
        if precision == 1 {
            return format!("{whole_units}.{fractional_part:01}");
        }
        if precision == 2 {
            return format!("{whole_units}.{fractional_part:02}");
        }
        if precision == 3 {
            return format!("{whole_units}.{fractional_part:03}");
        }
        if precision == 4 {
            return format!("{whole_units}.{fractional_part:04}");
        }
        if precision == 5 {
            return format!("{whole_units}.{fractional_part:05}");
        }
        assert!(precision == 6, "cost precision branch must be exhausted");
        format!("{whole_units}.{fractional_part:06}")
    }
}

#[derive(Debug, Clone, Copy)]
struct CostDivisor {
    value: u64,
}

impl CostDivisor {
    fn new(value: u64) -> Self {
        assert!(value != 0, "cost divisor must be non-zero");
        Self { value }
    }

    fn half(self) -> u64 {
        self.value / 2
    }
}

fn fractional_units_for_precision(precision: u32) -> CostDivisor {
    assert!(precision <= 6, "cost precision is bounded by micro-units");
    if precision == 0 {
        return CostDivisor::new(1);
    }
    if precision == 1 {
        return CostDivisor::new(10);
    }
    if precision == 2 {
        return CostDivisor::new(100);
    }
    if precision == 3 {
        return CostDivisor::new(1_000);
    }
    if precision == 4 {
        return CostDivisor::new(10_000);
    }
    if precision == 5 {
        return CostDivisor::new(100_000);
    }
    assert!(precision == 6, "cost precision branch must be exhausted");
    CostDivisor::new(COST_MICROS_PER_UNIT)
}

fn rounding_divisor_for_precision(precision: u32) -> CostDivisor {
    assert!(precision <= 6, "cost precision is bounded by micro-units");
    if precision == 0 {
        return CostDivisor::new(COST_MICROS_PER_UNIT);
    }
    if precision == 1 {
        return CostDivisor::new(100_000);
    }
    if precision == 2 {
        return CostDivisor::new(10_000);
    }
    if precision == 3 {
        return CostDivisor::new(1_000);
    }
    if precision == 4 {
        return CostDivisor::new(100);
    }
    if precision == 5 {
        return CostDivisor::new(10);
    }
    assert!(precision == 6, "cost precision branch must be exhausted");
    CostDivisor::new(1)
}

fn divide_by_nonzero(value: u64, divisor: CostDivisor) -> u64 {
    let divisor_value = divisor.value;
    assert!(divisor_value != 0, "cost divisor must be non-zero");
    value / divisor_value
}

fn remainder_by_nonzero(value: u64, divisor: CostDivisor) -> u64 {
    let divisor_value = divisor.value;
    assert!(divisor_value != 0, "cost divisor must be non-zero");
    value % divisor_value
}

/// Current budget status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetStatus {
    /// No budget configured.
    NoBudget,
    /// Under all limits.
    Ok { remaining: CostMicros },
    /// Over soft limit, under hard limit.
    Warning {
        over_soft_by: CostMicros,
        hard_limit_remaining: Option<CostMicros>,
    },
    /// Over hard limit.
    Exceeded { over_hard_by: CostMicros },
}

/// Budget events emitted after recording usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetEvent {
    /// Soft budget threshold reached.
    Warning { threshold: CostMicros, current: CostMicros },
    /// Hard budget limit exceeded.
    Exceeded { limit: CostMicros, current: CostMicros },
    /// Cost milestone hit (for example every $1).
    Milestone { milestone: CostMicros, total: CostMicros },
}

/// Aggregate cost summary for display and receipts.
#[derive(Debug, Clone)]
pub struct CostSummary {
    /// Total cost across all models.
    pub total_cost: CostMicros,
    /// Per-model breakdown.
    pub by_model: Vec<ModelCostBreakdown>,
    /// Current budget status.
    pub budget_status: BudgetStatus,
    /// Most expensive model this session.
    pub most_expensive: Option<String>,
}

/// Cost breakdown for a single model.
#[derive(Debug, Clone)]
pub struct ModelCostBreakdown {
    pub model_id: String,
    pub display_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: CostMicros,
    /// Percentage of total cost.
    pub percentage: f32,
}

/// Trait for accessing cost data without depending on the concrete tracker.
pub trait CostProvider: Send + Sync {
    /// Full cost summary with per-model breakdown.
    fn summary(&self) -> CostSummary;
    /// Current budget status.
    fn budget_status(&self) -> BudgetStatus;
    /// Total cost across all models.
    fn total_cost(&self) -> CostMicros;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_micros_formats_major_units_with_fixed_precision() {
        let amount = CostMicros::from_micros(1_234_567);

        assert_eq!(amount.format_major_units(0), "1");
        assert_eq!(amount.format_major_units(2), "1.23");
        assert_eq!(amount.format_major_units(4), "1.2346");
        assert_eq!(amount.format_major_units(6), "1.234567");
    }

    #[test]
    fn cost_micros_saturates_arithmetic() {
        let low = CostMicros::from_micros(3);
        let high = CostMicros::from_micros(u64::MAX);

        assert_eq!(low.saturating_sub(CostMicros::from_micros(5)), CostMicros::ZERO);
        assert_eq!(high.saturating_add(CostMicros::from_micros(1)), high);
    }
}
