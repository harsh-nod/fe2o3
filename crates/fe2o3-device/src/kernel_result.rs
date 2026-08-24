//! Recoverable source-level failures for device kernels.

/// A failure that prevents one kernel invocation from completing safely.
///
/// Returning an error ends the current invocation at the physical unit-return
/// kernel boundary. The variants describe source intent; compiler verification
/// remains responsible for proving memory, synchronization, and ownership
/// safety for every reachable path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
#[rustc_diagnostic_item = "fe2o3_device_kernel_error_v1"]
pub enum KernelError {
    /// A runtime argument does not satisfy the kernel's source contract.
    InvalidArgument = 1,
    /// A checked index or region is outside its backing allocation.
    OutOfBounds = 2,
    /// The requested operation is unavailable for the active configuration.
    Unsupported = 3,
}

/// The ordinary Rust result type accepted by typed kernel entry functions.
pub type KernelResult<T = ()> = Result<T, KernelError>;

#[cfg(test)]
mod tests {
    use super::{KernelError, KernelResult};

    fn checked_index(index: usize, len: usize) -> KernelResult<usize> {
        (index < len)
            .then_some(index)
            .ok_or(KernelError::OutOfBounds)
    }

    fn checked_kernel_path(index: usize, len: usize) -> KernelResult {
        let _ = checked_index(index, len)?;
        Ok(())
    }

    #[test]
    fn native_question_mark_propagates_kernel_errors() {
        assert_eq!(checked_kernel_path(1, 2), Ok(()));
        assert_eq!(checked_kernel_path(2, 2), Err(KernelError::OutOfBounds));
    }

    #[test]
    fn error_codes_have_a_stable_device_representation() {
        assert_eq!(KernelError::InvalidArgument as u32, 1);
        assert_eq!(KernelError::OutOfBounds as u32, 2);
        assert_eq!(KernelError::Unsupported as u32, 3);
    }
}
