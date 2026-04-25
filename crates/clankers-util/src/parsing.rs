//! Shared parsing utilities.
//!
//! Small parsers that were scattered across `main.rs` and `interactive.rs`.

/// Parse a human-readable duration string into a [`Duration`].
///
/// Supports: `30m`, `1h`, `24h`, `7d`, `30d`, `365d`, `1y`.
pub fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 'm')
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 'h')
    } else if let Some(stripped) = s.strip_suffix('d') {
        (stripped, 'd')
    } else if let Some(stripped) = s.strip_suffix('y') {
        (stripped, 'y')
    } else {
        return None;
    };

    let num: u64 = num_str.parse().ok()?;
    let secs = match unit {
        'm' => num * 60,
        'h' => num * 3600,
        'd' => num * 86400,
        'y' => num * 86400 * 365,
        _ => return None,
    };

    Some(std::time::Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("30m"), Some(std::time::Duration::from_secs(30 * 60)));
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration("1h"), Some(std::time::Duration::from_secs(3600)));
        assert_eq!(parse_duration("24h"), Some(std::time::Duration::from_secs(86400)));
    }

    #[test]
    fn parse_duration_days() {
        assert_eq!(parse_duration("7d"), Some(std::time::Duration::from_secs(7 * 86400)));
    }

    #[test]
    fn parse_duration_years() {
        assert_eq!(parse_duration("1y"), Some(std::time::Duration::from_secs(365 * 86400)));
    }

    #[test]
    fn parse_duration_empty() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("  "), None);
    }

    #[test]
    fn parse_duration_invalid() {
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("5x"), None);
    }
}
