#![allow(
    dead_code,
    reason = "shared integration-test helpers are used by different test binaries"
)]

use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

pub struct ReadinessSandbox {
    _home: tempfile::TempDir,
    _work: tempfile::TempDir,
    pub home: PathBuf,
    pub work: PathBuf,
}

impl ReadinessSandbox {
    pub fn new() -> Self {
        let home = tempfile::TempDir::new().expect("temp home should be creatable");
        let work = tempfile::TempDir::new().expect("temp workdir should be creatable");
        let home_path = home.path().to_path_buf();
        std::fs::create_dir_all(home_path.join(".config")).expect("config dir should be creatable");
        std::fs::create_dir_all(home_path.join(".cache")).expect("cache dir should be creatable");
        std::fs::create_dir_all(home_path.join(".local/share")).expect("data dir should be creatable");
        std::fs::create_dir_all(home_path.join(".run")).expect("runtime dir should be creatable");
        Self {
            home: home_path,
            work: work.path().to_path_buf(),
            _home: home,
            _work: work,
        }
    }

    pub fn clankers(&self) -> ReadinessCommand {
        let mut command = ReadinessCommand::new(clankers_bin());
        command
            .current_dir(&self.work)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_RUNTIME_DIR", self.home.join(".run"))
            .env("CLANKERS_NO_DAEMON", "1")
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "off");
        command
    }
}

pub struct ReadinessCommand {
    command: Command,
    timeout: Duration,
}

impl ReadinessCommand {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let mut command = Command::new(program);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        Self {
            command,
            timeout: Duration::from_secs(90),
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    pub fn run(&mut self) -> CapturedOutput {
        let mut child = self.command.spawn().expect("readiness child should spawn");
        let started = Instant::now();
        loop {
            if child.try_wait().expect("readiness child should poll").is_some() {
                let output = child.wait_with_output().expect("readiness child output should collect");
                return CapturedOutput {
                    status: output.status.code().unwrap_or(-1),
                    timed_out: false,
                    stdout: redact(&String::from_utf8_lossy(&output.stdout)),
                    stderr: redact(&String::from_utf8_lossy(&output.stderr)),
                };
            }
            if started.elapsed() > self.timeout {
                let _ = child.kill();
                let output = child.wait_with_output().expect("timed out child output should collect");
                return CapturedOutput {
                    status: -1,
                    timed_out: true,
                    stdout: redact(&String::from_utf8_lossy(&output.stdout)),
                    stderr: redact(&String::from_utf8_lossy(&output.stderr)),
                };
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

pub struct CapturedOutput {
    pub status: i32,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CapturedOutput {
    pub fn assert_success(&self) {
        assert!(
            self.status == 0 && !self.timed_out,
            "command failed: status={} timed_out={}\nstdout:\n{}\nstderr:\n{}",
            self.status,
            self.timed_out,
            self.stdout,
            self.stderr
        );
    }

    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

pub fn clankers_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_clankers")
        .map(PathBuf::from)
        .expect("CARGO_BIN_EXE_clankers should be set for integration tests")
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn skip_unless_env(var: &str) -> bool {
    match std::env::var(var) {
        Ok(value) if value == "1" || value.eq_ignore_ascii_case("true") => false,
        _ => {
            println!("skip: set {var}=1 to run this opt-in readiness check");
            true
        }
    }
}

pub fn redact(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|token| {
                    let lower = token.to_ascii_lowercase();
                    let redacted = lower.contains("secret")
                        || lower.contains("password")
                        || lower.contains("apikey")
                        || lower.contains("api_key")
                        || lower.starts_with("sk-")
                        || lower.starts_with("bearer");
                    if redacted { "[REDACTED]" } else { token }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
