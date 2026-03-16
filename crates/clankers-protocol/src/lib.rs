//! Wire protocol types for daemon-client communication.
//!
//! Two enums — `SessionCommand` (client → daemon) and `DaemonEvent` (daemon →
//! client) — carried over length-prefixed JSON frames on a Unix domain socket
//! or QUIC stream.
//!
//! This crate contains only types, serialization, and frame helpers. Transport
//! bindings live in the daemon and client code.

pub mod command;
pub mod control;
pub mod event;
pub mod frame;
pub mod types;

pub use command::SessionCommand;
pub use control::ControlCommand;
pub use control::ControlResponse;
pub use control::DaemonStatus;
pub use control::SessionSummary;
pub use event::DaemonEvent;
pub use event::ToolInfo;

pub use frame::FrameError;
pub use frame::read_frame;
pub use frame::write_frame;
pub use types::ALPN_DAEMON;
pub use types::AttachResponse;
pub use types::DaemonRequest;
pub use types::Handshake;
pub use types::ImageData;
pub use types::ProcessInfo;
pub use types::ProcessState;
pub use types::SerializedMessage;
pub use types::SessionKey;
