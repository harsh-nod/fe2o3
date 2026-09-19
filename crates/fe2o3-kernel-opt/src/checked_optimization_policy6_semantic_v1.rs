//! Policy6 semantic composition over unchanged P5 components and O/I F2NTR1.
//! Raw fixed records remain claims unless matched to a real sealed P6 owner.

use crate::{
    CanonicalPolicy5SemanticErrorV1, CanonicalPolicy5SemanticInputsV1,
    CheckedCanonicalKernelIrOwnerPolicy6V1 as CheckedOwner, CheckedCanonicalOptimizationReceiptV1,
    CheckedCanonicalPolicy5ExecutionRelationV1, KernelIrCheckedOptimizationReceiptErrorV1,
    POLICY6_EXECUTION_RECORD_BYTES_V1, ReplayedPolicy5SemanticRelationV1,
    check_canonical_policy5_execution_relation_v1, check_published_policy5_semantic_relation_v1,
    decode_and_check_canonical_optimization_receipt_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirTransitionCandidateV1 as Candidate, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1, IntegerContinuationClaimErrorV1,
    KirOptimizationMapErrorV12, UnauthenticatedIntegerContinuationClaimV1,
    read_unauthenticated_integer_continuation_claim_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub enum CanonicalPolicy6SemanticErrorV1 {
    Composition,
    ExecutionWitness,
    InputHistory,
    Occurrences,
    Claim(IntegerContinuationClaimErrorV1),
    Policy5(CanonicalPolicy5SemanticErrorV1),
    Transition(KernelIrCheckedOptimizationReceiptErrorV1),
    Map(KirOptimizationMapErrorV12),
    Resource(Resource),
    Panicked,
}
type Error = CanonicalPolicy6SemanticErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy6 semantic composition: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Borrowed existing records only, not a new frame or execution authority.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy6ContinuationClaimsV1<'a> {
    pub composition_record: &'a [u8],
    pub integer_record: &'a [u8],
    pub transition_wire: &'a [u8],
}

/// Caller-selected semantic subjects, never a producer policy selector.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy6SemanticInputsV1<'a> {
    pub prefix: CanonicalPolicy5SemanticInputsV1<'a>,
    pub output: &'a Owner,
    pub continuation: CanonicalPolicy6ContinuationClaimsV1<'a>,
}

/// Owned P5 and O/I receipt storage plus the new wrapper. Borrowed graph/record
/// backing is caller-reserved once or explicitly externally owned, not copied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy6SemanticStorageV1(usize);
impl CanonicalPolicy6SemanticStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Independently checked B/C/S/O/I semantics with unauthenticated execution
/// claims. No source/proof/target/publication authority or fixed-point claim.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy6SemanticRelationV1;
/// fn clone(value: ReplayedPolicy6SemanticRelationV1<'_>) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::{CanonicalPolicy6SemanticInputsV1 as Inputs,
///     ReplayedPolicy6SemanticRelationV1 as Receipt,
///     check_published_policy6_semantic_relation_v1 as check};
/// fn escape<'a>(inputs: Inputs<'a>, i: Owner, budget: &mut Budget<'_>) -> Receipt<'a> {
///     check(Inputs { output: &i, ..inputs }, budget).unwrap()
/// }
/// ```
pub struct ReplayedPolicy6SemanticRelationV1<'a> {
    prefix: ReplayedPolicy5SemanticRelationV1<'a>,
    continuation: CheckedCanonicalOptimizationReceiptV1<'a, 'a>,
    composition: &'a [u8; POLICY6_EXECUTION_RECORD_BYTES_V1],
    claim: UnauthenticatedIntegerContinuationClaimV1<'a>,
    storage: CanonicalPolicy6SemanticStorageV1,
}
impl<'a> ReplayedPolicy6SemanticRelationV1<'a> {
    pub const fn policy5_relation(&self) -> &ReplayedPolicy5SemanticRelationV1<'a> {
        &self.prefix
    }
    pub const fn continuation(&self) -> &CheckedCanonicalOptimizationReceiptV1<'a, 'a> {
        &self.continuation
    }
    pub const fn unauthenticated_composition_record(
        &self,
    ) -> &'a [u8; POLICY6_EXECUTION_RECORD_BYTES_V1] {
        self.composition
    }
    pub const fn integer_claim(&self) -> &UnauthenticatedIntegerContinuationClaimV1<'a> {
        &self.claim
    }
    pub const fn storage(&self) -> CanonicalPolicy6SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Actual sealed P6 execution plus independent O/I semantic replay. The observed
