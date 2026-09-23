use std::{error::Error, fmt};

use crate::{CanonicalKernelIrWorkBudgetV1, CanonicalKernelIrWorkLimitV1};

/// Observed resources for one completed exact-decoded semantic verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrVerificationResourceReceiptV1 {
    work: usize,
    peak_storage: usize,
}

impl CanonicalKernelIrVerificationResourceReceiptV1 {
    pub(crate) const fn new(work: usize, peak_storage: usize) -> Self {
        Self { work, peak_storage }
    }

    /// Returns work accepted specifically by semantic verification.
    pub const fn work(self) -> usize {
        self.work
    }

    /// Returns the verifier-local peak, excluding caller-owned module bytes.
    pub const fn peak_storage(self) -> usize {
        self.peak_storage
    }
}

/// Exact failure from a bounded canonical Kernel IR verification-storage meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrVerificationStorageLimitV1 {
    actual: usize,
    limit: usize,
}

impl CanonicalKernelIrVerificationStorageLimitV1 {
    const fn new(actual: usize, limit: usize) -> Self {
        Self { actual, limit }
    }

    /// Returns attempted live storage, or `usize::MAX` on arithmetic overflow.
    pub const fn actual(self) -> usize {
        self.actual
    }

    /// Returns the admitted live verification-storage limit.
    pub const fn limit(self) -> usize {
        self.limit
    }
}

impl fmt::Display for CanonicalKernelIrVerificationStorageLimitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "canonical Kernel IR verification storage {} exceeds limit {}",
            self.actual, self.limit
        )
    }
}

impl Error for CanonicalKernelIrVerificationStorageLimitV1 {}

/// Resource or allocation failure from metered canonical Kernel IR verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKernelIrVerificationResourceErrorV1 {
    /// The shared canonical work meter rejected the next operation.
    Work(CanonicalKernelIrWorkLimitV1),
    /// The verifier-local live-storage meter rejected the next allocation.
    Storage(CanonicalKernelIrVerificationStorageLimitV1),
    /// A logically admitted allocation failed in the host allocator.
    Allocation,
    /// An internal verifier-storage owner attempted an invalid release.
    Accounting,
    /// A verifier-local resource formula overflowed before its operation.
    Arithmetic,
}

impl fmt::Display for CanonicalKernelIrVerificationResourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Work(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
            Self::Allocation => {
                formatter.write_str("canonical Kernel IR verification allocation failed")
            }
            Self::Accounting => {
                formatter.write_str("canonical Kernel IR verification storage accounting failed")
            }
            Self::Arithmetic => {
                formatter.write_str("canonical Kernel IR verification resource formula overflowed")
            }
        }
    }
}

impl Error for CanonicalKernelIrVerificationResourceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Work(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::Allocation | Self::Accounting | Self::Arithmetic => None,
        }
    }
}

/// Shared work and verifier-local live-storage ledger for one verification.
///
/// Every work charge and storage reservation is admitted before the associated
/// traversal, comparison, allocation, or mutation. Rejected charges preserve
/// the accepted prefix. Callers retain the peak after rolling current scratch
/// storage back to an earlier checkpoint. Units and owner lifetimes are part of
/// each caller's accounting contract; this ledger is not an allocator/RSS meter.
pub struct CanonicalKernelIrVerificationResourceBudgetV1<'work> {
    work: &'work mut CanonicalKernelIrWorkBudgetV1,
    storage: usize,
    peak_storage: usize,
    failed_storage: Option<usize>,
    storage_limit: usize,
}

/// Equality-only identity of a borrowed live Work meter, not its Budget slot.
///
/// This inert token is meaningful only while the originating Work borrow stays
/// live. Addresses can be reused after that borrow ends. It is not a receipt,
/// persistent identity, resource reservation, or compiler authority.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CanonicalKernelIrWorkLedgerIdentityV1(usize);

impl<'work> CanonicalKernelIrVerificationResourceBudgetV1<'work> {
    /// Creates an empty verification-storage ledger over a shared work meter.
    pub fn new(work: &'work mut CanonicalKernelIrWorkBudgetV1, storage_limit: usize) -> Self {
        Self {
            work,
            storage: 0,
            peak_storage: 0,
            failed_storage: None,
            storage_limit,
        }
    }

    /// Admits work before a caller's bounded traversal. This is inert accounting,
    /// not semantic verification or compiler authority.
    pub fn charge_work(
        &mut self,
        amount: usize,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.work
            .charge_work(amount)
            .map_err(CanonicalKernelIrVerificationResourceErrorV1::Work)
    }

