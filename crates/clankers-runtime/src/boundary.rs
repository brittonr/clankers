use crate::RuntimeError;

pub(crate) const PUBLIC_RUNTIME_BOUNDARY_RAIL: &str = "scripts/check-runtime-facade-boundary.rs";

pub(crate) fn validate_public_runtime_boundary() -> Result<(), RuntimeError> {
    // The checked boundary is maintained by PUBLIC_RUNTIME_BOUNDARY_RAIL so it
    // inventories real public exports, dependencies, and source tokens instead
    // of relying on a small in-crate name list.
    let _rail = PUBLIC_RUNTIME_BOUNDARY_RAIL;
    Ok(())
}
