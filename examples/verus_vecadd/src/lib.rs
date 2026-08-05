#![no_std]
#![forbid(unsafe_code)]

use fe2o3_contracts::{IdentityWriteIndex, LaunchDomain1d, ThreadInDomain1d};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecAddError {
    DomainLengthMismatch,
    ArithmeticOverflow,
}

/// Executes the work assigned to one logical thread.
pub fn vecadd_thread(
    domain: LaunchDomain1d,
    thread: ThreadInDomain1d,
    a: &[u32],
    b: &[u32],
    output: &mut [u32],
) -> Result<(), VecAddError> {
    if thread.domain() != domain
        || a.len() != domain.len()
        || b.len() != domain.len()
        || output.len() != domain.len()
    {
        return Err(VecAddError::DomainLengthMismatch);
    }

    let write =
        IdentityWriteIndex::new(thread, output.len()).ok_or(VecAddError::DomainLengthMismatch)?;
    let index = write.index().value();
    let value = a[index]
        .checked_add(b[index])
        .ok_or(VecAddError::ArithmeticOverflow)?;
    output[index] = value;
    Ok(())
}

/// CPU reference driver over the same per-thread contract.
pub fn vecadd(a: &[u32], b: &[u32], output: &mut [u32]) -> Result<(), VecAddError> {
    if a.len() != b.len() || a.len() != output.len() {
        return Err(VecAddError::DomainLengthMismatch);
    }

    let domain = LaunchDomain1d::new(output.len());
    for linear in 0..domain.len() {
        let thread = domain
            .thread(linear)
            .ok_or(VecAddError::DomainLengthMismatch)?;
        vecadd_thread(domain, thread, a, b, output)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vecadd_executes_one_disjoint_write_per_thread() {
        let a = [1, 2, 3, 4];
        let b = [10, 20, 30, 40];
        let mut output = [0; 4];

        assert_eq!(vecadd(&a, &b, &mut output), Ok(()));
        assert_eq!(output, [11, 22, 33, 44]);
    }

    #[test]
    fn vecadd_rejects_a_domain_buffer_mismatch() {
        let domain = LaunchDomain1d::new(3);
        let thread = domain.thread(0).unwrap();
        let mut output = [0; 2];

        assert_eq!(
            vecadd_thread(domain, thread, &[1, 2, 3], &[4, 5, 6], &mut output),
            Err(VecAddError::DomainLengthMismatch)
        );
    }

    #[test]
    fn vecadd_reports_integer_overflow() {
        let mut output = [0];

        assert_eq!(
            vecadd(&[u32::MAX], &[1], &mut output),
            Err(VecAddError::ArithmeticOverflow)
        );
        assert_eq!(output, [0]);
    }
}
