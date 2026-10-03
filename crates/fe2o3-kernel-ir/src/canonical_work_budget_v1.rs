use std::{error::Error, fmt};

/// Exact failure from a bounded canonical Kernel IR work meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrWorkLimitV1 {
    actual: usize,
    limit: usize,
}

impl CanonicalKernelIrWorkLimitV1 {
    pub(crate) const fn new(actual: usize, limit: usize) -> Self {
        Self { actual, limit }
    }

    /// Returns attempted cumulative work, or `usize::MAX` on arithmetic overflow.
    pub const fn actual(self) -> usize {
        self.actual
    }

    /// Returns the admitted cumulative-work limit.
    pub const fn limit(self) -> usize {
        self.limit
    }
}

impl fmt::Display for CanonicalKernelIrWorkLimitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "canonical Kernel IR work {} exceeds limit {}",
            self.actual, self.limit
        )
    }
}

impl Error for CanonicalKernelIrWorkLimitV1 {}

/// Bounded accepted-work meter shared by one canonical Kernel IR operation.
///
/// Rejected charges do not advance [`Self::work`]. The attempted cumulative
/// value remains available so a caller can debit accepted work to its parent
/// ledger. Arithmetic overflow is reported with `usize::MAX`, not an exact sum.
/// A rejected charge does not prevent a later smaller charge from succeeding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrWorkBudgetV1 {
    work: usize,
    failed_work: Option<usize>,
    limit: usize,
}

impl CanonicalKernelIrWorkBudgetV1 {
    /// Creates an empty work meter with the supplied cumulative limit.
    pub const fn new(limit: usize) -> Self {
        Self {
            work: 0,
            failed_work: None,
            limit,
        }
    }

    /// Charges work before the corresponding canonical operation executes.
    pub fn charge_work(&mut self, amount: usize) -> Result<(), CanonicalKernelIrWorkLimitV1> {
        let Some(work) = self.work.checked_add(amount) else {
            self.failed_work.get_or_insert(usize::MAX);
            return Err(CanonicalKernelIrWorkLimitV1::new(usize::MAX, self.limit));
        };
        if work > self.limit {
            self.failed_work.get_or_insert(work);
            return Err(CanonicalKernelIrWorkLimitV1::new(work, self.limit));
        }
        self.work = work;
        Ok(())
    }

    /// Returns accepted cumulative work.
    pub const fn work(&self) -> usize {
        self.work
    }

    /// Returns the first rejected cumulative work, when a charge failed.
    pub const fn failed_work(&self) -> Option<usize> {
        self.failed_work
    }

    /// Returns the cumulative-work limit.
    pub const fn limit(&self) -> usize {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_work_preserves_the_accepted_prefix() {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(5);
        budget.charge_work(2).unwrap();
        assert_eq!(
            budget.charge_work(4),
            Err(CanonicalKernelIrWorkLimitV1::new(6, 5))
        );
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.failed_work(), Some(6));
        assert_eq!(
            budget.charge_work(usize::MAX),
            Err(CanonicalKernelIrWorkLimitV1::new(usize::MAX, 5))
        );
        assert_eq!(budget.failed_work(), Some(6));
        budget.charge_work(3).unwrap();
        budget.charge_work(0).unwrap();
        assert_eq!(budget.work(), 5);
        assert_eq!(budget.failed_work(), Some(6));
    }

    #[test]
    fn overflow_preserves_the_accepted_prefix() {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        budget.charge_work(1).unwrap();
        assert_eq!(
            budget.charge_work(usize::MAX),
            Err(CanonicalKernelIrWorkLimitV1::new(usize::MAX, usize::MAX))
        );
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.failed_work(), Some(usize::MAX));
        budget.charge_work(usize::MAX - 1).unwrap();
        assert_eq!(budget.work(), usize::MAX);
        budget.charge_work(0).unwrap();
        assert_eq!(budget.failed_work(), Some(usize::MAX));
    }
}
