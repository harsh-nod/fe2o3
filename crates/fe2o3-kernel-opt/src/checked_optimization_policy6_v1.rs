//! Consuming Policy6: unchanged actual Policy5 B/C/S/O, then one fixed
//! integer-identity sweep and DCE to I. No native O artifacts are constructed.

use crate::{
    CanonicalPolicy5OptimizationErrorV1, CheckedCanonicalKernelIrOwnerPolicy5V1 as Prefix,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, CanonicalKirTransitionErrorV1,
    check_canonical_kir_transition_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerIntegerContinuationV1 as Continuation,
    KirCheckedNeutralOptimizationErrorV1, KirNeutralOptimizationErrorV1,
    KirOptimizationMapErrorV12, optimize_native_neutral_kernel_ir_integer_continuation_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Header, five exact subjects, continuation map digest and fixed pass count.
pub const POLICY6_EXECUTION_RECORD_BYTES_V1: usize = 256;

#[derive(Debug)]
pub enum CanonicalPolicy6OptimizationErrorV1 {
    Resource(Resource),
    Policy5(CanonicalPolicy5OptimizationErrorV1),
    Observation(KirNeutralOptimizationErrorV1),
    Check(KirCheckedNeutralOptimizationErrorV1),
    Relation(CanonicalKirTransitionErrorV1),
    Inventory(CanonicalKirInventoryErrorV1),
    Map(KirOptimizationMapErrorV12),
    Execution,
    Panicked,
}
type Error = CanonicalPolicy6OptimizationErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "checked Policy6 composition: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Sealed composition execution, not a decoder-created proof or compiler origin.
pub struct Policy6ExecutionWitnessV1 {
    bytes: [u8; POLICY6_EXECUTION_RECORD_BYTES_V1],
}
impl Policy6ExecutionWitnessV1 {
    pub const fn policy_version(&self) -> u16 {
        6
    }
    pub const fn canonical_bytes(&self) -> &[u8; POLICY6_EXECUTION_RECORD_BYTES_V1] {
        &self.bytes
    }
}

/// Owns the original Policy5 prefix once and the separately checked actual I.
/// No Clone, from-parts, old-policy conversion, or authority-granting API.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1;
/// fn copy(owner: CheckedCanonicalKernelIrOwnerPolicy6V1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::{CheckedCanonicalKernelIrOwnerPolicy5V1, CheckedCanonicalKernelIrOwnerPolicy6V1};
/// fn old(owner: CheckedCanonicalKernelIrOwnerPolicy6V1) -> CheckedCanonicalKernelIrOwnerPolicy5V1 { owner }
/// ```
pub struct CheckedCanonicalKernelIrOwnerPolicy6V1 {
    prefix: Prefix,
    continuation: Continuation,
    execution: Policy6ExecutionWitnessV1,
    retained: usize,
}
impl CheckedCanonicalKernelIrOwnerPolicy6V1 {
    pub const fn intermediate_policy5(&self) -> &Prefix {
        &self.prefix
    }
    pub const fn owner(&self) -> &Owner {
        self.continuation.owner()
    }
    pub const fn continuation(&self) -> &Continuation {
        &self.continuation
    }
    pub const fn execution(&self) -> &Policy6ExecutionWitnessV1 {
        &self.execution
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub fn native_input_audit_bytes(&self) -> &[u8] {
        self.prefix.native_input_audit_bytes()
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Replays both existing B/C/S/O and the actual O/I relation, not optimizer execution.
    pub fn replay(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<(), Error> {
        scoped(budget, |budget| {
            self.prefix.replay(input, budget).map_err(Error::Policy5)?;
            self.replay_continuation(input, budget)
        })
    }

    /// Only O/I and the complete composition subjects. The source consumer must
    /// separately admit the retained Policy5 prefix before composing its relation.
    pub fn replay_continuation(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<(), Error> {
        scoped(budget, |budget| {
            check_input(input, &self.prefix, budget)?;
            budget.charge_work(POLICY6_EXECUTION_RECORD_BYTES_V1 * 2 + 5)?;
            if self.execution.bytes != record(input, &self.prefix, &self.continuation) {
                return Err(Error::Execution);
            }
            let source = self.prefix.owner();
            let bytes = source.canonical().canonical_bytes();
            budget.charge_work(bytes.len())?;
            if bytes != self.continuation.native_input_audit_bytes() {
                return Err(Error::Execution);
            }
            self.continuation.execution().check_against(
                source,
                self.owner(),
                self.continuation.report(),
                self.continuation.map(),
                budget,
            )?;
            self.continuation
                .map()
                .check_against(source, self.owner(), budget)
                .map_err(Error::Map)?;
            let (before, before_storage) =
                CanonicalKirInventoryV1::derive(source, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(before_storage.retained_storage())?;
            let (after, after_storage) =
                CanonicalKirInventoryV1::derive(self.owner(), budget).map_err(Error::Inventory)?;
            budget.reserve_storage(after_storage.retained_storage())?;
            let (_checked, storage) = check_canonical_kir_transition_v1(
                &before,
                &after,
                self.continuation.occurrences().candidate(),
                budget,
            )
            .map_err(Error::Relation)?;
            budget.reserve_storage(storage.retained_storage())?;
            Ok(())
        })
    }
}

fn check_input(input: &Owner, prefix: &Prefix, budget: &mut Budget<'_>) -> Result<(), Error> {
    let bytes = input.canonical().canonical_bytes();
    budget.charge_work(bytes.len())?;
    if bytes != prefix.native_input_audit_bytes() {
        return Err(Error::Execution);
    }
    Ok(())
}

fn record(
    input: &Owner,
    prefix: &Prefix,
    continuation: &Continuation,
) -> [u8; POLICY6_EXECUTION_RECORD_BYTES_V1] {
    let mut bytes = [0; POLICY6_EXECUTION_RECORD_BYTES_V1];
    bytes[..8].copy_from_slice(b"F2P6EX1\0");
    bytes[8..16].copy_from_slice(&[1, 0, 6, 0, 5, 0, 2, 0]);
    for (offset, owner) in [
        (16, input),
        (
            56,
            prefix.intermediate_policy4().intermediate_policy3().owner(),
        ),
        (96, prefix.intermediate_policy4().owner()),
        (136, prefix.owner()),
        (176, continuation.owner()),
    ] {
        let identity = owner.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(identity.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    bytes[216..248].copy_from_slice(continuation.map().digest());
    bytes[248..256].copy_from_slice(&2_u64.to_le_bytes());
    bytes
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    finish_scope(budget, floor, run)
}

fn finish_scope<'work, T>(
    budget: &mut Budget<'work>,
    floor: usize,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let ledger = budget.work_ledger_identity_v1();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    match release {
        Ok(()) => result,
        Err(error) => {
            drop(result);
            Err(error.into())
        }
    }
}

/// Consume an already executed Policy5 prefix; never rerun it or emit native O.
/// B and the prefix must already be reserved on this same ledger. Every exit
/// removes the consumed prefix reservation after its custody has dropped or moved.
/// Success returns the full Policy6 transfer receipt unreserved; reserve it before
/// further controlled allocation. Work/peak/failure history is never reset.
/// This deterministic physical-order sweep is not a general fixed point.
pub fn continue_checked_canonical_kernel_ir_policy6_v1(
    input: &Owner,
    prefix: Prefix,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalKernelIrOwnerPolicy6V1, Error> {
    let Some(floor) = budget.storage().checked_sub(prefix.retained_storage()) else {
        drop(prefix);
        return Err(Resource::Accounting.into());
    };
    finish_scope(budget, floor, move |budget| {
        check_input(input, &prefix, budget)?;
        budget.charge_work(3)?;
        let wrapper = size_of::<CheckedCanonicalKernelIrOwnerPolicy6V1>()
            .checked_sub(size_of::<Prefix>())
            .and_then(|bytes| bytes.checked_sub(size_of::<Continuation>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let observed =
            optimize_native_neutral_kernel_ir_integer_continuation_v1(prefix.owner(), budget)
                .map_err(Error::Observation)?;
        budget.reserve_storage(observed.storage().retained_storage())?;
        budget.charge_work(1)?;
        if !std::ptr::eq(prefix.owner(), observed.input()) {
            return Err(Error::Execution);
        }
        let continuation = observed
            .try_check_and_finish_v1(budget)
            .map_err(Error::Check)?;
        budget.reserve_storage(continuation.storage().retained_storage())?;
        budget.charge_work(POLICY6_EXECUTION_RECORD_BYTES_V1 + 5)?;
        let execution = Policy6ExecutionWitnessV1 {
            bytes: record(input, &prefix, &continuation),
        };
        let retained = prefix
            .retained_storage()
            .checked_add(continuation.storage().retained_storage())
            .and_then(|bytes| bytes.checked_add(wrapper))
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalKernelIrOwnerPolicy6V1 {
            prefix,
            continuation,
            execution,
            retained,
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy6_v1_tests.rs"]
mod tests;
