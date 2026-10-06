//! Checked logical operation costs. These values never establish authority.
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

/// Work and peak additional logical Rust storage on the original account.
/// Persistent input owners are excluded from this additional peak, but their
/// deliberate copies inside nested operation windows are included. Filesystem
/// transaction I/O, executable pages, child RSS and LLVM limits remain separate
/// existing domains. Quotes do not enlarge any domain or grant admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerOperationQuoteV5 {
    work: usize,
    additional_storage: usize,
}
impl ConditionalWorkerOperationQuoteV5 {
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn additional_storage(self) -> usize {
        self.additional_storage
    }
    pub(crate) const fn new(work: usize, additional_storage: usize) -> Self {
        Self {
            work,
            additional_storage,
        }
    }
    /// Nested scopes coexist; both scratch allowances must remain affordable.
    pub(crate) fn nested(self, next: Self) -> Result<Self, Resource> {
        Ok(Self::new(
            self.work
                .checked_add(next.work)
                .ok_or(Resource::Arithmetic)?,
            self.additional_storage
                .checked_add(next.additional_storage)
                .ok_or(Resource::Arithmetic)?,
        ))
    }
    /// Pure completed scopes restore scratch but never work. Terminal operations
    /// with retained output must add that owner separately, not use this helper.
    pub(crate) fn sequential(self, next: Self) -> Result<Self, Resource> {
        Ok(Self::new(
            self.work
                .checked_add(next.work)
                .ok_or(Resource::Arithmetic)?,
            self.additional_storage.max(next.additional_storage),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::ConditionalWorkerOperationQuoteV5 as Q;
    #[test]
    fn nested_and_sequential_costs_preserve_overlap_and_checked_arithmetic() {
        let a = Q::new(7, 11);
        let b = Q::new(13, 17);
        assert_eq!(a.nested(b).unwrap(), Q::new(20, 28));
        assert_eq!(a.sequential(b).unwrap(), Q::new(20, 17));
        assert!(Q::new(usize::MAX, 0).nested(a).is_err());
        assert!(Q::new(0, usize::MAX).nested(a).is_err());
        assert!(Q::new(usize::MAX, 0).sequential(a).is_err());
        assert_eq!(
            Q::new(0, usize::MAX)
                .sequential(a)
                .unwrap()
                .additional_storage(),
            usize::MAX
        );
    }
}
