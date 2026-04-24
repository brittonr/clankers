//! Logging setup utilities.
//!
//! Extracted from `main.rs` to keep the entry point thin and make the
//! iroh/wasmtime filter list maintainable in one place.

use tracing_subscriber::EnvFilter;

/// Noisy crate targets that are silenced unless the user explicitly sets `RUST_LOG`.
const SILENCED_TARGETS: &[&str] = &[
    "iroh",
    "iroh_base",
    "iroh_blobs",
    "iroh_io",
    "iroh_metrics",
    "iroh_quinn",
    "iroh_quinn_proto",
    "iroh_quinn_udp",
    "iroh_relay",
    "iroh_tickets",
    "netwatch",
    "portmapper",
    "netlink_packet_route",
    "swarm_discovery",
    "wasmtime",
    "wasmtime_internal_cache",
    "extism",
];

/// Build the tracing [`EnvFilter`] for the application.
///
/// If `RUST_LOG` is set in the environment, it is used as-is.
/// Otherwise, the given `default_level` is used and noisy transitive
/// dependencies (iroh, quinn, wasmtime, etc.) are silenced to `error`.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "static filter directive strings are valid")
)]
pub fn build_env_filter(default_level: tracing::Level) -> EnvFilter {
    if std::env::var("RUST_LOG").is_ok() {
        return EnvFilter::from_default_env();
    }

    let mut filter = EnvFilter::new("").add_directive(default_level.into());
    for target in SILENCED_TARGETS {
        filter = filter.add_directive(format!("{}=error", target).parse().expect("static directive"));
    }
    filter
}
