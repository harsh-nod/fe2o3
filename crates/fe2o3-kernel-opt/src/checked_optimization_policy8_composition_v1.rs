//! Borrowed P7-prefix plus J/K semantics, without an authenticated outer record.
use crate::{
    CanonicalPolicy7SemanticErrorV1, CanonicalPolicy7SemanticInputsV1,
    CanonicalPolicy8ContinuationClaimsV1, CanonicalPolicy8SemanticErrorV1,
    CheckedCanonicalPolicy8ContinuationRelationV1, ReplayedPolicy7SemanticRelationV1,
    check_canonical_policy8_continuation_relation_v1, check_published_policy7_semantic_relation_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Semantic subjects and inert claims only. J is always `prefix.output`, never
/// a second caller-selected join. Earlier record bytes keep their old meaning.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy8SemanticInputsV1<'a> {
    pub prefix: CanonicalPolicy7SemanticInputsV1<'a>,
    pub output: &'a Owner,
    pub continuation: CanonicalPolicy8ContinuationClaimsV1<'a>,
}

#[derive(Debug)]
pub enum CanonicalPolicy8CompositionErrorV1 {
    /// Diagnostic allocation only, outside the returned semantic receipt domain.
    Policy7(Box<CanonicalPolicy7SemanticErrorV1>),
    Continuation(CanonicalPolicy8SemanticErrorV1),
    Resource(Resource),
    Panicked,
}
type Error = CanonicalPolicy8CompositionErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Policy8 semantic composition: {self:?}")
    }
}
impl std::error::Error for Error {}

/// New logical retained storage, including both nested receipts, unreserved.
/// Caller-owned graph, name and row/record backing is not counted again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy8CompositionStorageV1(usize);
impl CanonicalPolicy8CompositionStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// One P7 semantic receipt and one J/K continuation. This is not a heterogeneous
/// authenticated execution record, signed/native owner, source proof or admission.
/// The continuation's J is the same actual borrow supplied as P7's final output.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy8SemanticRelationV1;
/// fn clone(value: ReplayedPolicy8SemanticRelationV1<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy8SemanticRelationV1 as Receipt;
/// fn escape<'a>(value: Receipt<'a>) -> Receipt<'static> { value }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{CanonicalPolicy8SemanticInputsV1 as Inputs,
///     ReplayedPolicy8SemanticRelationV1 as Receipt, check_published_policy8_semantic_relation_v1 as check};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape<'a>(k: Owner, mut inputs: Inputs<'a>, budget: &mut Budget<'_>) -> Receipt<'a> {
///     inputs.output = &k;
///     check(inputs, budget).unwrap()
/// }
/// ```
pub struct ReplayedPolicy8SemanticRelationV1<'a> {
    prefix: ReplayedPolicy7SemanticRelationV1<'a>,
    continuation: CheckedCanonicalPolicy8ContinuationRelationV1<'a, 'a, 'a>,
    storage: CanonicalPolicy8CompositionStorageV1,
}
impl<'a> ReplayedPolicy8SemanticRelationV1<'a> {
    pub const fn policy7_relation(&self) -> &ReplayedPolicy7SemanticRelationV1<'a> {
        &self.prefix
    }
    pub const fn continuation(&self) -> &CheckedCanonicalPolicy8ContinuationRelationV1<'a, 'a, 'a> {
        &self.continuation
    }
    pub const fn output(&self) -> &'a Owner {
        self.continuation.output()
    }
    pub const fn storage(&self) -> CanonicalPolicy8CompositionStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn wrapper() -> Result<usize, Error> {
    size_of::<ReplayedPolicy8SemanticRelationV1<'_>>()
        .checked_sub(size_of::<ReplayedPolicy7SemanticRelationV1<'_>>())
        .and_then(|size| {
            size.checked_sub(size_of::<
                CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>,
            >())
        })
        .ok_or(Error::Resource(Resource::Arithmetic))
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}

/// Replays the supplied P7 semantics exactly once, then the fixed J/K relation.
/// J is derived solely from the same `inputs.prefix.output` borrow; the inert J
/// locator is checked against that owner, not accepted as a replacement for it.
/// No optimizer, second prefix decoder or sealed execution construction runs.
/// No new outer wire is accepted or heterogeneous execution authentication implied.
///
/// All nested work shares the caller's cumulative Budget. Its borrowed backing
/// remains caller-owned/external or separately prepaid once; aliases must not be
/// double counted. Only the new wrapper minus embedded headers and each nested
/// receipt's logical storage are reserved. Scratch follows the inherited P7 and
/// J/K contracts, not an allocator/RSS bound. Success transfers both receipts
/// once, unreserved; reserve the returned total before later controlled work.
/// Errors drop already-retained nested receipts before restoring the same-ledger
/// intact floor. Work/peak/first-denial remain cumulative. Invalid ledgers/floors
/// are refused without refunding a foreign ledger; panic payloads follow cleanup.
pub fn check_published_policy8_semantic_relation_v1<'a>(
    inputs: CanonicalPolicy8SemanticInputsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<ReplayedPolicy8SemanticRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        let wrapper = wrapper()?;
        budget.reserve_storage(wrapper)?;
        let prefix = check_published_policy7_semantic_relation_v1(inputs.prefix, budget)
            .map_err(|error| Error::Policy7(Box::new(error)))?;
        let prefix_storage = prefix.storage().retained_storage();
        budget.reserve_storage(prefix_storage)?;
        let continuation = check_canonical_policy8_continuation_relation_v1(
            inputs.prefix.output,
            inputs.output,
            inputs.continuation,
            budget,
        )
        .map_err(Error::Continuation)?;
        let continuation_storage = continuation.storage().retained_storage();
        budget.reserve_storage(continuation_storage)?;
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|size| size.checked_add(continuation_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(ReplayedPolicy8SemanticRelationV1 {
            prefix,
            continuation,
            storage: CanonicalPolicy8CompositionStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy8_composition_resource_v1_tests.rs"]
mod resource_tests;
