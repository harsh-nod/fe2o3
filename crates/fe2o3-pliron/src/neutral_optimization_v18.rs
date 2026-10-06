//! Nominal V18 observations from the shared fixed scalar executor.
use super::{
    KirCheckedNeutralOptimizationErrorV1, KirCheckedNeutralOptimizationStorageV1,
    KirNeutralOptimizationStorageV1, KirNeutralOwnedOriginStorageV1,
    checked_neutral_optimization_v1::{CheckedParts, ObservedParts},
};
use crate::{
    KirBridgeErrorV18, KirBridgeReportV18, KirNeutralOccurrenceRowsV1, KirOptimizationMapErrorV12,
    KirOptimizationMapPolicy3V18, PlironOptimizationErrorV1, PlironOptimizationErrorV12,
    PlironOptimizationReportV1, Policy3ExecutionWitnessV18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{error::Error, fmt, mem::size_of};

#[derive(Debug)]
pub enum KirNeutralOptimizationErrorV18 {
    Bridge(KirBridgeErrorV18),
    Execution(PlironOptimizationErrorV12),
    Pass(PlironOptimizationErrorV1),
    Mapping(KirOptimizationMapErrorV12),
    Resource(Resource),
    Endpoint,
    Limit,
    Panicked,
}
impl From<Resource> for KirNeutralOptimizationErrorV18 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for KirNeutralOptimizationErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(e) => e.fmt(f),
            Self::Execution(e) => e.fmt(f),
            Self::Pass(e) => e.fmt(f),
            Self::Mapping(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::Endpoint => f.write_str("V18 optimized endpoint or epoch mismatch"),
            Self::Limit => f.write_str("V18 scalar optimizer canonical growth limit exceeded"),
            Self::Panicked => f.write_str("V18 fixed optimization panicked"),
        }
    }
}
impl Error for KirNeutralOptimizationErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bridge(error) => Some(error),
            Self::Execution(error) => Some(error),
            Self::Pass(error) => Some(error),
            Self::Mapping(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Endpoint | Self::Limit | Self::Panicked => None,
        }
    }
}

type Observed<'a> = ObservedParts<
    'a,
    KirOptimizationMapPolicy3V18,
    Policy3ExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;
type Checked = CheckedParts<
    KirOptimizationMapPolicy3V18,
    Policy3ExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;

/// Move-only observed candidate. Structural validity and actual pass execution
/// are not semantic preservation, source proof, final safety or native authority.
///
/// ```compile_fail
/// use fe2o3_pliron::{KirNeutralOptimizationOutputV18, KirNeutralOptimizationOutputV1};
/// fn erase<'a>(v: KirNeutralOptimizationOutputV18<'a>) -> KirNeutralOptimizationOutputV1<'a> { v }
/// ```
pub struct KirNeutralOptimizationOutputV18<'input> {
    parts: Observed<'input>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    caller_floor: usize,
}

fn wrapper<T>() -> Option<usize> {
    [
        size_of::<Owner>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeReportV18>(),
        size_of::<KirOptimizationMapPolicy3V18>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ]
    .into_iter()
    .try_fold(size_of::<T>(), usize::checked_sub)
}
fn observed_wrapper() -> Option<usize> {
    wrapper::<KirNeutralOptimizationOutputV18<'_>>()
}
fn checked_wrapper() -> Option<usize> {
    wrapper::<CheckedNeutralKernelIrOwnerV18>()
}

impl<'input> KirNeutralOptimizationOutputV18<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.parts.input
    }
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn map(&self) -> &KirOptimizationMapPolicy3V18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &Policy3ExecutionWitnessV18 {
        &self.parts.extra
    }
    pub const fn storage(&self) -> KirNeutralOptimizationStorageV1 {
        self.parts.storage
    }
}

/// Run the fixed eight-pass scalar cluster once, using one private V18 session
/// and the actual occurrence/capture engines. There is no configurable pass list
/// or unoptimized fallback. All private session state drops before transfer.
/// Reserve input custody before calling and the returned receipt before any
/// subsequent controlled allocation. Final source/safety checks remain required.
/// Adoption must use this same budget instance at the same address; moving to
/// a new ledger or budget wrapper does not transfer observed custody.
pub fn optimize_neutral_kernel_ir_v18<'input>(
    input: &'input Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<KirNeutralOptimizationOutputV18<'input>, KirNeutralOptimizationErrorV18> {
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(budget) as usize;
    let caller_floor = budget.storage();
    let wrapper = observed_wrapper().ok_or(Resource::Arithmetic)?;
    let crate::kir_bridge_v1::ExecutedV18Parts {
        owner,
        report,
        bridge,
        map,
        occurrences,
        execution,
        retained,
    } = crate::kir_bridge_v1::optimize_v18_graph(input, layouts, wrapper, budget)?;
    Ok(KirNeutralOptimizationOutputV18 {
        ledger,
        slot,
        caller_floor,
        parts: ObservedParts {
            input,
            owner,
            report,
            bridge,
            map,
            occurrences,
            extra: execution,
            storage: KirNeutralOptimizationStorageV1 { retained },
        },
    })
}

#[path = "checked_neutral_optimization_v18.rs"]
mod checked;
pub use checked::CheckedNeutralKernelIrOwnerV18;

#[cfg(test)]
#[path = "neutral_optimization_v18_hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "neutral_optimization_v18_resource_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "neutral_optimization_v18_tests.rs"]
pub(crate) mod tests;