/// map/report remain owned once by the borrowed real P6, never decoded or cloned.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::{CanonicalPolicy6ContinuationClaimsV1 as Claims,
///     check_canonical_policy6_execution_relation_v1 as check};
/// fn no_raw_witness(b: &Owner, i: &Owner, claims: Claims<'_>, budget: &mut Budget<'_>) {
///     let _ = check(b, i, &[], &[], &[], claims, budget);
/// }
/// ```
pub struct CheckedCanonicalPolicy6ExecutionRelationV1<'a> {
    prefix: CheckedCanonicalPolicy5ExecutionRelationV1<'a>,
    continuation: CheckedCanonicalOptimizationReceiptV1<'a, 'a>,
    checked: &'a CheckedOwner,
    composition: &'a [u8; POLICY6_EXECUTION_RECORD_BYTES_V1],
    claim: UnauthenticatedIntegerContinuationClaimV1<'a>,
    storage: CanonicalPolicy6SemanticStorageV1,
}
impl<'a> CheckedCanonicalPolicy6ExecutionRelationV1<'a> {
    pub const fn policy5_execution(&self) -> &CheckedCanonicalPolicy5ExecutionRelationV1<'a> {
        &self.prefix
    }
    pub const fn continuation(&self) -> &CheckedCanonicalOptimizationReceiptV1<'a, 'a> {
        &self.continuation
    }
    pub const fn execution_owner(&self) -> &'a CheckedOwner {
        self.checked
    }
    pub const fn authenticated_composition_record(
        &self,
    ) -> &'a [u8; POLICY6_EXECUTION_RECORD_BYTES_V1] {
        self.composition
    }
    pub const fn authenticated_integer_record(
        &self,
    ) -> &[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1] {
        self.claim.canonical_bytes()
    }
    pub const fn storage(&self) -> CanonicalPolicy6SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        true
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
    let mut panic_payload = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            panic_payload = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if budget.work_ledger_identity_v1() != ledger {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    match cleanup {
        Ok(()) => {
            drop(panic_payload);
            result
        }
        Err(error) => {
            let drop_payload = catch_unwind(AssertUnwindSafe(|| drop(result))).err();
            drop(panic_payload);
            drop(drop_payload);
            Err(error.into())
        }
    }
}

fn composition<'a>(
    inputs: CanonicalPolicy6SemanticInputsV1<'a>,
    claim: &UnauthenticatedIntegerContinuationClaimV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<&'a [u8; POLICY6_EXECUTION_RECORD_BYTES_V1], Error> {
    budget.charge_work(2)?;
    let bytes: &[u8; POLICY6_EXECUTION_RECORD_BYTES_V1] = inputs
        .continuation
        .composition_record
        .try_into()
        .map_err(|_| Error::Composition)?;
    budget.charge_work(POLICY6_EXECUTION_RECORD_BYTES_V1)?;
    if &bytes[..8] != b"F2P6EX1\0"
        || bytes[8..16] != [1, 0, 6, 0, 5, 0, 2, 0]
        || bytes[248..256] != 2u64.to_le_bytes()
    {
        return Err(Error::Composition);
    }
    for (offset, owner) in [
        (16, inputs.prefix.input),
        (56, inputs.prefix.intermediate),
        (96, inputs.prefix.stored),
        (136, inputs.prefix.output),
        (176, inputs.output),
    ] {
        let identity = owner.canonical().identity();
        if &bytes[offset..offset + 32] != identity.digest()
            || bytes[offset + 32..offset + 40] != identity.canonical_length().to_le_bytes()
        {
            return Err(Error::Composition);
        }
    }
    if &bytes[216..248] != claim.declared_map_digest() {
        return Err(Error::Composition);
    }
    Ok(bytes)
}

fn same_rows<T: PartialEq>(a: &[T], b: &[T], budget: &mut Budget<'_>) -> Result<bool, Error> {
    budget.charge_work(1)?;
    let work = a
        .len()
        .checked_mul(size_of::<T>())
        .and_then(|n| {
            b.len()
                .checked_mul(size_of::<T>())
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    Ok(a == b)
}

fn exact_occurrences(
    a: Candidate<'_>,
    b: Candidate<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    macro_rules! exact {
        ($($field:ident),+ $(,)?) => { $(
            if !same_rows(a.$field, b.$field, budget)? { return Err(Error::Occurrences); }
        )+ };
    }
    exact!(
        functions,
        blocks,
        segments,
        operations,
        definitions,
        definition_outputs,
        uses,
        edges,
        edge_arguments
    );
    Ok(())
}

fn wrapper<T, P>(budget: &mut Budget<'_>) -> Result<usize, Error> {
    budget.charge_work(3)?;
    let bytes = size_of::<T>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    Ok(bytes)
}

/// Reuse exactly one P5 semantic result and one unchanged O/I F2NTR decoder.
/// Execution claims do not authenticate the map/report or the claimed roster.
/// Existing child caps apply independently; this is not a serialized outer frame.
/// Incoming backing stays reserved/external; success returns unreserved retained
/// metadata, preserving the same Work ledger and restoring the incoming floor.
pub fn check_published_policy6_semantic_relation_v1<'a>(
    inputs: CanonicalPolicy6SemanticInputsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<ReplayedPolicy6SemanticRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        let wrapper = wrapper::<
            ReplayedPolicy6SemanticRelationV1<'_>,
            ReplayedPolicy5SemanticRelationV1<'_>,
        >(budget)?;
        let claim = read_unauthenticated_integer_continuation_claim_v1(
            inputs.prefix.output,
            inputs.output,
            inputs.continuation.integer_record,
            budget,
        )
        .map_err(Error::Claim)?;
        let composition = composition(inputs, &claim, budget)?;
        let prefix = check_published_policy5_semantic_relation_v1(inputs.prefix, budget)
            .map_err(Error::Policy5)?;
        let prefix_storage = prefix.storage().retained_storage();
        budget.reserve_storage(prefix_storage)?;
        let continuation = decode_and_check_canonical_optimization_receipt_v1(
            inputs.prefix.output,
            inputs.output,
            inputs.continuation.transition_wire,
            budget,
        )
        .map_err(Error::Transition)?;
        let continuation_storage = continuation.storage().retained_storage();
        budget.reserve_storage(continuation_storage)?;
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|n| n.checked_add(continuation_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(ReplayedPolicy6SemanticRelationV1 {
            prefix,
            continuation,
            composition,
            claim,
            storage: CanonicalPolicy6SemanticStorageV1(retained),
        })
    })
}

