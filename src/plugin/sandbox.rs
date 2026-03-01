//! Plugin sandbox (permission model)

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "fs:read")]
    FsRead,
    #[serde(rename = "fs:write")]
    FsWrite,
    #[serde(rename = "net")]
    Net,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "ui")]
    Ui,
}

/// Check if a set of permissions includes a specific permission
pub fn has_permission(granted: &[String], required: Permission) -> bool {
    let required_str = match required {
        Permission::FsRead => "fs:read",
        Permission::FsWrite => "fs:write",
        Permission::Net => "net",
        Permission::Exec => "exec",
        Permission::Ui => "ui",
    };
    granted.iter().any(|p| p == required_str || p == "all")
}
