//! File utilities (gitignore-aware walking)
//!
//! Uses the `ignore` crate (same library ripgrep uses) for directory walking
//! that automatically respects `.gitignore`, `.ignore`, and hidden-file rules.

use std::fs::File;
use std::io::Read;
use std::io::Result;
use std::path::Path;
use std::path::PathBuf;

use ignore::WalkBuilder;

/// Check if a file appears to be binary by scanning for null bytes.
///
/// Reads up to the first 8KB of the file and returns `true` if any null bytes
/// are found, which typically indicates a binary file.
///
/// # Examples
///
/// ```no_run
/// use clankers_util::fs::is_binary_file;
/// use std::path::Path;
///
/// let is_binary = is_binary_file(Path::new("example.txt"))?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn is_binary_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = file.read(&mut buffer)?;

    // Check for null bytes in the portion we read
    Ok(buffer[..bytes_read].contains(&0))
}

/// Options for directory walking.
#[derive(Debug, Clone)]
pub struct WalkOptions {
    /// Include hidden files/directories (default: false)
    pub include_hidden: bool,
    /// Maximum depth to recurse (None = unlimited)
    pub max_depth: Option<usize>,
    /// Respect `.gitignore` rules (default: true)
    pub use_gitignore: bool,
    /// Respect `.ignore` files (default: true)
    pub use_ignore: bool,
    /// Follow symbolic links (default: false)
    pub follow_symlinks: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            max_depth: None,
            use_gitignore: true,
            use_ignore: true,
            follow_symlinks: false,
        }
    }
}

/// Walk a directory and return all file paths, respecting `.gitignore` rules.
///
/// Uses the `ignore` crate for fast, correct gitignore-aware traversal.
/// By default, hidden files and gitignored files are excluded.
///
/// # Examples
///
/// ```no_run
/// use clankers_util::fs::walk_directory;
/// use std::path::Path;
///
/// let files = walk_directory(Path::new("."));
/// // Returns all non-hidden, non-ignored files
/// ```
pub fn walk_directory(root: &Path) -> Vec<PathBuf> {
    walk_directory_with_options(root, &WalkOptions::default())
}

/// Walk a directory with custom options.
///
/// # Examples
///
/// ```no_run
/// use clankers_util::fs::{walk_directory_with_options, WalkOptions};
/// use std::path::Path;
///
/// let opts = WalkOptions {
///     include_hidden: true,
///     max_depth: Some(3),
///     ..Default::default()
/// };
/// let files = walk_directory_with_options(Path::new("."), &opts);
/// ```
pub fn walk_directory_with_options(root: &Path, opts: &WalkOptions) -> Vec<PathBuf> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!opts.include_hidden)
        .git_ignore(opts.use_gitignore)
        .git_global(opts.use_gitignore)
        .git_exclude(opts.use_gitignore)
        .ignore(opts.use_ignore)
        .follow_links(opts.follow_symlinks)
        .sort_by_file_name(|a, b| a.cmp(b));

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth));
    }

    builder
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .map(|entry| entry.into_path())
        .collect()
}