/// Require real sealed P6 custody, authenticate unchanged P5 once, and replay
/// actual O/I once. Exact nine-axis row equality links that independent replay to
/// the actual retained execution. Map/report checking uses existing typed APIs,
/// never decoded maps, cloned graphs or optimizer execution. No proof authority.
pub fn check_canonical_policy6_execution_relation_v1<'a>(
    input: &'a Owner,
    checked: &'a CheckedOwner,
    policy4_wire: &'a [u8],
    policy5_record: &'a [u8],
    load_rows: &'a [CanonicalKirLoadForwardingRowV1],
    claims: CanonicalPolicy6ContinuationClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy6ExecutionRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        let wrapper = wrapper::<
            CheckedCanonicalPolicy6ExecutionRelationV1<'_>,
            CheckedCanonicalPolicy5ExecutionRelationV1<'_>,
        >(budget)?;
        let p5 = checked.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        let inputs = CanonicalPolicy6SemanticInputsV1 {
            prefix: CanonicalPolicy5SemanticInputsV1 {
                input,
                intermediate: p4.intermediate_policy3().owner(),
                stored: p4.owner(),
                output: p5.owner(),
                policy4_wire,
                policy5_record,
                load_rows,
            },
            output: checked.owner(),
            continuation: claims,
        };
        let claim = read_unauthenticated_integer_continuation_claim_v1(
            p5.owner(),
            checked.owner(),
            claims.integer_record,
            budget,
        )
        .map_err(Error::Claim)?;
        let composition = composition(inputs, &claim, budget)?;
        let prefix = check_canonical_policy5_execution_relation_v1(
            input,
            p5,
            policy4_wire,
            policy5_record,
            load_rows,
            budget,
        )
        .map_err(Error::Policy5)?;
        let prefix_storage = prefix.storage().retained_storage();
        budget.reserve_storage(prefix_storage)?;
        let continuation = decode_and_check_canonical_optimization_receipt_v1(
            p5.owner(),
            checked.owner(),
            claims.transition_wire,
            budget,
        )
        .map_err(Error::Transition)?;
        let continuation_storage = continuation.storage().retained_storage();
        budget.reserve_storage(continuation_storage)?;
        let actual = checked.continuation();
        budget.charge_work(
            POLICY6_EXECUTION_RECORD_BYTES_V1 + INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1,
        )?;
        if composition != checked.execution().canonical_bytes()
            || claim.canonical_bytes() != actual.execution().canonical_bytes()
        {
            return Err(Error::ExecutionWitness);
        }
        budget.charge_work(2)?;
        let bytes = p5.owner().canonical().canonical_bytes();
        let historical = actual.native_input_audit_bytes();
        budget.charge_work(
            bytes
                .len()
                .checked_add(historical.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if bytes != historical {
            return Err(Error::InputHistory);
        }
        exact_occurrences(
            continuation.receipt().candidate(),
            actual.occurrences().candidate(),
            budget,
        )?;
        check_integer_execution(checked, budget)?;
        actual
            .map()
            .check_against(p5.owner(), checked.owner(), budget)
            .map_err(Error::Map)?;
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|n| n.checked_add(continuation_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalPolicy6ExecutionRelationV1 {
            prefix,
            continuation,
            checked,
            composition,
            claim,
            storage: CanonicalPolicy6SemanticStorageV1(retained),
        })
    })
}

fn check_integer_execution(checked: &CheckedOwner, budget: &mut Budget<'_>) -> Result<(), Error> {
    // The retained witness and its additional expected record coexist during replay.
    budget.reserve_storage(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)?;
    let actual = checked.continuation();
    let result = actual.execution().check_against(
        checked.intermediate_policy5().owner(),
        checked.owner(),
        actual.report(),
        actual.map(),
        budget,
    );
    // The callee's fixed expected record is gone before this reservation is released.
    budget.release_storage(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)?;
    result.map_err(Error::Resource)
}

#[cfg(test)]
#[path = "checked_optimization_policy6_semantic_resource_v1_tests.rs"]
mod resource_tests;
