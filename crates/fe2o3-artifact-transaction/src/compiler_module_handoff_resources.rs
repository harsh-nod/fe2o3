//! Payload/decoder accounting shared by the versioned handoff schemas.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Logical scope/error scratch, excluding success payloads prepaid by the caller.
pub(crate) const fn fixed_scope_overhead<T>() -> usize {
    use std::mem::size_of;
    size_of::<Result<T, HandoffEngineError>>()
        + size_of::<std::thread::Result<Result<T, HandoffEngineError>>>()
        + size_of::<Result<T, Resource>>()
        - 3 * size_of::<T>()
        + size_of::<Resources<'static, 'static>>()
        + 2 * size_of::<Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>>()
        + 2 * size_of::<usize>()
        + size_of::<Result<usize, Resource>>()
        + size_of::<Result<(), Resource>>()
        + size_of::<Result<(), HandoffEngineError>>()
}

/// Shared storage scope for fixed subject codecs as well as transaction owners.
/// Cleanup preserves work, denial history and exact ledger identity.
pub(crate) fn with_budget<T>(
    budget: &mut Budget<'_>,
    f: impl FnOnce(&mut Budget<'_>) -> T,
) -> Result<T, Resource> {
    Resources::Metered(budget)
        .scoped(|r| Ok(f(r.budget()?)))
        .map_err(|error| match error {
            HandoffEngineError::Resource(error) => error,
            _ => Resource::Accounting,
        })
}

pub(super) enum Resources<'budget, 'work> {
    Legacy,
    Metered(&'budget mut Budget<'work>),
}

impl<'budget, 'work> Resources<'budget, 'work> {
    pub(super) fn budget(&mut self) -> Result<&mut Budget<'work>, HandoffEngineError> {
        match self {
            Self::Legacy => Err(Resource::Accounting.into()),
            Self::Metered(budget) => Ok(budget),
        }
    }

    fn identity(&self) -> Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)> {
        match self {
            Self::Legacy => None,
            Self::Metered(budget) => Some((
                &**budget as *const Budget<'_> as usize,
                budget.work_ledger_identity_v1(),
            )),
        }
    }

    pub(super) fn require<S: HandoffSchema>(&self) -> Result<(), HandoffEngineError> {
        if S::METERED && matches!(self, Self::Legacy) {
            return Err(HandoffEngineError::Resource(Resource::Accounting));
        }
        Ok(())
    }

    pub(super) fn work(&mut self, amount: usize) -> Result<(), HandoffEngineError> {
        if let Self::Metered(budget) = self {
            budget
                .charge_work(amount)
                .map_err(HandoffEngineError::Resource)?;
        }
        Ok(())
    }

    pub(super) fn reserve(&mut self, amount: usize) -> Result<(), HandoffEngineError> {
        if let Self::Metered(budget) = self {
            budget
                .reserve_storage(amount)
                .map_err(HandoffEngineError::Resource)?;
        }
        Ok(())
    }

    pub(super) fn storage(&self) -> usize {
        match self {
            Self::Legacy => 0,
            Self::Metered(budget) => budget.storage(),
        }
    }

    /// Temporaries must be dropped inside `f`; returned owners transfer their
    /// admitted storage to the caller, following the verifier's receipt convention.
    pub(super) fn scoped<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, HandoffEngineError>,
    ) -> Result<T, HandoffEngineError> {
        let identity = self.identity();
        let floor = self.storage();
        let result = catch_unwind(AssertUnwindSafe(|| f(self)));
        let release = if self.identity() == identity {
            self.storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
        } else {
            Err(Resource::Accounting)
        };
        let cleanup = release.and_then(|release| match self {
            Self::Legacy => Ok(()),
            Self::Metered(budget) => budget.release_storage(release),
        });
        match result {
            Ok(value) => {
                cleanup.map_err(HandoffEngineError::Resource)?;
                value
            }
            Err(payload) => resume_unwind(payload),
        }
    }

    pub(super) fn buffer(&mut self, length: usize) -> Result<Vec<u8>, HandoffEngineError> {
        self.reserve(length)?;
        let bytes = try_allocate_payload_buffer(length)?;
        self.reserve(
            bytes
                .capacity()
                .checked_sub(length)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok(bytes)
    }
}

impl From<Resource> for HandoffEngineError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn resource_scope_restores_storage_on_success_refusal_and_unwind() {
        for mode in 0..3 {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            budget.reserve_storage(13).unwrap();
            let mut resources = Resources::Metered(&mut budget);
            let result = catch_unwind(AssertUnwindSafe(|| {
                resources.scoped(|r| {
                    r.reserve(17)?;
                    r.work(11)?;
                    assert!(r.reserve(usize::MAX).is_err());
                    assert!(r.work(usize::MAX).is_err());
                    match mode {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation.into()),
                        _ => panic!("scope unwind"),
                    }
                })
            }));
            match mode {
                0 => result.unwrap().unwrap(),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(HandoffEngineError::Resource(Resource::Allocation))
                )),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 13);
            assert_eq!(budget.peak_storage(), 30);
            assert_eq!(budget.work(), 11);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }

    #[test]
    fn resource_scope_never_releases_a_substituted_budget_or_work_ledger() {
        for (mode, outcome) in (0..3).flat_map(|mode| (0..3).map(move |outcome| (mode, outcome))) {
            let mut first_work = Work::new(100);
            let mut second_work = Work::new(100);
            let mut first = Budget::new(&mut first_work, 100);
            let mut second = Budget::new(&mut second_work, 100);
            first.reserve_storage(13).unwrap();
            second.reserve_storage(29).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                let mut resources = Resources::Metered(&mut first);
                resources.scoped(|r| {
                    r.reserve(17)?;
                    match mode {
                        0 => *r = Resources::Legacy,
                        1 => *r = Resources::Metered(&mut second),
                        _ => std::mem::swap(r.budget()?, &mut second),
                    }
                    match outcome {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation.into()),
                        _ => panic!("substitution unwind"),
                    }
                })
            }));
            if outcome == 2 {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(HandoffEngineError::Resource(Resource::Accounting))
                ));
            }
            assert_eq!(
                (first.storage(), second.storage()),
                if mode == 2 { (29, 30) } else { (30, 29) }
            );
        }
    }

    #[test]
    fn resource_scope_rejects_released_inherited_floor() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(13).unwrap();
        let error = Resources::Metered(&mut budget)
            .scoped(|r| {
                r.budget()?.release_storage(1)?;
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            HandoffEngineError::Resource(Resource::Accounting)
        ));
        assert_eq!(budget.storage(), 12);
    }
}