/// Ensure a given entry exists in the `.gitignore` file at `project_root`.
///
/// If the `.gitignore` already contains the entry (exact line match), this is a
/// no-op. Otherwise the entry is appended. The file is created if it doesn't
/// exist. Errors are silently ignored (best-effort).
pub fn ensure_gitignore_entry(project_root: &Path, entry: &str) {
    let gitignore = project_root.join(".gitignore");
    if let Ok(contents) = std::fs::read_to_string(&gitignore)
        && contents.lines().any(|line| line.trim() == entry)
    {
        return;
    }
    // Append the entry (with a leading newline to be safe)
    let mut file = match std::fs::OpenOptions::new().create(true).append(true).open(&gitignore) {
        Ok(f) => f,
        Err(_) => return,
    };
    use std::io::Write;
    let _ = writeln!(file, "{}", entry);
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_is_binary_file_text() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "This is a text file")?;

        let is_binary = is_binary_file(file.path())?;
        assert!(!is_binary);
        Ok(())
    }

    #[test]
    fn test_is_binary_file_binary() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(&[0x00, 0x01, 0x02, 0x03])?;

        let is_binary = is_binary_file(file.path())?;
        assert!(is_binary);
        Ok(())
    }

    fn create_test_tree(dir: &Path, files: &[&str]) {
        for f in files {
            let path = dir.join(f);
            std::fs::create_dir_all(path.parent().expect("test path should have parent"))
                .expect("test dir creation should succeed");
            std::fs::write(&path, format!("content of {f}")).expect("test file write should succeed");
        }
    }

    fn relative_paths(root: &Path, paths: &[PathBuf]) -> Vec<String> {
        let mut result: Vec<String> = paths
            .iter()
            .filter_map(|p| p.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        result.sort();
        result
    }

    #[test]
    fn test_walk_directory_basic() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        create_test_tree(tmp.path(), &["a.txt", "sub/b.txt", "sub/deep/c.rs"]);

        let files = walk_directory(tmp.path());
        let names = relative_paths(tmp.path(), &files);
        assert_eq!(names, vec!["a.txt", "sub/b.txt", "sub/deep/c.rs"]);
    }

    #[test]
    fn test_walk_directory_respects_gitignore() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        // The ignore crate needs a .git dir to recognise .gitignore files
        std::fs::create_dir(tmp.path().join(".git")).expect("test .git dir creation should succeed");
        create_test_tree(tmp.path(), &["keep.rs", "build/output.o", "target/debug/bin"]);
        std::fs::write(tmp.path().join(".gitignore"), "build/\ntarget/\n")
            .expect("test .gitignore write should succeed");

        let files = walk_directory(tmp.path());
        let names = relative_paths(tmp.path(), &files);
        // .gitignore itself is hidden, so only keep.rs should appear
        assert_eq!(names, vec!["keep.rs"]);
    }

    #[test]
    fn test_walk_directory_excludes_hidden() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        create_test_tree(tmp.path(), &["visible.txt", ".hidden_file", ".hidden_dir/secret.txt"]);

        let files = walk_directory(tmp.path());
        let names = relative_paths(tmp.path(), &files);
        assert_eq!(names, vec!["visible.txt"]);
    }

    #[test]
    fn test_walk_directory_include_hidden() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        create_test_tree(tmp.path(), &["visible.txt", ".hidden_file"]);

        let opts = WalkOptions {
            include_hidden: true,
            ..Default::default()
        };
        let files = walk_directory_with_options(tmp.path(), &opts);
        let names = relative_paths(tmp.path(), &files);
        assert!(names.contains(&"visible.txt".to_string()));
        assert!(names.contains(&".hidden_file".to_string()));
    }

    #[test]
    fn test_walk_directory_max_depth() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        create_test_tree(tmp.path(), &["top.txt", "a/mid.txt", "a/b/deep.txt"]);

        // depth 1 = root dir only (files directly in root)
        let opts = WalkOptions {
            max_depth: Some(1),
            ..Default::default()
        };
        let files = walk_directory_with_options(tmp.path(), &opts);
        let names = relative_paths(tmp.path(), &files);
        assert_eq!(names, vec!["top.txt"]);

        // depth 2 = root + one level of subdirs
        let opts2 = WalkOptions {
            max_depth: Some(2),
            ..Default::default()
        };
        let files2 = walk_directory_with_options(tmp.path(), &opts2);
        let names2 = relative_paths(tmp.path(), &files2);
        assert_eq!(names2, vec!["a/mid.txt", "top.txt"]);
    }

    #[test]
    fn test_walk_directory_ignore_file() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        create_test_tree(tmp.path(), &["keep.txt", "skip.log"]);
        std::fs::write(tmp.path().join(".ignore"), "*.log\n").expect("test .ignore write should succeed");

        let files = walk_directory(tmp.path());
        let names = relative_paths(tmp.path(), &files);
        assert_eq!(names, vec!["keep.txt"]);
    }

    #[test]
    fn test_walk_directory_empty() {
        let tmp = TempDir::new().expect("tempdir creation should succeed");
        let files = walk_directory(tmp.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_walk_directory_nonexistent() {
        let files = walk_directory(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(files.is_empty());
    }
}
