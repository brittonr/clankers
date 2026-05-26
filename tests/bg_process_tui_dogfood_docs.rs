const README: &str = include_str!("../README.md");
const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");
const DOGFOOD_FULL_EVIDENCE: &str = include_str!("../docs/src/reference/internal-readiness-2026-05-26-dogfood-full.md");
const DOGFOOD_RAIL: &str = include_str!("../scripts/check-bg-process-tui-dogfood.rs");
const TEST_HARNESS: &str = include_str!("../scripts/test-harness.sh");

const CANONICAL_COMMAND: &str = "./scripts/test-harness.sh dogfood bg-process-tui";
const HARNESS_SELECTOR: &str = "run_dogfood_selector bg-process-tui";
const RUNTIME_BOUNDARY: &str = "real Clankers TUI in tmux";

const REQUIRED_RECEIPT_CRITERIA: &[RequiredCriterion] = &[
    RequiredCriterion::new("result: pass", &["result: pass", "Result: `pass`"]),
    RequiredCriterion::new("layout_toggle_bg_visible: true", &[
        "layout_toggle_bg_visible: true",
        "`/layout toggle bg` visibility: `true`",
    ]),
    RequiredCriterion::new("active_processes_observed > 0", &[
        "active_processes_observed > 0",
        "Active processes observed: `1`",
    ]),
    RequiredCriterion::new("command_visible: true", &["command_visible: true", "Bounded command visibility: `true`"]),
    RequiredCriterion::new("sentinel_processes_cleaned_up: true", &[
        "sentinel_processes_cleaned_up: true",
        "Sentinel process cleanup: `true`",
    ]),
];

#[derive(Clone, Copy)]
struct RequiredCriterion {
    label: &'static str,
    accepted_phrases: &'static [&'static str],
}

impl RequiredCriterion {
    const fn new(label: &'static str, accepted_phrases: &'static [&'static str]) -> Self {
        Self {
            label,
            accepted_phrases,
        }
    }
}

#[test]
fn bg_process_tui_dogfood_docs_match_harness_and_receipt_contract() {
    assert!(TEST_HARNESS.contains(HARNESS_SELECTOR), "full harness should run the canonical dogfood selector");
    assert!(
        TEST_HARNESS.contains("./scripts/check-bg-process-tui-dogfood.rs"),
        "dogfood selector should dispatch to the maintained rail"
    );
    for receipt_field in [
        "result",
        "layout_toggle_bg_visible",
        "active_processes_observed",
        "active_title",
        "command_visible",
        "bounded_command_seconds",
        "sentinel_processes_cleaned_up",
    ] {
        assert!(DOGFOOD_RAIL.contains(receipt_field), "dogfood rail should keep receipt field {receipt_field:?}");
    }

    validate_dogfood_command_doc("README", README).expect("README should document canonical dogfood command");
    validate_dogfood_doc("release-readiness", RELEASE_READINESS)
        .expect("release-readiness docs should document the dogfood receipt contract");
    validate_dogfood_doc("dogfood-full evidence", DOGFOOD_FULL_EVIDENCE)
        .expect("dogfood-full evidence should document observed dogfood receipt facts");
}

#[test]
fn bg_process_tui_dogfood_doc_checker_reports_missing_required_criterion() {
    let incomplete_doc = format!(
        "{CANONICAL_COMMAND}\n{RUNTIME_BOUNDARY}\nresult: pass\nlayout_toggle_bg_visible: true\ncommand_visible: true\nsentinel_processes_cleaned_up: true\n"
    );

    let error = validate_dogfood_doc("negative fixture", &incomplete_doc)
        .expect_err("omitting active_processes_observed should fail deterministically");

    assert!(
        error.contains("missing active_processes_observed > 0"),
        "negative fixture should name the missing criterion, got {error:?}"
    );
}

#[test]
fn bg_process_tui_dogfood_doc_checker_does_not_run_live_rail() {
    let valid_doc = format!(
        "{CANONICAL_COMMAND}\n{RUNTIME_BOUNDARY}\n{}\n",
        REQUIRED_RECEIPT_CRITERIA
            .iter()
            .map(|criterion| criterion.accepted_phrases[0])
            .collect::<Vec<_>>()
            .join("\n")
    );

    validate_dogfood_doc("synthetic valid fixture", &valid_doc)
        .expect("synthetic fixture should validate without launching tmux or live models");
}

fn validate_dogfood_command_doc(name: &str, doc: &str) -> Result<(), String> {
    if doc.contains(CANONICAL_COMMAND) || doc.contains("dogfood bg-process-tui") {
        Ok(())
    } else {
        Err(format!("{name}: missing canonical command {CANONICAL_COMMAND:?}"))
    }
}

fn validate_dogfood_doc(name: &str, doc: &str) -> Result<(), String> {
    let mut missing = Vec::new();
    if !doc.contains(CANONICAL_COMMAND) && !doc.contains("dogfood bg-process-tui") {
        missing.push(format!("missing canonical command {CANONICAL_COMMAND:?}"));
    }
    if !doc.contains(RUNTIME_BOUNDARY) && !doc.contains("real TUI") {
        missing.push(format!("missing runtime-boundary text {RUNTIME_BOUNDARY:?}"));
    }
    for criterion in REQUIRED_RECEIPT_CRITERIA {
        if !criterion.accepted_phrases.iter().any(|phrase| doc.contains(phrase)) {
            missing.push(format!("missing {}", criterion.label));
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("{name}: {}", missing.join(", ")))
    }
}
