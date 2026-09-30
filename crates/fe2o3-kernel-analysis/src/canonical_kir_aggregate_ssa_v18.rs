//! Closed aggregate-leaf replacement and secondary SSA on actual V18 owners.
use crate::canonical_kir_private_cell_pair_resources_v1 as resources;
use crate::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV18 as Inventory};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Constant, FixedVectorTypeV12,
    MemoryAccess, Module, OperationKind as Kind, ScalarType, StorageLayoutIdV1,
    StorageLayoutKindV1 as LayoutKind, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrIdentityV18 as Identity, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_mir_model::{
    SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaConstructionPlanV1, SsaEdgeIdV1,
    SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaPlanIdentityV1, SsaPlannerErrorV1,
    SsaPlannerLimitsV1, SsaResolvedEventV1, SsaValueV1, SsaVariableIdV1, plan_ssa_with_limits_v1,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_aggregate_ssa_census_v18.rs"]
mod census;
#[path = "canonical_kir_aggregate_memory_inventory_v31.rs"]
mod memory_inventory_v31;
pub use memory_inventory_v31::*;
#[path = "canonical_kir_aggregate_occurrences_v30.rs"]
mod occurrences;
#[path = "canonical_kir_aggregate_ssa_pair_v18.rs"]
mod pair;
#[path = "canonical_kir_aggregate_ssa_plan_v18.rs"]
mod plan;
pub use occurrences::*;

/// No partial candidate or witness is returned on a denied transaction.
#[derive(Debug)]
pub enum CanonicalKirAggregateSsaErrorV18 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Planner(SsaPlannerErrorV1),
    Inconsistent(&'static str),
    ForeignInput,
    Panicked,
}
type Error = CanonicalKirAggregateSsaErrorV18;
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
impl From<SsaPlannerErrorV1> for Error {
    fn from(e: SsaPlannerErrorV1) -> Self {
        Self::Planner(e)
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
            Self::Planner(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirAggregateSsaLeafTypeV18 {
    Scalar(ScalarType),
    Vector(FixedVectorTypeV12),
}
impl CanonicalKirAggregateSsaLeafTypeV18 {
    pub fn ty(self) -> Type {
        match self {
            Self::Scalar(x) => Type::Scalar(x),
            Self::Vector(x) => Type::Vector(x),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirAggregateSsaActionV18 {
    Retain,
    Remove,
    Copy(ValueId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateSsaParameterV18 {
    pub block: usize,
    pub value: ValueId,
    pub ty: CanonicalKirAggregateSsaLeafTypeV18,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateSsaEdgeArgumentV18 {
    pub edge: usize,
    pub value: ValueId,
}

/// Exact original private leaf slot. The allocation coordinate names the
/// original Alloca, not a caller-supplied provenance or a runtime address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateSsaMemorySlotV18 {
    pub allocation: usize,
    pub layout: StorageLayoutIdV1,
    pub offset: u64,
    pub ty: CanonicalKirAggregateSsaLeafTypeV18,
}
/// Concrete local-memory events removed/replaced by this checked relation.
/// None covers every retained operation, including all unselected memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirAggregateSsaMemoryEventV18 {
    None,
    Allocate {
        allocation: usize,
    },
    Project {
        allocation: usize,
    },
    Read {
        slot: usize,
        output: ValueId,
        replacement: ValueId,
    },
    Write {
        slot: usize,
        value: ValueId,
    },
}
/// Associates a newly appended SSA parameter with its exact original leaf slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateSsaMemoryParameterV18 {
    pub parameter: usize,
    pub slot: usize,
}

/// Complete deterministic recipe, not a second executable graph or a proof.
struct Witness {
    actions: Vec<Action>,
    parameters: Vec<Parameter>,
    arguments: Vec<EdgeArgument>,
    conditions: Vec<Option<ValueId>>,
    selected: Vec<usize>,
    planners: Vec<(usize, SsaPlanIdentityV1)>,
    memory_slots: Vec<Option<CanonicalKirAggregateSsaMemorySlotV18>>,
    memory_events: Vec<CanonicalKirAggregateSsaMemoryEventV18>,
    memory_parameters: Vec<CanonicalKirAggregateSsaMemoryParameterV18>,
}
fn bytes<T>(v: &Vec<T>) -> Result<usize> {
    v.capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
fn append<T>(rows: &mut Vec<T>, value: T, meter: &mut Meter<'_, '_>) -> Result<()> {
    if rows.len() == rows.capacity() {
        let extra = rows.capacity().max(4);
        let requested = add(rows.capacity(), extra)?;
        meter.work(rows.len())?;
        meter.reserve(
            extra
                .checked_mul(size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        rows.try_reserve_exact(extra)
            .map_err(|_| Resource::Allocation)?;
        meter.capacity::<T>(requested, rows.capacity())?;
    }
    meter.push(rows, value)
}
impl Witness {
    fn retained(&self) -> Result<usize> {
        [
            bytes(&self.actions)?,
            bytes(&self.parameters)?,
            bytes(&self.arguments)?,
            bytes(&self.conditions)?,
            bytes(&self.selected)?,
            bytes(&self.planners)?,
            bytes(&self.memory_slots)?,
            bytes(&self.memory_events)?,
            bytes(&self.memory_parameters)?,
        ]
        .into_iter()
        .try_fold(size_of::<Self>(), add)
    }
}

type LeafType = CanonicalKirAggregateSsaLeafTypeV18;
type Action = CanonicalKirAggregateSsaActionV18;
type Parameter = CanonicalKirAggregateSsaParameterV18;
type EdgeArgument = CanonicalKirAggregateSsaEdgeArgumentV18;

/// Inert complete original-derived rewrite recipe. No caller construction,
/// mutable rows, cloning, source proof or publication authority is exposed.
pub struct CanonicalKirAggregateSsaWitnessV18 {
    input_identity: Identity,
    rows: Witness,
    retained: usize,
}
impl CanonicalKirAggregateSsaWitnessV18 {
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub fn actions(&self) -> &[CanonicalKirAggregateSsaActionV18] {
        &self.rows.actions
    }
    pub fn parameters(&self) -> &[CanonicalKirAggregateSsaParameterV18] {
        &self.rows.parameters
    }
    pub fn arguments(&self) -> &[CanonicalKirAggregateSsaEdgeArgumentV18] {
        &self.rows.arguments
    }
    pub fn conditions(&self) -> &[Option<ValueId>] {
        &self.rows.conditions
    }
    pub fn selected_allocations(&self) -> &[usize] {
        &self.rows.selected
    }
    pub fn planner_identities(&self) -> &[(usize, SsaPlanIdentityV1)] {
        &self.rows.planners
    }
    /// Paid original census order. None denotes an unselected leaf and cannot
    /// be used as an optimized slot. No memory graph or contents are copied.
    pub fn memory_slots(&self) -> &[Option<CanonicalKirAggregateSsaMemorySlotV18>] {
        &self.rows.memory_slots
    }
    /// One exact event for each original operation, independently regenerated
    /// by pair replay. An Allocate kills every selected slot of that Alloca.
    pub fn memory_events(&self) -> &[CanonicalKirAggregateSsaMemoryEventV18] {
        &self.rows.memory_events
    }
    pub fn memory_parameters(&self) -> &[CanonicalKirAggregateSsaMemoryParameterV18] {
        &self.rows.memory_parameters
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
fn retained(rows: &Witness) -> Result<usize> {
    add(
        size_of::<CanonicalKirAggregateSsaWitnessV18>() - size_of::<Witness>(),
        rows.retained()?,
    )
}
/// Derives a deterministic closed-use/layout census and bounded SSA plan from
/// the genuine V18 owner. Pointer/slice payloads, unions/tags, dynamic indices,
/// copies, escapes, opaque calls/assembly and uninitialized reads stay in memory.
/// Records/fixed arrays are not expanded by element count: only actual typed
/// leaf uses become cells. This is a restricted scalar-replacement relation,
/// not complete SROA, source lifetime/Drop refinement or final native admission.
/// The returned witness's receipt is unreserved; keep the input paid separately.
pub fn derive_canonical_kir_aggregate_ssa_v18(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CanonicalKirAggregateSsaWitnessV18> {
    resources::scoped(budget, |meter| {
        meter.reserve(
            size_of::<CanonicalKirAggregateSsaWitnessV18>()
                + size_of::<Result<CanonicalKirAggregateSsaWitnessV18>>()
                + size_of::<(&Owner, &mut Budget<'_>)>(),
        )?;
        let (inventory, receipt) = meter.derive(|b| Ok(Inventory::derive_v18(input, b)?))?;
        meter.reserve(receipt.retained_storage())?;
        let rows = plan::derive(&inventory, meter)?;
        let retained = retained(&rows)?;
        Ok(CanonicalKirAggregateSsaWitnessV18 {
            input_identity: *input.identity(),
            rows,
            retained,
        })
    })
}
/// Borrowed independently replayed actual endpoints, not a mutation permission.
pub struct CheckedCanonicalKirAggregateSsaV18<'a> {
    input: &'a Owner,
    output: &'a Owner,
    witness: &'a CanonicalKirAggregateSsaWitnessV18,
}
impl<'a> CheckedCanonicalKirAggregateSsaV18<'a> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn witness(&self) -> &'a CanonicalKirAggregateSsaWitnessV18 {
        self.witness
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
#[derive(Clone, Copy)]
pub struct CanonicalKirAggregateSsaStorageV18(usize);
impl CanonicalKirAggregateSsaStorageV18 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
/// Regenerates all original facts and checks every final operation, parameter,
/// edge and unchanged layout/metadata without invoking the mutation routine.
/// Matching identities or recipe rows never replace actual endpoint replay.
pub fn check_canonical_kir_aggregate_ssa_v18<'a>(
    input: &'a Owner,
    output: &'a Owner,
    witness: &'a CanonicalKirAggregateSsaWitnessV18,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirAggregateSsaV18<'a>,
    CanonicalKirAggregateSsaStorageV18,
)> {
    resources::scoped(budget, |meter| {
        meter.reserve(
            size_of::<CheckedCanonicalKirAggregateSsaV18<'a>>()
                + size_of::<CanonicalKirAggregateSsaStorageV18>()
                + size_of::<
                    Result<(
                        CheckedCanonicalKirAggregateSsaV18<'a>,
                        CanonicalKirAggregateSsaStorageV18,
                    )>,
                >()
                + size_of::<(
                    &Owner,
                    &Owner,
                    &CanonicalKirAggregateSsaWitnessV18,
                    &mut Budget<'_>,
                )>(),
        )?;
        meter.work(size_of::<Identity>())?;
        if input.identity() != &witness.input_identity {
            return Err(Error::ForeignInput);
        }
        if witness.retained != retained(&witness.rows)? {
            return Err(Resource::Accounting.into());
        }
        let (inventory, receipt) = meter.derive(|b| Ok(Inventory::derive_v18(input, b)?))?;
        meter.reserve(receipt.retained_storage())?;
        let expected = plan::derive(&inventory, meter)?;
        meter.work(add(
            input.canonical_bytes().len(),
            output.canonical_bytes().len(),
        )?)?;
        let rows = &witness.rows;
        if rows.actions != expected.actions
            || rows.parameters != expected.parameters
            || rows.arguments != expected.arguments
            || rows.conditions != expected.conditions
            || rows.selected != expected.selected
            || rows.planners != expected.planners
            || rows.memory_slots != expected.memory_slots
            || rows.memory_events != expected.memory_events
            || rows.memory_parameters != expected.memory_parameters
        {
            return Err(Error::Inconsistent(
                "complete deterministic original witness",
            ));
        }
        pair::check(&inventory, &expected, output.module(), meter)?;
        Ok((
            CheckedCanonicalKirAggregateSsaV18 {
                input,
                output,
                witness,
            },
            CanonicalKirAggregateSsaStorageV18(size_of::<CheckedCanonicalKirAggregateSsaV18<'a>>()),
        ))
    })
}

#[cfg(test)]
#[path = "canonical_kir_aggregate_ssa_v18_tests.rs"]
mod tests;
