//! Allocation-free mechanical gate reads shared by protected-service launchers.
//! No descriptor, child, profile or deployment authority is admitted here.

/// Finite logical read-attempt allowance; it does not bound a blocking read's duration.
pub const MAX_CHILD_GATE_ATTEMPTS_V2: usize = 64;

/// Reads one byte within the prepaid attempt limit, retrying only EINTR.
/// Callers must independently validate the exact release byte and channel custody.
pub fn read_child_gate(
    mut read: impl FnMut(&mut u8) -> rustix::io::Result<usize>,
) -> Result<u8, ()> {
    let mut release = 0_u8;
    for _ in 0..MAX_CHILD_GATE_ATTEMPTS_V2 {
        match read(&mut release) {
            Ok(1) => return Ok(release),
            Err(rustix::io::Errno::INTR) => {}
            Ok(_) | Err(_) => return Err(()),
        }
    }
    Err(())
}

#[cfg(test)]
#[path = "pre_exec_tests.rs"]
mod tests;
