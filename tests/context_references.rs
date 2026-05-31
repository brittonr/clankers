use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::process::Command;
use std::thread;

use clankers_session::SessionManager;
use clankers_session::entry::SessionEntry;
use clankers_util::at_file::ContextReferenceKind;
use clankers_util::at_file::ContextReferencePolicy;
use clankers_util::at_file::ContextReferenceStatus;
use clankers_util::at_file::expand_at_refs_with_images;
use clankers_util::at_file::expand_at_refs_with_policy;

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
    assert!(expanded.text.contains("URL references are disabled by policy"));
    assert_eq!(expanded.references.len(), 1);
    assert_eq!(expanded.references[0].kind, ContextReferenceKind::Unsupported);
    assert_eq!(expanded.references[0].status, ContextReferenceStatus::Unsupported);
    assert_eq!(expanded.references[0].target, "https:");
}

#[test]
fn context_reference_git_diff_expands_and_records_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let status = Command::new("git").current_dir(tmp.path()).args(["init"]).status().unwrap();
    assert!(status.success());
    let file = tmp.path().join("notes.txt");
    std::fs::write(&file, "alpha\n").unwrap();
    assert!(Command::new("git").current_dir(tmp.path()).args(["add", "notes.txt"]).status().unwrap().success());
    assert!(
        Command::new("git")
            .current_dir(tmp.path())
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "init"
            ])
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(&file, "alpha\nbeta\n").unwrap();

    let expanded = expand_at_refs_with_images("review @diff", tmp.path().to_str().unwrap());

    assert!(expanded.text.contains("+beta"));
    assert_eq!(expanded.references.len(), 1);
    assert_eq!(expanded.references[0].kind, ContextReferenceKind::GitDiff);
    assert_eq!(expanded.references[0].status, ContextReferenceStatus::Expanded);
    assert_eq!(expanded.references[0].target, "git:diff");
}

#[test]
fn context_reference_url_fetch_expands_when_policy_allows() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 512];
        let _ = stream.read(&mut buf);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nurl content!")
            .unwrap();
    });
    let policy = ContextReferencePolicy {
        allow_url_fetch: true,
        ..ContextReferencePolicy::default()
    };
    let prompt = format!("fetch @http://{addr}/note");

    let expanded = expand_at_refs_with_policy(&prompt, "/tmp", &policy);
    handle.join().unwrap();

    assert!(expanded.text.contains("url content!"));
    assert_eq!(expanded.references.len(), 1);
    assert_eq!(expanded.references[0].kind, ContextReferenceKind::Url);
    assert_eq!(expanded.references[0].status, ContextReferenceStatus::Expanded);
    assert_eq!(expanded.references[0].target, "http:");
}
