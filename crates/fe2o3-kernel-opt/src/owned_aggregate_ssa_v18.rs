//! Closed aggregate-leaf replacement and secondary SSA on actual V18 owners.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV18 as Inventory};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18, CanonicalKernelIrReplayStorageV18,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Constant, Module, Operation,
    OperationKind as Kind, StorageLayoutLimitsV1, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrIdentityV18 as Identity, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "aggregate_ssa_materialize_v18.rs"]
mod materialize;

/// No partial candidate or witness is returned on a denied transaction.
#[derive(Debug)]
pub enum OwnedAggregateSsaErrorV18 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Admission(CanonicalKernelIrReplayAdmissionErrorV18),
    Check(fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18),
    Inconsistent(&'static str),
    ForeignInput,
    Panicked,
}
type Error = OwnedAggregateSsaErrorV18;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CanonicalKirInventoryErrorV1> for Error {
    fn from(e: CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(e)
    }
}
impl From<CanonicalKernelIrReplayAdmissionErrorV18> for Error {
    fn from(e: CanonicalKernelIrReplayAdmissionErrorV18) -> Self {
        Self::Admission(e)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18> for Error {
    fn from(e: fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18) -> Self {
        Self::Check(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "aggregate secondary SSA: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Admission(e) => Some(e),
            Self::Check(e) => Some(e),
            _ => None,
        }
    }
}

use fe2o3_kernel_analysis::{
    CanonicalKirAggregateSsaActionV18 as Action, CanonicalKirAggregateSsaWitnessV18 as Witness,
    check_canonical_kir_aggregate_ssa_v18, derive_canonical_kir_aggregate_ssa_v18,
};
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
/// Move-only freshly admitted output and exact original-to-final witness.
/// Records/fixed arrays with closed scalar/vector leaf uses are supported;
/// unions, variants, pointer/slice payloads, dynamic projections, whole-object
/// copies, escaping addresses and observable lifetime/Drop uses stay in memory.
/// This is not complete SROA, source allocation refinement, or native authority.
pub struct OwnedAggregateSsaContinuationV18 {
    output: Owner,
    output_storage: CanonicalKernelIrReplayStorageV18,
    input_identity: Identity,
    witness: Witness,
    retained: usize,
}
impl OwnedAggregateSsaContinuationV18 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Original-derived inert rows, not a caller-constructible mutation permit.
    pub const fn witness(&self) -> &Witness {
        &self.witness
    }
    pub fn promoted_allocations(&self) -> usize {
        self.witness.selected_allocations().len()
    }
    pub fn inserted_parameters(&self) -> usize {
        self.witness.parameters().len()
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Rebuilds original census and planner facts, then independently compares
    /// every real final operation, parameter and edge. Identity is only a filter.
    pub fn replay_against(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<()> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(size_of::<Identity>())?;
            if input.identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            if self.retained != retained(self.output_storage, &self.witness)? {
                return Err(Resource::Accounting.into());
            }
            check(input, &self.output, &self.witness, meter)
        })
    }
}
fn retained(output: CanonicalKernelIrReplayStorageV18, witness: &Witness) -> Result<usize> {
    add(
        add(
            size_of::<OwnedAggregateSsaContinuationV18>()
                - size_of::<Owner>()
                - size_of::<Witness>(),
            output.retained_storage(),
        )?,
        witness.retained_storage(),
    )
}

/// Runs one closed census/SSA transaction on the genuine storage-aware graph.
/// All selected allocations are removed as units, never partially invalidated.
/// Read-before-initialization and unsupported uses retain the whole allocation.
/// The independent planner handles diamonds, loops and distinct parallel edges.
/// Original producers/effects remain ordered; loads become concrete same-value
/// Selects so existing scalar canonicalization can remove the copies afterward.
///
/// The input reservation remains caller-owned. Success restores entry storage
/// and transfers the output receipt unreserved; reserve it before further work.
/// All scratch/candidates are dropped before same-ledger cleanup on refusal.
/// Original CFG/MemorySSA analyses cannot be reused for the newly admitted owner.
pub fn prepare_owned_aggregate_ssa_v18(
    input: &Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedAggregateSsaContinuationV18> {
    resources::scoped(budget, |meter| {
        meter.reserve(
            size_of::<OwnedAggregateSsaContinuationV18>()
                + size_of::<StorageLayoutLimitsV1>()
                + size_of::<Result<OwnedAggregateSsaContinuationV18>>()
                + size_of::<(&Owner, StorageLayoutLimitsV1, &mut Budget<'_>)>(),
        )?;
        let (inventory, receipt) = meter.derive(|b| Ok(Inventory::derive_v18(input, b)?))?;
        meter.reserve(receipt.retained_storage())?;
        let witness = meter.derive(|b| Ok(derive_canonical_kir_aggregate_ssa_v18(input, b)?))?;
        meter.reserve(witness.retained_storage())?;
        let (mut candidate, copied) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v18(b)?))?;
        meter.reserve(copied.retained_storage())?;
        materialize::apply(&inventory, &witness, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v18(
                &candidate, layouts, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        check(input, &output, &witness, meter)?;
        let retained = retained(output_storage, &witness)?;
        drop(candidate);
        drop(inventory);
        meter.work(1)?;
        Ok(OwnedAggregateSsaContinuationV18 {
            output,
            output_storage,
            input_identity: *input.identity(),
            witness,
            retained,
        })
    })
}
fn check(
    input: &Owner,
    output: &Owner,
    witness: &Witness,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    let (checked, receipt) = meter.derive(|b| {
        Ok(check_canonical_kir_aggregate_ssa_v18(
            input, output, witness, b,
        )?)
    })?;
    meter.reserve(receipt.retained_storage())?;
    drop(checked);
    meter.release(receipt.retained_storage())
}
#[cfg(test)]
#[path = "aggregate_ssa_v18_tests.rs"]
mod tests;
