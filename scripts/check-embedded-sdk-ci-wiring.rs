use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: embedded SDK receipt rail is wired through Rust/Nix checks");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let flake = read("flake.nix")?;
    require(
        flake.contains("embedded-sdk-release-receipt"),
        "flake.nix must expose checks.<system>.embedded-sdk-release-receipt",
    )?;
    require(
        flake.contains("check-embedded-sdk-ci-wiring.rs"),
        "flake.nix embedded SDK check must run the Rust wiring guard",
    )?;

    let receipt = read("scripts/emit-embedded-sdk-release-receipt.rs")?;
    require(
        receipt.contains("scripts/check-embedded-agent-sdk.rs"),
        "release receipt must name the Rust embedded SDK rail",
    )?;
    require(
        receipt.contains("scripts/check-embedded-sdk-ci-wiring.rs"),
        "release receipt must hash the Nix/Rust wiring guard",
    )?;
    require(
        receipt.contains("scripts/check-llm-contract-boundary.rs"),
        "release receipt must hash the Rust LLM boundary guard",
    )?;
    require(
        !receipt.contains("scripts/check-llm-contract-boundary.sh"),
        "release receipt should not depend on the legacy shell LLM boundary guard",
    )?;

    let readme = read("README.md")?;
    require(
        readme.contains("embedded-sdk-release-receipt"),
        "README.md must document the embedded SDK receipt Nix check",
    )?;
    require(
        readme.contains("scripts/check-embedded-agent-sdk.rs"),
        "README.md must document the Rust embedded SDK rail",
    )?;

    let readiness = read("docs/src/reference/release-readiness.md")?;
    require(
        readiness.contains("embedded-sdk-release-receipt"),
        "release-readiness docs must include the embedded SDK receipt Nix check",
    )?;

    Ok(())
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.to_owned()) }
}
