//! Borrowed fixed Policy8 J/K semantics, not execution or publication authority.
//! Earlier policy records and F2NTR1's historical checker meaning are unchanged.
use fe2o3_kernel_analysis::{
    CanonicalKirCommutativeBitwiseCseErrorV1, CanonicalKirInventoryErrorV1,
    CanonicalKirInventoryV1 as Inventory, check_canonical_kir_commutative_bitwise_cse_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirTransitionCandidateV1 as Candidate, InertCanonicalKirTransitionGraphIdentityV1,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Existing closed raw-pass name, checked as a claim, never a pass selector.
/// This is not a wire tag or proof that the named producer actually executed.
pub const POLICY8_COMMUTATIVE_PASS_NAME_V1: &str = "gpu-commutative-bitwise-dominance-cse-v1";

/// Caller-owned inert endpoint locators and complete typed occurrence rows.
/// No graph admission, execution witness, new record or wire format is implied.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalPolicy8ContinuationClaimsV1<'rows> {
    pub pass_name: &'rows str,
    pub input: InertCanonicalKirTransitionGraphIdentityV1,
    pub output: InertCanonicalKirTransitionGraphIdentityV1,
    pub occurrences: Candidate<'rows>,
}

#[derive(Debug)]
pub enum CanonicalPolicy8SemanticErrorV1 {
    PassIdentity,
    InputIdentity,
    OutputIdentity,
    Inventory(CanonicalKirInventoryErrorV1),
    Continuation(CanonicalKirCommutativeBitwiseCseErrorV1),
    Resource(Resource),
    Panicked,
}
type Error = CanonicalPolicy8SemanticErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy8 semantic continuation: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Logical returned header storage only, unreserved. All graph, name and row
/// backing is borrowed; inventory and checker scratch is not retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy8SemanticStorageV1(usize);
impl CanonicalPolicy8SemanticStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Independent J/K relation tied to the supplied immutable owners and rows.
/// Equal-byte independently admitted owners are valid semantic subjects, not
/// authenticated producer custody. No P7-prefix/source/final-proof replay occurs.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalPolicy8ContinuationRelationV1;
/// fn clone(value: CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::{CanonicalPolicy8ContinuationClaimsV1 as Claims,
///     CheckedCanonicalPolicy8ContinuationRelationV1 as Receipt,
///     check_canonical_policy8_continuation_relation_v1 as check};
/// fn escape<'a>(j: Owner, k: &'a Owner, claims: Claims<'a>, b: &mut Budget<'_>)
///     -> Receipt<'a, 'a, 'a> { check(&j, k, claims, b).unwrap() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalPolicy8ContinuationRelationV1 as Receipt;
/// fn escape_rows<'i, 'o, 'r>(value: Receipt<'i, 'o, 'r>) -> Receipt<'i, 'o, 'static> {
///     value
/// }
/// ```
pub struct CheckedCanonicalPolicy8ContinuationRelationV1<'input, 'output, 'rows> {
    input: &'input Owner,
    output: &'output Owner,
    claims: CanonicalPolicy8ContinuationClaimsV1<'rows>,
    proved_pairs: usize,
    storage: CanonicalPolicy8SemanticStorageV1,
}
impl<'input, 'output, 'rows> CheckedCanonicalPolicy8ContinuationRelationV1<'input, 'output, 'rows> {
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    pub const fn output(&self) -> &'output Owner {
        self.output
    }
    pub const fn claims(&self) -> CanonicalPolicy8ContinuationClaimsV1<'rows> {
        self.claims
    }
    /// Omitted definitions independently proved equivalent to retained anchors.
    pub const fn proved_pairs(&self) -> usize {
        self.proved_pairs
    }
    /// Semantic deletion status, not a raw pass's changed/execution observation.
    pub const fn has_substitutions(&self) -> bool {
        self.proved_pairs != 0
    }
    pub const fn storage(&self) -> CanonicalPolicy8SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
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
    // Potentially hostile payload destruction follows valid original-ledger cleanup.
    drop(payloads);
    result
}

fn check_claims(
    input: &Owner,
    output: &Owner,
    claims: CanonicalPolicy8ContinuationClaimsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(3)?;
    if claims.pass_name.len() != POLICY8_COMMUTATIVE_PASS_NAME_V1.len() {
        return Err(Error::PassIdentity);
    }
    budget.charge_work(2 * POLICY8_COMMUTATIVE_PASS_NAME_V1.len() + 80)?;
    if claims.pass_name != POLICY8_COMMUTATIVE_PASS_NAME_V1 {
        return Err(Error::PassIdentity);
    }
    if !claims.input.matches_verified(input.canonical().identity()) {
        return Err(Error::InputIdentity);
    }
    if !claims
        .output
        .matches_verified(output.canonical().identity())
    {
        return Err(Error::OutputIdentity);
    }
    Ok(())
}

/// Checks one fixed commutative J/K relation without running any optimizer.
/// Complete actual owners, not endpoint hashes, feed the independent checker.
/// It rejects other rewrites, missing/extra/reordered rows, altered effects/CFG
/// and invalid producer substitutions. Empty/no-op and nonempty substitutions
/// remain distinct. This does not validate the earlier Policy7 prefix.
///
/// Caller graph/row/name backing is externally owned or separately prepaid once;
/// this API neither copies it nor infers its reservation from a numeric floor.
/// Both inventories and checker scratch share the caller's cumulative budget
/// and retain their existing logical resource contracts, not allocator/RSS bounds.
/// Scratch owners drop before every scope cleanup. With an intact ledger and
/// inherited floor, success/error restores that floor while keeping accepted
/// work/peak/denial. An accounting violation is refused, never refunded against
/// another ledger. Caught panic payloads are destroyed only after cleanup.
/// Reserve the returned header before further controlled work, and release only
/// after the receipt drops. No wire is decoded and no existing byte cap is raised.
pub fn check_canonical_policy8_continuation_relation_v1<'input, 'output, 'rows>(
    input: &'input Owner,
    output: &'output Owner,
    claims: CanonicalPolicy8ContinuationClaimsV1<'rows>,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy8ContinuationRelationV1<'input, 'output, 'rows>, Error> {
    scoped(budget, |budget| {
        let retained = size_of::<CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>>();
        budget.reserve_storage(retained)?;
        check_claims(input, output, claims, budget)?;
        let proved_pairs = {
            let (before, before_storage) =
                Inventory::derive(input, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(before_storage.retained_storage())?;
            let (after, after_storage) =
                Inventory::derive(output, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(after_storage.retained_storage())?;
            let pairs = {
                let (relation, storage) = check_canonical_kir_commutative_bitwise_cse_v1(
                    &before,
                    &after,
                    claims.occurrences,
                    budget,
                )
                .map_err(Error::Continuation)?;
                budget.reserve_storage(storage.retained_storage())?;
                relation.proved_pairs()
            };
            drop(after);
            drop(before);
            pairs
        };
        Ok(CheckedCanonicalPolicy8ContinuationRelationV1 {
            input,
            output,
            claims,
            proved_pairs,
            storage: CanonicalPolicy8SemanticStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy8_semantic_resource_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "checked_optimization_policy8_semantic_v1_tests.rs"]
mod tests;
