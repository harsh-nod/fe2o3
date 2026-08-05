#![no_std]
#![forbid(unsafe_code)]

use fe2o3_contracts::{IdentityWriteIndex, LaunchDomain1d, ThreadInDomain1d};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecAddError {
    DomainLengthMismatch,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillError {
    DomainLengthMismatch,
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

/// Executes one identity-indexed fill write for a logical thread.
pub fn fill_thread(
    domain: LaunchDomain1d,
    thread: ThreadInDomain1d,
    output: &mut [u32],
    value: u32,
) -> Result<(), FillError> {
    if thread.domain() != domain || output.len() != domain.len() {
        return Err(FillError::DomainLengthMismatch);
    }

    let write =
        IdentityWriteIndex::new(thread, output.len()).ok_or(FillError::DomainLengthMismatch)?;
    output[write.index().value()] = value;
    Ok(())
}

/// CPU reference driver over the per-thread fill contract.
pub fn fill(output: &mut [u32], value: u32) -> Result<(), FillError> {
    let domain = LaunchDomain1d::new(output.len());
    for linear in 0..domain.len() {
        let thread = domain
            .thread(linear)
            .ok_or(FillError::DomainLengthMismatch)?;
        fill_thread(domain, thread, output, value)?;
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

    #[test]
    fn fill_executes_one_identity_write_per_thread() {
        let mut output = [1, 2, 3, 4];

        assert_eq!(fill(&mut output, 17), Ok(()));
        assert_eq!(output, [17; 4]);
    }

    #[test]
    fn fill_accepts_an_empty_domain() {
        assert_eq!(fill(&mut [], 17), Ok(()));
    }

    #[test]
    fn fill_rejects_a_thread_from_another_domain() {
        let domain = LaunchDomain1d::new(2);
        let other_domain = LaunchDomain1d::new(3);
        let thread = other_domain.thread(0).unwrap();
        let mut output = [0; 2];

        assert_eq!(
            fill_thread(domain, thread, &mut output, 17),
            Err(FillError::DomainLengthMismatch)
        );
        assert_eq!(output, [0; 2]);
    }
}
