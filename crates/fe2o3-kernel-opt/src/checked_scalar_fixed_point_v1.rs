//! One closed scalar schedule, iterated to exact byte equality or typed refusal.
use crate::{
    KernelIrCheckedOptimizationErrorV1, optimize_checked_canonical_kernel_ir_policy3_v1,
    private_cell_promotion_resources_v1 as resources,
};
use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirTransitionErrorV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerIntegerContinuationV1 as Integer,
    CheckedNeutralKernelIrOwnerPolicy3V1 as Scalar, INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1,
    KirCheckedNeutralOptimizationErrorV1, KirNeutralOptimizationErrorV1,
    KirOptimizationMapErrorV12, PlironOptimizationPassV1 as Pass, Policy3ExecutionClaimErrorV1,
    UnauthenticatedPolicy3ExecutionClaimV1,
    optimize_native_neutral_kernel_ir_integer_continuation_v1,
};
use std::{error::Error as StdError, fmt, mem::size_of};

#[path = "checked_scalar_fixed_point_replay_v1.rs"]
mod replay;

/// Includes the mandatory terminal round; no caller-selected production limit.
pub const SCALAR_FIXED_POINT_MAX_ROUNDS_V1: usize = 16;
pub const SCALAR_FIXED_POINT_POLICY_ID_V1: &[u8] = b"FE2O3/CHECKED-SCALAR-FIXED-POINT/V1\0";
/// In-process execution header, not a decodable execution authority or wire.
pub const SCALAR_FIXED_POINT_EXECUTION_BYTES_V1: usize = 128;
const INTEGER_PASSES: [Pass; 2] = [
    Pass::IntegerNeutralCanonicalization,
    Pass::DeadCodeElimination,
];
const SCALAR_PASSES: [Pass; 8] = [
    Pass::SparseConditionalConstantPropagation,
    Pass::SimplifyControlFlow,
    Pass::SelectSameValueCanonicalization,
    Pass::DeadCodeElimination,
    Pass::LocalPureCommonSubexpressionElimination,
    Pass::DominancePureCommonSubexpressionElimination,
    Pass::DeadCodeElimination,
    Pass::SimplifyControlFlow,
];
// Fresh fixed record/claim/roster scratch, separate from retained witnesses.
const CHECK_SCRATCH: usize = INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1
    + SCALAR_FIXED_POINT_EXECUTION_BYTES_V1
    + size_of::<UnauthenticatedPolicy3ExecutionClaimV1<'static>>()
    + size_of::<[Pass; 8]>()
    + size_of::<[Pass; 2]>()
    + 128;

#[derive(Debug)]
#[non_exhaustive]
pub enum CheckedScalarFixedPointErrorV1 {
    Resource(Resource),
    IntegerObservation(KirNeutralOptimizationErrorV1),
    IntegerCheck(KirCheckedNeutralOptimizationErrorV1),
    Scalar(KernelIrCheckedOptimizationErrorV1),
    Inventory(CanonicalKirInventoryErrorV1),
    Transition(CanonicalKirTransitionErrorV1),
    Map(KirOptimizationMapErrorV12),
    ScalarExecution(Policy3ExecutionClaimErrorV1),
    History,
    IterationLimit { completed: usize, limit: usize },
    Panicked,
}
type Error = CheckedScalarFixedPointErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked scalar fixed point: {self:?}")
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::IntegerObservation(e) => Some(e),
            Self::IntegerCheck(e) => Some(e),
            Self::Scalar(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Transition(e) => Some(e),
            Self::Map(e) => Some(e),
            Self::ScalarExecution(e) => Some(e),
            _ => None,
        }
    }
}

/// Genuine adjacent checked substages, inspectable only through immutable views.
/// Complete maps/occurrences retain their own input coordinates, not original-Q0
/// coordinates. This is not a constructor for the eventual expanded policy.
pub struct CheckedScalarFixedPointRoundV1 {
    ordinal: u16,
    integer: Integer,
    scalar: Scalar,
}
impl CheckedScalarFixedPointRoundV1 {
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
    pub const fn integer(&self) -> &Integer {
        &self.integer
    }
    pub const fn scalar(&self) -> &Scalar {
        &self.scalar
    }
    pub const fn output(&self) -> &Owner {
        self.scalar.owner()
    }
}

