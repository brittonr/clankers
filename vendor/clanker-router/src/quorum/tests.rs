use super::*;

// ── Text similarity tests ───────────────────────────────────────

#[test]
fn test_similarity_identical() {
    assert!((text_similarity("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_similarity_case_insensitive() {
    assert!(text_similarity("Hello World", "hello world") > 0.99);
}

#[test]
fn test_similarity_whitespace_normalised() {
    assert!(text_similarity("hello   world", "hello world") > 0.99);
}

#[test]
fn test_similarity_completely_different() {
    assert!(text_similarity("aaaaaa", "zzzzzz") < 0.1);
}

#[test]
fn test_similarity_partial_overlap() {
    let sim = text_similarity("the quick brown fox", "the quick red fox");
    assert!(sim > 0.5, "expected >0.5, got {sim}");
    assert!(sim < 1.0, "expected <1.0, got {sim}");
}

#[test]
fn test_similarity_empty() {
    // Two empty strings are trivially identical
    assert!((text_similarity("", "") - 1.0).abs() < f64::EPSILON);
    // One empty, one non-empty: no bigrams to compare
    assert!((text_similarity("a", "") - 0.0).abs() < f64::EPSILON);
    assert!((text_similarity("", "a") - 0.0).abs() < f64::EPSILON);
}

// ── Clustering tests ────────────────────────────────────────────

#[test]
fn test_cluster_identical() {
    let texts = &["hello world", "hello world", "hello world"];
    let clusters = cluster_by_similarity(texts, 0.8);
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0].len(), 3);
}

#[test]
fn test_cluster_two_groups() {
    let texts = &[
        "the answer is 42",
        "the answer is 42",
        "I don't know the answer",
        "I have no idea",
    ];
    let clusters = cluster_by_similarity(texts, 0.6);
    assert!(clusters.len() >= 2, "expected ≥2 clusters, got {}", clusters.len());
    // Largest cluster should be the "42" pair
    assert_eq!(clusters[0].len(), 2);
}

#[test]
fn test_cluster_all_different() {
    let texts = &["aaa", "bbb", "ccc"];
    let clusters = cluster_by_similarity(texts, 0.9);
    assert_eq!(clusters.len(), 3);
}

#[test]
fn test_cluster_representative() {
    let texts = &["the answer is 42", "the answer is 42!", "the answer is forty-two"];
    let clusters = cluster_by_similarity(texts, 0.5);
    let rep = cluster_representative(texts, &clusters[0]);
    // The representative should be one of the first two (most similar to each other)
    assert!(rep < 2, "expected rep 0 or 1, got {rep}");
}

// ── QuorumTarget builder tests ──────────────────────────────────

#[test]
fn test_target_models() {
    let target = QuorumTarget::models(["claude-sonnet", "gpt-4o", "deepseek-chat"]);
    assert_eq!(target.len(), 3);
    assert_eq!(target.slots[0].model, "claude-sonnet");
    assert_eq!(target.slots[1].model, "gpt-4o");
    assert!(target.slots[0].temperature.is_none());
}

#[test]
fn test_target_replicas() {
    let target = QuorumTarget::replicas("claude-sonnet", 5);
    assert_eq!(target.len(), 5);
    for slot in &target.slots {
        assert_eq!(slot.model, "claude-sonnet");
    }
    assert_eq!(target.slots[0].label.as_deref(), Some("replica-1"));
    assert_eq!(target.slots[4].label.as_deref(), Some("replica-5"));
}

#[test]
fn test_target_temperature_spread() {
    let target = QuorumTarget::replicas("sonnet", 3).with_temperature_spread(0.0, 1.0);
    assert!((target.slots[0].temperature.unwrap() - 0.0).abs() < f64::EPSILON);
    assert!((target.slots[1].temperature.unwrap() - 0.5).abs() < f64::EPSILON);
    assert!((target.slots[2].temperature.unwrap() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_target_temperature_uniform() {
    let target = QuorumTarget::replicas("sonnet", 3).with_temperature(0.7);
    for slot in &target.slots {
        assert!((slot.temperature.unwrap() - 0.7).abs() < f64::EPSILON);
    }
}

// ── Consensus evaluation tests ──────────────────────────────────

fn mock_response(model: &str, text: &str) -> MultiResponse {
    use crate::streaming::ContentDelta;
    use crate::streaming::StreamEvent;

    MultiResponse {
        model: model.into(),
        provider: "test".into(),
        events: vec![StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: text.into() },
        }],
        usage: Usage::default(),
        duration_ms: 100,
        error: None,
    }
}

fn mock_error(model: &str) -> MultiResponse {
    MultiResponse {
        model: model.into(),
        provider: "test".into(),
        events: vec![],
        usage: Usage::default(),
        duration_ms: 50,
        error: Some("failed".into()),
    }
}

#[test]
fn test_unanimous_all_agree() {
    let responses = vec![
        mock_response("a", "the answer is 42"),
        mock_response("b", "the answer is 42"),
        mock_response("c", "the answer is 42"),
    ];
    let (_winner, agreeing, agreement) = evaluate_unanimous(&responses, 0.8, 2);
    assert_eq!(agreeing, 3);
    assert!((agreement - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_unanimous_disagree() {
    let responses = vec![
        mock_response("a", "the answer is 42"),
        mock_response("b", "the answer is completely unknown"),
        mock_response("c", "the answer is 42"),
    ];
    let (_, agreeing, _) = evaluate_unanimous(&responses, 0.8, 2);
    assert_eq!(agreeing, 1); // unanimity broken
}

#[test]
fn test_majority_picks_largest_cluster() {
    let responses = vec![
        mock_response("a", "the answer is 42"),
        mock_response("b", "the answer is 42"),
        mock_response("c", "I think the answer is probably 99"),
    ];
    let (winner, agreeing, agreement) = evaluate_majority(&responses, 0.7, 2);
    assert_eq!(agreeing, 2);
    assert!(agreement > 0.5);
    // Winner should be from the "42" cluster
    assert!(responses[winner].text().contains("42"));
}

#[test]
fn test_majority_skips_errors() {
    let responses = vec![
        mock_error("a"),
        mock_response("b", "the answer is 42"),
        mock_response("c", "the answer is 42"),
    ];
    let (_, agreeing, _) = evaluate_majority(&responses, 0.7, 2);
    assert_eq!(agreeing, 2);
}

// ── Judge prompt tests ──────────────────────────────────────────

#[test]
fn test_build_judge_prompt() {
    let prompt = build_judge_prompt(
        "What is 2+2?",
        &[(0, "claude", "4"), (1, "gpt-4o", "The answer is 4.")],
        "mathematical correctness",
    );
    assert!(prompt.contains("What is 2+2?"));
    assert!(prompt.contains("Response 1"));
    assert!(prompt.contains("Response 2"));
    assert!(prompt.contains("claude"));
    assert!(prompt.contains("mathematical correctness"));
    assert!(prompt.contains("winner"));
}

#[test]
fn test_parse_judge_response_valid() {
    let json = r#"{"winner": 2, "reasoning": "more complete", "agreement": 0.8}"#;
    let (winner, reasoning, agreement) = parse_judge_response(json).unwrap();
    assert_eq!(winner, 1); // 0-based
    assert_eq!(reasoning, "more complete");
    assert!((agreement - 0.8).abs() < f64::EPSILON);
}

#[test]
fn test_parse_judge_response_with_markdown_fences() {
    let text = "Here is my evaluation:\n```json\n{\"winner\": 1, \"reasoning\": \"correct\", \"agreement\": 0.9}\n```";
    let (winner, _, _) = parse_judge_response(text).unwrap();
    assert_eq!(winner, 0); // 1-based → 0-based
}

#[test]
fn test_parse_judge_response_invalid() {
    assert!(parse_judge_response("no json here").is_none());
    assert!(parse_judge_response("{broken").is_none());
}

// ── ConsensusStrategy display ───────────────────────────────────

#[test]
fn test_strategy_display() {
    assert!(ConsensusStrategy::Collect.to_string().contains("collect"));
    assert!(
        ConsensusStrategy::Majority {
            similarity_threshold: 0.7
        }
        .to_string()
        .contains("majority")
    );
}
