use clankers::clankers_session::SessionManager;
use clankers::clankers_session::entry::SessionEntry;
use clankers::util::at_file::ContextReferenceKind;
use clankers::util::at_file::ContextReferenceStatus;
use clankers::util::at_file::expand_at_refs_with_images;

#[test]
fn context_reference_primary_path_expands_file_and_persists_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().join("project");
    std::fs::create_dir(&cwd).unwrap();
    std::fs::write(cwd.join("notes.md"), "alpha\nbeta\ngamma\n").unwrap();

    let expanded = expand_at_refs_with_images("summarize @notes.md:2", cwd.to_str().unwrap());

    assert!(expanded.text.contains("beta"));
    assert!(expanded.images.is_empty());
    assert_eq!(expanded.references.len(), 1);
    assert_eq!(expanded.references[0].kind, ContextReferenceKind::File);
    assert_eq!(expanded.references[0].status, ContextReferenceStatus::Expanded);

    let session_dir = tmp.path().join("sessions");
    let mut manager =
        SessionManager::create(&session_dir, cwd.to_str().unwrap(), "test-model", None, None, None).unwrap();
    manager
        .record_custom(
            "context_references",
            serde_json::json!({
                "source": "context_references",
                "cwd": cwd,
                "references": expanded.references,
            }),
        )
        .unwrap();

    let reopened = SessionManager::open(manager.file_path().to_path_buf()).unwrap();
    let tree = reopened.load_tree().unwrap();
    let custom = tree.entries().iter().find_map(|entry| match entry {
        SessionEntry::Custom(custom) if custom.kind == "context_references" => Some(custom),
        _ => None,
    });
    let custom = custom.expect("context reference metadata should be persisted");
    assert_eq!(custom.data["source"], "context_references");
    assert_eq!(custom.data["references"][0]["status"], "expanded");
    assert_eq!(custom.data["references"][0]["kind"], "file");
}

#[test]
fn context_reference_unsupported_url_is_actionable_failure() {
    let expanded = expand_at_refs_with_images("read @https://example.com/private", "/tmp");

    assert!(expanded.images.is_empty());
    assert!(expanded.text.contains("Unsupported context reference @https://example.com/private"));
    assert!(expanded.text.contains("URL references are not supported yet"));
    assert_eq!(expanded.references.len(), 1);
    assert_eq!(expanded.references[0].kind, ContextReferenceKind::Unsupported);
    assert_eq!(expanded.references[0].status, ContextReferenceStatus::Unsupported);
    assert_eq!(expanded.references[0].target, "https:");
}