    /// Admits additional coexisting logical payload before allocation. Callers
    /// preserve the existing unit convention and retain every live owner floor.
    pub fn reserve_storage(
        &mut self,
        amount: usize,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(storage) = self.storage.checked_add(amount) else {
            self.failed_storage.get_or_insert(usize::MAX);
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(
                CanonicalKernelIrVerificationStorageLimitV1::new(usize::MAX, self.storage_limit),
            ));
        };
        if storage > self.storage_limit {
            self.failed_storage.get_or_insert(storage);
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(
                CanonicalKernelIrVerificationStorageLimitV1::new(storage, self.storage_limit),
            ));
        }
        self.storage = storage;
        self.peak_storage = self.peak_storage.max(storage);
        Ok(())
    }

    /// Releases a previously admitted payload after its owner has been dropped
    /// or explicitly transferred. Underflow fails without changing the ledger.
    pub fn release_storage(
        &mut self,
        amount: usize,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(storage) = self.storage.checked_sub(amount) else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        self.storage = storage;
        Ok(())
    }

    pub(crate) const fn storage_checkpoint(&self) -> usize {
        self.storage
    }

    /// Runs a bounded operation on this same ledger, with prepaid outer work
    /// and scratch. Nested operations must charge this ledger, never a fresh
    /// budget. `entry_work` is charged before checking the input-owner floor;
    /// the rest of `work` is charged before reserving `scratch` or calling out.
    ///
    /// Returns entry storage on success, error, and unwind, retaining work,
    /// peak, and denial history. The callback must preserve the reserved frame
    /// and ledger identity. A replaced ledger is never released. Returned
    /// owners are unreserved: callers must reserve their retained charge before
    /// keeping them. This is logical accounting, not admission or authority.
    pub fn with_prepaid_scope<T, E: From<CanonicalKernelIrVerificationResourceErrorV1>>(
        &mut self,
        input_floor: usize,
        entry_work: usize,
        work: usize,
        scratch: usize,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        use CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

        self.charge_work(entry_work)?;
        if self.storage() < input_floor {
            return Err(Resource::Accounting.into());
        }
        self.charge_work(work.checked_sub(entry_work).ok_or(Resource::Arithmetic)?)?;
        let floor = self.storage();
        let ledger = self.work_ledger_identity_v1();
        self.reserve_storage(scratch)?;
        let protected_floor = self.storage();
        let result = catch_unwind(AssertUnwindSafe(|| operation(self)));
        let frame_intact = self.storage() >= protected_floor;
        let cleanup = if self.work_ledger_identity_v1() == ledger {
            self.storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
                .and_then(|release| self.release_storage(release))
        } else {
            Err(Resource::Accounting)
        };
        match result {
            Ok(result) => {
                cleanup?;
                if !frame_intact {
                    return Err(Resource::Accounting.into());
                }
                result
            }
            Err(panic) => resume_unwind(panic),
        }
    }

    pub(crate) fn rollback_storage(
        &mut self,
        checkpoint: usize,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(release) = self.storage.checked_sub(checkpoint) else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        self.release_storage(release)
    }

    pub(crate) fn work_budget_v1(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1 {
        self.work
    }

    pub(crate) fn work_limit_v1(&self) -> usize {
        self.work.limit()
    }

    /// Identifies the borrowed Work meter without exposing mutable access.
    /// A moved Budget keeps this token; replacing its Work borrow changes it.
    pub fn work_ledger_identity_v1(&self) -> CanonicalKernelIrWorkLedgerIdentityV1 {
        CanonicalKernelIrWorkLedgerIdentityV1(
            &*self.work as *const CanonicalKernelIrWorkBudgetV1 as usize,
        )
    }

    /// Returns accepted shared canonical work.
    pub fn work(&self) -> usize {
        self.work.work()
    }

    /// Returns accepted live verifier-local storage.
    pub const fn storage(&self) -> usize {
        self.storage
    }

    /// Returns peak verifier-local storage, including rolled-back scratch.
    pub const fn peak_storage(&self) -> usize {
        self.peak_storage
    }

    /// Returns the first rejected live-storage total, when present.
    pub const fn failed_storage(&self) -> Option<usize> {
        self.failed_storage
    }

    /// Returns the verifier-local live-storage limit.
    pub const fn storage_limit(&self) -> usize {
        self.storage_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepaid_scope_honors_variable_entry_cost_before_callback_or_scratch() {
        type Resource = CanonicalKernelIrVerificationResourceErrorV1;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100);
        budget.reserve_storage(9).unwrap();
        let result =
            budget.with_prepaid_scope::<(), Resource>(10, 3, 7, 20, |_| panic!("unpaid input"));
        assert!(matches!(result, Err(Resource::Accounting)));
        assert_eq!(budget.work(), 3);
        let result =
            budget.with_prepaid_scope::<(), Resource>(9, 4, 3, 20, |_| panic!("invalid quota"));
        assert!(matches!(result, Err(Resource::Arithmetic)));
        assert_eq!(budget.work(), 7);
        assert_eq!(budget.peak_storage(), 9);
        budget
            .with_prepaid_scope::<(), Resource>(9, 0, 7, 20, |budget| {
                assert_eq!(budget.storage(), 29);
                budget.charge_work(5)
            })
            .unwrap();
        assert_eq!(budget.storage(), 9);
        assert_eq!(budget.work(), 19);
        assert_eq!(budget.peak_storage(), 29);
    }

    #[test]
    fn work_identity_follows_the_meter_not_the_resource_budget_slot() {
        let mut first_work = CanonicalKernelIrWorkBudgetV1::new(17);
        let mut second_work = CanonicalKernelIrWorkBudgetV1::new(17);
        let mut first = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut first_work, 9);
        let mut second = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut second_work, 9);
        let first_identity = first.work_ledger_identity_v1();
        let second_identity = second.work_ledger_identity_v1();
        assert!(first_identity != second_identity);
        assert_eq!(
            std::mem::size_of_val(&first_identity),
            std::mem::size_of::<usize>()
        );
        first.charge_work(3).unwrap();
        first.reserve_storage(4).unwrap();
        second.charge_work(3).unwrap();
        second.reserve_storage(4).unwrap();
        let mut moved = first;
        assert!(moved.work_ledger_identity_v1() == first_identity);
        let slot = &moved as *const _;
        std::mem::swap(&mut moved, &mut second);
        assert_eq!(&moved as *const _, slot);
        assert!(moved.work_ledger_identity_v1() == second_identity);
        assert!(second.work_ledger_identity_v1() == first_identity);
        assert_eq!((moved.work(), moved.storage()), (3, 4));
        assert_eq!((second.work(), second.storage()), (3, 4));
    }

    #[test]
    fn rejected_work_and_storage_preserve_accepted_prefixes() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 7);
        budget.charge_work(2).unwrap();
        budget.reserve_storage(3).unwrap();
        assert!(matches!(
            budget.charge_work(4),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == 6 && error.limit() == 5
        ));
        assert!(matches!(
            budget.reserve_storage(5),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                if error.actual() == 8 && error.limit() == 7
        ));
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.storage(), 3);
        assert_eq!(budget.peak_storage(), 3);
        assert_eq!(budget.failed_storage(), Some(8));
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert_eq!(budget.failed_storage(), Some(8));
        budget.reserve_storage(4).unwrap();
        budget.charge_work(3).unwrap();
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.peak_storage(), 7);
        assert_eq!(budget.work(), 5);
        assert_eq!(budget.failed_storage(), Some(8));
    }

    #[test]
    fn rollback_preserves_peak_and_closes_current_storage() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 9);
        budget.reserve_storage(2).unwrap();
        let floor = budget.storage_checkpoint();
        budget.reserve_storage(7).unwrap();
        budget.rollback_storage(floor).unwrap();
        assert_eq!(budget.storage(), 2);
        assert_eq!(budget.peak_storage(), 9);
    }

    #[test]
    fn invalid_release_and_forward_checkpoint_leave_the_ledger_unchanged() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(9);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 7);
        budget.charge_work(3).unwrap();
        budget.reserve_storage(4).unwrap();
        for result in [budget.release_storage(5), budget.rollback_storage(5)] {
            assert_eq!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
            );
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (3, 4, 4)
        );
        assert_eq!(budget.failed_storage(), None);
        budget.rollback_storage(4).unwrap();
        budget.rollback_storage(0).unwrap();
        assert_eq!((budget.storage(), budget.peak_storage()), (0, 4));
    }

    #[test]
    fn overflow_does_not_mutate_current_storage() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(1).unwrap();
        assert!(matches!(
            budget.reserve_storage(usize::MAX),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                if error.actual() == usize::MAX && error.limit() == usize::MAX
        ));
        assert_eq!(budget.storage(), 1);
        assert_eq!(budget.peak_storage(), 1);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
}
