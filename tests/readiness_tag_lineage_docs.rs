const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");

const TAG_LINEAGE: &[ReadinessTag] = &[
    ReadinessTag::new(
        "internal-readiness-2026-05-25",
        "44aadbdd2842e5ca10b5665b4372814b69cdc8b0",
        "Fix clankers runtime tigerstyle readiness",
    ),
    ReadinessTag::new(
        "internal-readiness-2026-05-26",
        "a9724c1881c443075af470ef3fa0c37c0a1a7b76",
        "Add background process TUI dogfood rail",
    ),
    ReadinessTag::new(
        "internal-readiness-2026-05-26-dogfood-full",
        "ccec74b659dc588934378aed34638b333304695f",
        "Promote BG process TUI dogfood to readiness",
    ),
];

#[derive(Clone, Copy)]
struct ReadinessTag {
    name: &'static str,
    target: &'static str,
    subject: &'static str,
}

impl ReadinessTag {
    const fn new(name: &'static str, target: &'static str, subject: &'static str) -> Self {
        Self { name, target, subject }
    }
}

#[test]
fn release_readiness_docs_record_tag_lineage_targets_and_boundaries() {
    validate_lineage_doc(RELEASE_READINESS).expect("release-readiness docs should record tag lineage");
}

#[test]
fn release_readiness_lineage_checker_reports_stale_target() {
    let stale_doc = RELEASE_READINESS
        .replace("ccec74b659dc588934378aed34638b333304695f", "0000000000000000000000000000000000000000");

    let error = validate_lineage_doc(&stale_doc).expect_err("stale documented target should fail");

    assert!(
        error.contains("internal-readiness-2026-05-26-dogfood-full"),
        "failure should name the tag with stale docs, got {error:?}"
    );
    assert!(
        error.contains("ccec74b659dc588934378aed34638b333304695f"),
        "failure should name expected target, got {error:?}"
    );
}

fn validate_lineage_doc(doc: &str) -> Result<(), String> {
    let mut missing = Vec::new();
    if !doc.contains("## Readiness tag lineage") {
        missing.push("missing readiness tag lineage section".to_string());
    }
    if !doc.contains("This lineage audit does not move existing tags") {
        missing.push("missing no-tag-move boundary".to_string());
    }
    if !doc.contains("not covered by those tags") {
        missing.push("missing later-commit evidence boundary".to_string());
    }
    if !doc.contains("rerun `./scripts/test-harness.sh full`") {
        missing.push("missing fresh full-harness requirement".to_string());
    }

    for tag in TAG_LINEAGE {
        for expected in [tag.name, tag.target, tag.subject] {
            if !doc.contains(expected) {
                missing.push(format!("{} missing {expected}", tag.name));
            }
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing.join(", "))
    }
}
