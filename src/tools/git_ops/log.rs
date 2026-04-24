//! Git log operations via libgit2.

use git2::Sort;

use super::GitError;
use super::Result;
use super::open_repo;

// ── Log ────────────────────────────────────────────────────────────────

/// A single log entry.
pub struct LogEntry {
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub relative_time: String,
}

/// Walk recent commits (like `git log -N`).
///
/// Returns up to `count` log entries from HEAD.
pub async fn log(count: usize) -> Result<Vec<LogEntry>> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(Sort::TIME)?;

        let mut entries = Vec::with_capacity(count);
        for (i, oid_result) in revwalk.enumerate() {
            if i >= count {
                break;
            }
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            let short = oid.to_string()[..7].to_string();
            let subject = commit.summary().unwrap_or("(no message)").to_string();
            let author = commit.author().name().unwrap_or("unknown").to_string();
            let time = commit.time();
            let relative = format_relative_time(time.seconds());

            entries.push(LogEntry {
                short_hash: short,
                subject,
                author,
                relative_time: relative,
            });
        }

        Ok(entries)
    })
    .await
    .map_err(|e| GitError(format!("join error: {}", e)))?
}

/// Time unit constants for relative time formatting
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3600;
const SECS_PER_DAY: u64 = 86400;
const SECS_PER_WEEK: u64 = 604_800;
const SECS_PER_MONTH: u64 = 2_592_000;
const SECS_PER_YEAR: u64 = 31_536_000;

/// Format a unix timestamp as a relative time string (e.g. "2 hours ago").
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
pub fn format_relative_time(epoch_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let delta = now - epoch_secs;
    if delta < 0 {
        return "in the future".to_string();
    }
    let delta = delta as u64;
    if delta < SECS_PER_MINUTE {
        format!("{} seconds ago", delta)
    } else if delta < SECS_PER_HOUR {
        let m = delta / SECS_PER_MINUTE;
        format!("{} minute{} ago", m, if m == 1 { "" } else { "s" })
    } else if delta < SECS_PER_DAY {
        let h = delta / SECS_PER_HOUR;
        format!("{} hour{} ago", h, if h == 1 { "" } else { "s" })
    } else if delta < SECS_PER_WEEK {
        let d = delta / SECS_PER_DAY;
        format!("{} day{} ago", d, if d == 1 { "" } else { "s" })
    } else if delta < SECS_PER_MONTH {
        let w = delta / SECS_PER_WEEK;
        format!("{} week{} ago", w, if w == 1 { "" } else { "s" })
    } else if delta < SECS_PER_YEAR {
        let m = delta / SECS_PER_MONTH;
        format!("{} month{} ago", m, if m == 1 { "" } else { "s" })
    } else {
        let y = delta / SECS_PER_YEAR;
        format!("{} year{} ago", y, if y == 1 { "" } else { "s" })
    }
}
