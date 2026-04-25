//! Confidence scoring for experiment improvements.

#[derive(Debug, Clone)]
pub struct ConfidenceResult {
    pub score: f64,
    pub noise_floor: f64,
    pub delta: f64,
}

pub fn compute_confidence(kept_metrics: &[f64], current: f64, minimize: bool) -> Option<ConfidenceResult> {
    if kept_metrics.len() < 3 {
        return None;
    }

    let n = kept_metrics.len() as f64;
    let mean = kept_metrics.iter().sum::<f64>() / n;
    let variance = kept_metrics.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let noise_floor = variance.sqrt();

    if noise_floor < f64::EPSILON {
        return Some(ConfidenceResult {
            score: f64::INFINITY,
            noise_floor: 0.0,
            delta: current - mean,
        });
    }

    let best = if minimize {
        kept_metrics.iter().copied().reduce(f64::min)?
    } else {
        kept_metrics.iter().copied().reduce(f64::max)?
    };

    let delta = if minimize { best - current } else { current - best };

    let score = delta.abs() / noise_floor;

    Some(ConfidenceResult {
        score,
        noise_floor,
        delta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_under_3_runs() {
        assert!(compute_confidence(&[1.0, 2.0], 3.0, false).is_none());
        assert!(compute_confidence(&[], 1.0, false).is_none());
    }

    #[test]
    fn maximize_positive_delta() {
        let kept = vec![10.0, 11.0, 12.0];
        let result = compute_confidence(&kept, 15.0, false).unwrap();
        assert!(result.delta > 0.0);
        assert!(result.score > 0.0);
    }

    #[test]
    fn minimize_positive_delta() {
        let kept = vec![10.0, 11.0, 12.0];
        let result = compute_confidence(&kept, 8.0, true).unwrap();
        assert!(result.delta > 0.0);
        assert!(result.score > 0.0);
    }

    #[test]
    fn noisy_vs_clean_data() {
        let clean = vec![10.0, 10.0, 10.0];
        let noisy = vec![5.0, 10.0, 15.0];

        let clean_result = compute_confidence(&clean, 11.0, false).unwrap();
        let noisy_result = compute_confidence(&noisy, 16.0, false).unwrap();

        assert!(
            clean_result.score > noisy_result.score,
            "clean data should show higher confidence for same improvement"
        );
    }

    #[test]
    fn zero_variance_returns_infinity() {
        let kept = vec![10.0, 10.0, 10.0];
        let result = compute_confidence(&kept, 11.0, false).unwrap();
        assert!(result.score.is_infinite());
    }
}
