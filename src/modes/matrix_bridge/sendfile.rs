//! Sendfile tag extraction and Matrix upload.

use tracing::{info, warn};

/// A file the agent wants to send back to the user.
pub(crate) struct SendfileTag {
    /// Absolute path to the file
    pub(crate) path: String,
}

/// Extract `<sendfile>/path</sendfile>` tags from response text.
/// Returns the cleaned text (tags stripped) and a list of file paths.
pub(crate) fn extract_sendfile_tags(text: &str) -> (String, Vec<SendfileTag>) {
    let mut cleaned = String::with_capacity(text.len());
    let mut tags = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<sendfile>") {
        // Copy text before the tag
        cleaned.push_str(&remaining[..start]);

        let after_open = &remaining[start + "<sendfile>".len()..];
        if let Some(end) = after_open.find("</sendfile>") {
            let path = after_open[..end].trim().to_string();
            if !path.is_empty() {
                tags.push(SendfileTag { path });
            }
            remaining = &after_open[end + "</sendfile>".len()..];
        } else {
            // Unclosed tag — keep it as-is (prefix already pushed above)
            cleaned.push_str("<sendfile>");
            remaining = after_open;
        }
    }
    cleaned.push_str(remaining);

    (cleaned, tags)
}

/// Guess MIME type from file extension.
pub(crate) fn guess_mime(path: &std::path::Path) -> mime::Mime {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "png" => mime::IMAGE_PNG,
        "jpg" | "jpeg" => mime::IMAGE_JPEG,
        "gif" => mime::IMAGE_GIF,
        "webp" => "image/webp".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "svg" => mime::IMAGE_SVG,
        "mp4" => "video/mp4".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "webm" => "video/webm".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "mp3" => "audio/mpeg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "ogg" => "audio/ogg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "wav" => "audio/wav".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "pdf" => mime::APPLICATION_PDF,
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "toml" | "yaml" | "yml" | "json" => mime::TEXT_PLAIN,
        _ => mime::APPLICATION_OCTET_STREAM,
    }
}

/// Check whether a path is safe to send over Matrix.
///
/// Blocks known sensitive directories and files to prevent the agent from
/// accidentally exfiltrating credentials, keys, or system secrets.
pub(crate) fn is_sendfile_path_allowed(path: &std::path::Path) -> std::result::Result<(), String> {
    // Canonicalize to resolve symlinks and ../ tricks
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {e}"))?;
    let s = canonical.to_string_lossy();

    // Blocked directory prefixes (home-relative)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let blocked_dirs = [
            ".ssh",
            ".gnupg",
            ".gpg",
            ".aws",
            ".azure",
            ".config/gcloud",
            ".kube",
            ".docker",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".clankers/matrix.json",
        ];
        for dir in &blocked_dirs {
            let blocked = format!("{}/{}", home_str, dir);
            if s.starts_with(&blocked) {
                return Err(format!("blocked: path inside ~/{dir}"));
            }
        }
    }

    // Blocked system paths
    let blocked_system = [
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/master.passwd",
        "/etc/sudoers",
    ];
    for bp in &blocked_system {
        if s.as_ref() == *bp || s.starts_with(&format!("{bp}.")) {
            return Err(format!("blocked: sensitive system file {bp}"));
        }
    }

    // Block private key files by name pattern
    let filename = canonical
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let blocked_names = [
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        "id_dsa",
        ".env",
        ".env.local",
        ".env.production",
    ];
    for bn in &blocked_names {
        if filename == *bn {
            return Err(format!("blocked: sensitive file name {bn}"));
        }
    }

    Ok(())
}

/// Upload sendfile tags to Matrix and return error annotations for failures.
pub(crate) async fn upload_sendfiles(
    client: &tokio::sync::RwLock<clankers_matrix::MatrixClient>,
    room_id: &clankers_matrix::ruma::OwnedRoomId,
    tags: &[SendfileTag],
) -> Vec<String> {
    let mut errors = Vec::new();

    for tag in tags {
        let path = std::path::Path::new(&tag.path);

        if !path.exists() || !path.is_file() {
            errors.push(format!("(failed to send file {}: file not found)", tag.path));
            continue;
        }

        // Path validation: block sensitive files
        if let Err(reason) = is_sendfile_path_allowed(path) {
            warn!("Sendfile blocked: {} ({})", tag.path, reason);
            errors.push(format!("(refused to send file {}: {})", tag.path, reason));
            continue;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!(
                    "(failed to send file {}: {})",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    e
                ));
                continue;
            }
        };

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let content_type = guess_mime(path);

        let c = client.read().await;
        if let Err(e) = c.send_file(room_id, &filename, &content_type, data).await {
            errors.push(format!("(failed to send file {}: {})", filename, e));
        } else {
            info!("Uploaded file to Matrix: {}", filename);
        }
    }

    errors
}
