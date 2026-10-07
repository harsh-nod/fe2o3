//! Terminal session custody around one full pair observation.

#![forbid(unsafe_code)]

use super::{MemorySessionError, SharedMemorySessionPhaseV1};

pub(crate) fn with_terminal_pair<T>(
    source: &mut SharedMemorySessionPhaseV1,
    peer: &mut SharedMemorySessionPhaseV1,
    check: impl FnOnce() -> Result<T, MemorySessionError>,
) -> Result<T, MemorySessionError> {
    if *source != SharedMemorySessionPhaseV1::Active || *peer != SharedMemorySessionPhaseV1::Active
    {
        return Err(MemorySessionError::SharedSessionQuarantined);
    }
    // The caller has finished pure binding checks. Retain both sessions on an
    // error or unwind; no fallible terminalizer can replace the original cause.
    *source = SharedMemorySessionPhaseV1::Quarantined;
    *peer = SharedMemorySessionPhaseV1::Quarantined;
    let value = check()?;
    *source = SharedMemorySessionPhaseV1::Active;
    *peer = SharedMemorySessionPhaseV1::Active;
    Ok(value)
}

#[cfg(test)]
mod tests;
