//! Fixed-point cost conversion helpers.

pub(crate) fn micros_from_major_units(amount: f64) -> Result<u64, String> {
    if !amount.is_finite() {
        return Err("cost amount must be finite".to_string());
    }
    if amount < 0.0 {
        return Err("cost amount must be non-negative".to_string());
    }
    let scaled = (amount * clanker_message::COST_MICROS_PER_UNIT as f64).round();
    if scaled > u64::MAX as f64 {
        return Err("cost amount is too large".to_string());
    }
    Ok(scaled as u64)
}

pub(crate) fn micros_from_major_units_or_zero(amount: Option<f64>) -> u64 {
    match amount {
        Some(value) => micros_from_major_units(value).unwrap_or(0),
        None => 0,
    }
}

pub(crate) fn major_units_from_micros(micros: u64) -> f64 {
    micros as f64 / clanker_message::COST_MICROS_PER_UNIT as f64
}

pub(crate) fn micros_from_stored_fields(
    micros: Option<u64>,
    legacy_major_units: Option<serde_json::Value>,
) -> Result<u64, String> {
    if let Some(value) = micros {
        return Ok(value);
    }
    if let Some(value) = legacy_major_units {
        return micros_from_legacy_major_units(value);
    }
    Ok(0)
}

fn micros_from_legacy_major_units(value: serde_json::Value) -> Result<u64, String> {
    match value {
        serde_json::Value::Number(number) => {
            let units = number
                .as_f64()
                .ok_or_else(|| "cost amount must be a non-negative number".to_string())?;
            micros_from_major_units(units)
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => Err("cost amount must be numeric".to_string()),
    }
}
