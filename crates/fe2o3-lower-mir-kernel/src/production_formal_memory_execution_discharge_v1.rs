//! Same-graph execution exclusions; never erase the raw formal obligations.

use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalGuardedPathV1, FormalIndexWidth, FormalMemoryObligationError,
    FormalMemoryObligations, FormalSingletonExecutionV1, FunctionOperationLocation, Kernel, Module,
    ValueId, derive_formal_access_execution_conditions_v1, verify_module_ref,
};

use crate::ProductionFormalMemoryErrorV1;

/// Exact descriptive coordinates of one freshly checked singleton predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionFormalMemoryExecutionWitnessV1 {
    pub(crate) invocation: u64,
    pub(crate) index: ValueId,
    pub(crate) threshold: ValueId,
    pub(crate) predicate: ValueId,
    pub(crate) path: FormalGuardedPathV1,
}

impl ProductionFormalMemoryExecutionWitnessV1 {
    /// The only invocation that may reach the witnessed access.
    pub const fn invocation(self) -> u64 {
        self.invocation
    }
    /// Actual SSA index used by the singleton comparison.
    pub const fn index(self) -> ValueId {
        self.index
    }
    /// Actual SSA value defining the comparison threshold.
    pub const fn threshold(self) -> ValueId {
        self.threshold
    }
    /// Actual SSA predicate selecting the dominating true edge.
    pub const fn predicate(self) -> ValueId {
        self.predicate
    }
    /// Descriptive control-flow coordinates, not independent proof authority.
    pub const fn path(self) -> FormalGuardedPathV1 {
        self.path
    }
}

impl From<FormalSingletonExecutionV1> for ProductionFormalMemoryExecutionWitnessV1 {
    fn from(value: FormalSingletonExecutionV1) -> Self {
        Self {
            invocation: value.invocation(),
            index: value.index(),
            threshold: value.threshold(),
            predicate: value.predicate(),
            path: value.path(),
        }
    }
}

/// One exact raw conflict excluded only for distinct invocations.
///
/// This is neither bounds/alias evidence nor a runtime launch authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionFormalMemoryExecutionDischargeV1 {
    pub(crate) conflict_ordinal: u32,
    pub(crate) left: FunctionOperationLocation,
    pub(crate) right: FunctionOperationLocation,
    pub(crate) allocation_parameter: u32,
    pub(crate) left_witness: ProductionFormalMemoryExecutionWitnessV1,
    pub(crate) right_witness: ProductionFormalMemoryExecutionWitnessV1,
}

impl ProductionFormalMemoryExecutionDischargeV1 {
    /// Position in the unchanged raw conflict roster.
    pub const fn conflict_ordinal(self) -> u32 {
        self.conflict_ordinal
    }
    /// Actual first memory operation of the raw conflict.
    pub const fn left(self) -> FunctionOperationLocation {
        self.left
    }
    /// Actual second memory operation of the raw conflict.
    pub const fn right(self) -> FunctionOperationLocation {
        self.right
    }
    /// Formal parameter identity, not an authenticated runtime allocation.
    pub const fn allocation_parameter(self) -> u32 {
        self.allocation_parameter
    }
    /// Singleton witness rederived for the first access.
    pub const fn left_witness(self) -> ProductionFormalMemoryExecutionWitnessV1 {
        self.left_witness
    }
    /// Singleton witness rederived for the second access.
    pub const fn right_witness(self) -> ProductionFormalMemoryExecutionWitnessV1 {
        self.right_witness
    }
    /// Descriptive evidence never grants source, native, or runtime authority.
    pub const fn grants_authority(self) -> bool {
        false
    }
}

// Every retained row must fit the unchanged outer evidence byte limit. The
// analysis itself retains its existing independent work/storage/record caps;
// these are not charges against an optimizer or semantic-emission allowance.
pub(crate) const MAX_EXECUTION_DISCHARGES_V1: usize =
    crate::MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4 / 104;

pub(crate) fn derive_execution_discharges_v1(
    module: &Module,
    kernel: &Kernel,
    obligations: &FormalMemoryObligations,
) -> Result<Box<[ProductionFormalMemoryExecutionDischargeV1]>, ProductionFormalMemoryErrorV1> {
    let conflicts = obligations.inter_invocation_conflicts();
    if conflicts.is_empty() {
        return Ok(Vec::new().into_boxed_slice());
    }
    if conflicts.len() > MAX_EXECUTION_DISCHARGES_V1
        || obligations.kernel() != &kernel.id
        || obligations.entry() != &kernel.entry
    {
        return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
    }
    let verified = verify_module_ref(module).map_err(|error| {
        ProductionFormalMemoryErrorV1::Analysis(FormalMemoryObligationError::InvalidModule(error))
    })?;
    let analysis = derive_formal_access_execution_conditions_v1(
        verified,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: kernel.domain.rank(),
            extents: super::production_formal_memory_v1::witness_extents(&kernel.domain),
        },
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    if analysis.invocations() != obligations.invocations() {
        return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
    }
    let witness = |location| {
        analysis
            .accesses()
            .binary_search_by_key(&location, |row| row.location())
            .ok()
            .and_then(|index| analysis.accesses()[index].singleton())
            .map(ProductionFormalMemoryExecutionWitnessV1::from)
    };
    let mut rows = Vec::new();
    rows.try_reserve_exact(conflicts.len())
        .map_err(|_| ProductionFormalMemoryErrorV1::ObligationMismatch)?;
    for (ordinal, conflict) in conflicts.iter().copied().enumerate() {
        if !analysis.excludes_distinct_invocations(
            verified,
            &kernel.id,
            conflict.left(),
            conflict.right(),
        ) {
            return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
                conflicts: conflicts.to_vec().into_boxed_slice(),
            });
        }
        rows.push(ProductionFormalMemoryExecutionDischargeV1 {
            conflict_ordinal: u32::try_from(ordinal)
                .map_err(|_| ProductionFormalMemoryErrorV1::ObligationMismatch)?,
            left: conflict.left(),
            right: conflict.right(),
            allocation_parameter: conflict.allocation().parameter_index(),
            left_witness: witness(conflict.left())
                .ok_or(ProductionFormalMemoryErrorV1::ObligationMismatch)?,
            right_witness: witness(conflict.right())
                .ok_or(ProductionFormalMemoryErrorV1::ObligationMismatch)?,
        });
    }
    Ok(rows.into_boxed_slice())
}

#[cfg(test)]
#[path = "production_formal_memory_execution_discharge_v1_tests.rs"]
pub(crate) mod tests;