/// Fixed composition header; ordered owning rounds carry the complete records.
/// No bytes-to-witness API, policy-3/6 conversion or native authority exists.
pub struct ScalarFixedPointExecutionV1 {
    bytes: [u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1],
}
impl ScalarFixedPointExecutionV1 {
    pub const fn policy_identity(&self) -> &'static [u8] {
        SCALAR_FIXED_POINT_POLICY_ID_V1
    }
    pub const fn canonical_bytes(&self) -> &[u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1] {
        &self.bytes
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Move-only typed history ending in an observed fixed point of the COMPLETE
/// integer/DCE then Policy3 schedule. Not proof that every scalar rule or each
/// individual pass is unchanged, nor that all inputs converge within 16 rounds.
/// Source/predecessor custody remains caller-owned; this grants no final formal,
/// source/ABI, native, publication or launch authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1;
/// fn duplicate(value: CheckedScalarFixedPointOwnerV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1;
/// fn forge() -> CheckedScalarFixedPointOwnerV1 { CheckedScalarFixedPointOwnerV1::default() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1};
/// fn attach(value: &mut CheckedScalarFixedPointOwnerV1, round: CheckedScalarFixedPointRoundV1) {
///     value.rounds().push(round);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedCanonicalKernelIrOwnerPolicy6V1};
/// fn relabel(value: CheckedScalarFixedPointOwnerV1) -> CheckedCanonicalKernelIrOwnerPolicy6V1 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn relabel(value: CheckedScalarFixedPointOwnerV1) -> CheckedNeutralKernelIrOwnerPolicy3V1 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1};
/// fn escape(value: CheckedScalarFixedPointOwnerV1) -> &'static CheckedScalarFixedPointRoundV1 { &value.rounds()[0] }
/// ```
pub struct CheckedScalarFixedPointOwnerV1 {
    rounds: Vec<CheckedScalarFixedPointRoundV1>,
    execution: ScalarFixedPointExecutionV1,
    retained: usize,
}
impl fmt::Debug for CheckedScalarFixedPointOwnerV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedScalarFixedPointOwnerV1")
            .field("round_count", &self.rounds.len())
            .field("retained", &self.retained)
            .finish_non_exhaustive()
    }
}
impl CheckedScalarFixedPointOwnerV1 {
    pub fn output(&self) -> &Owner {
        self.rounds
            .last()
            .expect("sealed nonempty fixed-point history")
            .output()
    }
    /// O(1) immutable view; caller retains this owner and pays further query work.
    pub fn rounds(&self) -> &[CheckedScalarFixedPointRoundV1] {
        &self.rounds
    }
    pub const fn execution(&self) -> &ScalarFixedPointExecutionV1 {
        &self.execution
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Rechecks EVERY adjacent pair against actual owners and original input.
    /// No optimizer runs here. Input, this history and all siblings stay prepaid
    /// on the caller's same ledger; temporary receipts end before refunds.
    pub fn replay_against(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<()> {
        replay::check(self, input, budget)
    }
}

fn equal_bytes(a: &[u8], b: &[u8], meter: &mut Meter<'_, '_>) -> Result<bool> {
    meter.work(
        a.len()
            .checked_add(b.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(a == b)
}
fn record(
    input: &Owner,
    output: &Owner,
    count: usize,
    meter: &mut Meter<'_, '_>,
) -> Result<[u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1]> {
    meter.work(SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 * 2)?;
    let count = u16::try_from(count).map_err(|_| Resource::Arithmetic)?;
    let mut bytes = [0; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1];
    bytes[..8].copy_from_slice(b"F2SFP1\0\0");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&(SCALAR_FIXED_POINT_MAX_ROUNDS_V1 as u16).to_le_bytes());
    bytes[12..14].copy_from_slice(&count.to_le_bytes());
    bytes[14..16].copy_from_slice(&2u16.to_le_bytes());
    for (offset, owner) in [(16, input), (56, output)] {
        let identity = owner.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(identity.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    // Each row is policy, pass count, zero-based substage ordinal; never a pass selector.
    bytes[96..104].copy_from_slice(&[6, 0, 2, 0, 0, 0, 0, 0]);
    bytes[104..112].copy_from_slice(&[3, 0, 8, 0, 1, 0, 0, 0]);
    Ok(bytes)
}
fn retained(capacity: usize, rounds: &[CheckedScalarFixedPointRoundV1]) -> Result<usize> {
    // Conservatively retain full component receipts AND every Vec slot. Inline
    // owner headers receive no inferred overlap discount; spare capacity is paid.
    let start = size_of::<CheckedScalarFixedPointOwnerV1>()
        .checked_add(
            capacity
                .checked_mul(size_of::<CheckedScalarFixedPointRoundV1>())
                .ok_or(Resource::Arithmetic)?,
        )
        .ok_or(Resource::Arithmetic)?;
    rounds.iter().try_fold(start, |n, round| {
        n.checked_add(round.integer.storage().retained_storage())
            .and_then(|n| n.checked_add(round.scalar.storage().retained_storage()))
            .ok_or_else(|| Resource::Arithmetic.into())
    })
}

fn checked_integer(input: &Owner, budget: &mut Budget<'_>) -> Result<Integer> {
    let observed = optimize_native_neutral_kernel_ir_integer_continuation_v1(input, budget)
        .map_err(Error::IntegerObservation)?;
    budget.reserve_storage(observed.storage().retained_storage())?;
    // The consuming finish refunds the observed receipt. This whole function is
    // ONE derive transaction; outer Meter accounting adopts only the result.
    observed
        .try_check_and_finish_v1(budget)
        .map_err(Error::IntegerCheck)
}

/// The caller prepays immutable input and siblings. One live ledger covers all
/// rounds, current observed/checker transactions and retained checked history.
/// Success restores entry storage and transfers the FULL receipt unreserved;
/// reserve it before any further controlled work. Failure discards all rounds.
/// The 16-round limit includes a real terminal round, never a partial fallback.
/// Existing structural/canonical stage caps remain unchanged. A future expanded
/// policy must additionally bound aggregate history across cleanup occurrences.
pub fn prepare_checked_scalar_fixed_point_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedScalarFixedPointOwnerV1> {
    prepare(input, SCALAR_FIXED_POINT_MAX_ROUNDS_V1, budget)
}
fn prepare(
    input: &Owner,
    limit: usize,
    budget: &mut Budget<'_>,
) -> Result<CheckedScalarFixedPointOwnerV1> {
    resources::scoped(budget, |meter| {
        meter.work(1)?;
        if limit == 0 || limit > SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
            return Err(Error::History);
        }
        meter.reserve(size_of::<CheckedScalarFixedPointOwnerV1>())?;
        meter.reserve(CHECK_SCRATCH)?;
        let (mut rounds, _) = meter.table::<CheckedScalarFixedPointRoundV1>(limit)?;
        for index in 0..limit {
            meter.work(1)?;
            let (integer, scalar, terminal) = {
                let source = rounds
                    .last()
                    .map_or(input, CheckedScalarFixedPointRoundV1::output);
                let integer = meter.derive(|b| checked_integer(source, b))?;
                meter.reserve(integer.storage().retained_storage())?;
                let scalar = meter.derive(|b| {
                    optimize_checked_canonical_kernel_ir_policy3_v1(integer.owner(), b)
                        .map_err(Error::Scalar)
                })?;
                meter.reserve(scalar.storage().retained_storage())?;
                replay::headers(source, &integer, &scalar, meter)?;
                let terminal = equal_bytes(
                    source.canonical().canonical_bytes(),
                    scalar.owner().canonical().canonical_bytes(),
                    meter,
                )?;
                (integer, scalar, terminal)
            };
            let ordinal = u16::try_from(index).map_err(|_| Resource::Arithmetic)?;
            meter.push(
                &mut rounds,
                CheckedScalarFixedPointRoundV1 {
                    ordinal,
                    integer,
                    scalar,
                },
            )?;
            if terminal {
                let output = rounds.last().ok_or(Error::History)?.output();
                let bytes = record(input, output, rounds.len(), meter)?;
                meter.work(rounds.len().checked_mul(3).ok_or(Resource::Arithmetic)?)?;
                let retained = retained(rounds.capacity(), &rounds)?;
                let value = CheckedScalarFixedPointOwnerV1 {
                    rounds,
                    execution: ScalarFixedPointExecutionV1 { bytes },
                    retained,
                };
                meter.work(1)?;
                return Ok(value);
            }
        }
        Err(Error::IterationLimit {
            completed: rounds.len(),
            limit,
        })
    })
}

#[cfg(test)]
#[path = "checked_scalar_fixed_point_fixture_v1_tests.rs"]
mod fixture;
#[cfg(test)]
#[path = "checked_scalar_fixed_point_process_v1_tests.rs"]
mod process_tests;
#[cfg(test)]
#[path = "checked_scalar_fixed_point_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "checked_scalar_fixed_point_v1_tests.rs"]
mod tests;
