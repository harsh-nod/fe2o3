//! Original-account wrapper over the unchanged closed-fill runtime admission.
use super::*;
use crate::retained_functional_refinement_runtime_v1::{
    RetainedFunctionalRefinementRuntimeErrorKindV1 as Kind,
    RetainedFunctionalRefinementRuntimeResourceErrorV1 as Inner,
    open_retained_closed_conditional_fill_runtime_bounded_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionalRefinementRuntimeResourceErrorV1 {
    Resource(Resource),
    Runtime(Kind),
}
type Error = FunctionalRefinementRuntimeResourceErrorV1;
impl From<Inner> for Error {
    fn from(value: Inner) -> Self {
        match value {
            Inner::Resource(e) => Self::Resource(e),
            Inner::Runtime(e) => Self::Runtime(e),
        }
    }
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bounded closed-fill runtime refused: {self:?}")
    }
}
impl std::error::Error for Error {}
/// Full unreserved original-owner charge; this is not proof or runtime authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionalRefinementRuntimeStorageV1(usize);
impl FunctionalRefinementRuntimeStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl FunctionalRefinementVerusRuntimeLeaseV1 {
    /// Opens the exact existing closed-fill toolchain under the original active
    /// account. The budget and its work borrow must stay at their admitting
    /// locations for this lease's lifetime, as required by the retained backend.
    /// Reserve the returned full charge before use; never replace/replenish the
    /// account. These conservative logical bounds are not aggregate child RSS.
    pub fn open_closed_conditional_fill_in_original_account_v1(
        root: impl AsRef<Path>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, FunctionalRefinementRuntimeStorageV1), Error> {
        let root = root.as_ref();
        // Wrapper headers are prepaid before the backend performs any I/O.
        let header = size_of::<Self>() + size_of::<FunctionalRefinementRuntimeStorageV1>();
        budget.reserve_storage(header)?;
        let (backend, storage) =
            open_retained_closed_conditional_fill_runtime_bounded_v1(root, budget)?;
        let charge = storage
            .retained_storage()
            .checked_add(header)
            .ok_or(Resource::Arithmetic)?;
        let value = Self {
            identity: FunctionalRefinementVerusRuntimeIdentityV1(backend.identity()),
            backend,
            broker: None,
        };
        budget.release_storage(header)?;
        Ok((value, FunctionalRefinementRuntimeStorageV1(charge)))
    }
    /// Revalidates the original bounded lease/account, without promoting legacy
    /// unmetered leases or changing process policy. Failure does not grant fallback.
    pub fn revalidate_closed_conditional_fill_in_original_account_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        budget.charge_work(8)?;
        if self.broker.is_some()
            || self.process_policy() != GeneratedProofProcessPolicyV2::ClosedConditionalFillV1
        {
            return Err(Resource::Accounting.into());
        }
        let floor = self
            .backend
            .required_retained_storage_v1()?
            .checked_add(size_of::<Self>())
            .and_then(|n| n.checked_add(size_of::<FunctionalRefinementRuntimeStorageV1>()))
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        self.backend
            .revalidate_bounded_v1(budget)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    #[test]
    fn bounded_closed_fill_denials_precede_path_observation() {
        let mut work = Work::new(0);
        let mut b = Budget::new(&mut work, 1_000_000);
        let result = FunctionalRefinementVerusRuntimeLeaseV1::open_closed_conditional_fill_in_original_account_v1(
            "/path-that-must-not-be-opened", &mut b);
        assert!(matches!(result, Err(Error::Resource(_))));
        assert!(b.failed_work().is_some());
        let mut work = Work::new(1_000_000);
        let mut b = Budget::new(&mut work, 0);
        let result = FunctionalRefinementVerusRuntimeLeaseV1::open_closed_conditional_fill_in_original_account_v1(
            "/path-that-must-not-be-opened", &mut b);
        assert!(matches!(result, Err(Error::Resource(_))));
        assert!(b.failed_storage().is_some());
    }
}
