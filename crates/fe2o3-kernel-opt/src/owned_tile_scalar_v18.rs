//! Explicit tile expansion on the complete actual V18 SSA/CFG.

use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV18 as Inventory};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18, CanonicalKernelIrReplayStorageV18,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirFunctionCoordinateV1,
    ExecutionOperationV15 as Execution, ExecutionTileLayoutV1, ExecutionTileScalarLoweringV1,
    ExecutionTileScheduleV1, Module, Operation, OperationKind as Kind, StorageLayoutLimitsV1, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrIdentityV18 as Identity,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "tile_scalar_check_v18.rs"]
mod check;
#[path = "tile_scalar_materialize_v18.rs"]
mod materialize;

/// Explicit policy for one actual function coordinate. The complete sorted
/// roster is input data, not source authority or an implicit target default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileScalarFunctionSelectionV18 {
    pub function: CanonicalKirFunctionCoordinateV1,
    pub layout: ExecutionTileLayoutV1,
}

#[derive(Debug)]
pub enum OwnedTileScalarErrorV18 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Admission(CanonicalKernelIrReplayAdmissionErrorV18),
    Inconsistent(&'static str),
    ForeignInput,
    Panicked,
}
type Error = OwnedTileScalarErrorV18;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CanonicalKirInventoryErrorV1> for Error {
    fn from(error: CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(error)
    }
}
impl From<CanonicalKernelIrReplayAdmissionErrorV18> for Error {
    fn from(error: CanonicalKernelIrReplayAdmissionErrorV18) -> Self {
        Self::Admission(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tile scalar expansion: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy)]
enum Action {
    Retain,
    Load(ExecutionTileScalarLoweringV1),
    Fragment,
    Parts(ExecutionTileScalarLoweringV1),
    Scope,
}
struct Plan {
    actions: Vec<Action>,
    roles: Vec<Option<ExecutionTileScalarLoweringV1>>,
}

/// A freshly admitted candidate with independently checked scalar contents and
/// complete unchanged CFG/metadata. It grants no source, formal, native, or
/// launch authority. The selected layout is observable and remains explicit.
pub struct OwnedTileScalarContinuationV18 {
    output: Owner,
    output_storage: CanonicalKernelIrReplayStorageV18,
    input_identity: Identity,
    selections: Vec<TileScalarFunctionSelectionV18>,
    retained: usize,
}
impl OwnedTileScalarContinuationV18 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    pub fn selections(&self) -> &[TileScalarFunctionSelectionV18] {
        &self.selections
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Re-derives the original role census and compares every actual output
    /// operation and CFG field without invoking the mutation routine.
    pub fn replay_against(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<()> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(size_of::<Identity>())?;
            if input.identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            if self.retained != retained(self.output_storage, self.selections.capacity())? {
                return Err(Resource::Accounting.into());
            }
            let (inventory, receipt) =
                meter.derive(|budget| Ok(Inventory::derive_v18(input, budget)?))?;
            meter.reserve(receipt.retained_storage())?;
            let plan = derive(&inventory, &self.selections, meter)?;
            check::compare(&inventory, &self.output, &plan, meter)
        })
    }
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
fn product(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(output: CanonicalKernelIrReplayStorageV18, selections: usize) -> Result<usize> {
    add(
        add(
            size_of::<OwnedTileScalarContinuationV18>() - size_of::<Owner>(),
            output.retained_storage(),
        )?,
        product(selections, size_of::<TileScalarFunctionSelectionV18>())?,
    )
}

/// Expands every tile load in the explicitly selected actual kernel roots.
/// Load effects stay at their original operation positions, including discarded
/// tiles. Fragment transports become pure scalar copies with the exact original
/// result IDs/types. Scope exits retain their workgroup owner and every unrelated
/// discard. CFG blocks, block arguments, successor occurrences and loop structure
/// are unchanged; role lookup never assumes physical block order is dominance.
///
/// The V18 verifier rejects execution roles in block/function parameters, so
/// loop-carried or merged role parameters are not silently scalarized. Ordinary
/// cross-block dominated role uses are supported. Unselected functions are exact
/// copies. Success restores incoming storage and transfers an unreserved receipt.
pub fn prepare_owned_tile_scalar_v18(
    input: &Owner,
    selections: &[TileScalarFunctionSelectionV18],
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedTileScalarContinuationV18> {
    resources::scoped(budget, |meter| {
        meter.reserve(
            size_of::<OwnedTileScalarContinuationV18>()
                + size_of::<Plan>()
                + size_of::<Result<OwnedTileScalarContinuationV18>>(),
        )?;
        let (mut selected, _) = meter.table(selections.len())?;
        for selection in selections {
            meter.push(&mut selected, *selection)?;
        }
        let (inventory, receipt) =
            meter.derive(|budget| Ok(Inventory::derive_v18(input, budget)?))?;
        meter.reserve(receipt.retained_storage())?;
        let plan = derive(&inventory, &selected, meter)?;
        let (mut candidate, copied) =
            meter.derive(|budget| Ok(input.copy_module_for_transformation_v18(budget)?))?;
        meter.reserve(copied.retained_storage())?;
        materialize::apply(&inventory, &plan, &mut candidate, meter)?;
        let (output, storage) = meter.derive(|budget| {
            Ok(Owner::from_module_ref_with_verification_budget_v18(
                &candidate, layouts, budget,
            )?)
        })?;
        meter.reserve(storage.retained_storage())?;
        check::compare(&inventory, &output, &plan, meter)?;
        let retained = retained(storage, selected.capacity())?;
        drop(candidate);
        drop(plan);
        drop(inventory);
        meter.work(1)?;
        Ok(OwnedTileScalarContinuationV18 {
            output,
            output_storage: storage,
            input_identity: *input.identity(),
            selections: selected,
            retained,
        })
    })
}

fn derive(
    inventory: &Inventory<'_>,
    selections: &[TileScalarFunctionSelectionV18],
    meter: &mut Meter<'_, '_>,
) -> Result<Plan> {
    let (mut policies, _) = meter.table(inventory.functions().len())?;
    let (mut next_values, _) = meter.table(inventory.functions().len())?;
    for _ in inventory.functions() {
        meter.push(&mut policies, None)?;
        meter.push(&mut next_values, 0_u32)?;
    }
    let mut previous = None;
    for selection in selections {
        meter.work(3)?;
        let ordinal = selection.function.0 as usize;
        if ordinal >= policies.len() || previous.is_some_and(|value| value >= selection.function.0)
        {
            return Err(Error::Inconsistent(
                "tile policies must be sorted unique actual functions",
            ));
        }
        policies[ordinal] = Some(selection.layout);
        previous = Some(selection.function.0);
    }
    let (mut geometry, _) = meter.table(inventory.functions().len())?;
    for _ in inventory.functions() {
        meter.push(&mut geometry, None)?;
    }
    for entry in inventory.kernels() {
        meter.work(4)?;
        let ordinal = entry.entry.0 as usize;
        if policies[ordinal].is_none() {
            continue;
        }
        let Some(size) = entry.kernel.workgroup_size else {
            return Err(Error::Inconsistent(
                "selected tile root has no explicit launch geometry",
            ));
        };
        if size.x == 0
            || size.x > 256
            || size.y != 1
            || size.z != 1
            || geometry[ordinal].is_some_and(|previous| previous != size.x as u16)
        {
            return Err(Error::Inconsistent(
                "selected tile root has incompatible launch geometry",
            ));
        }
        geometry[ordinal] = Some(size.x as u16);
    }
    for function in inventory.functions() {
        meter.work(2)?;
        let ordinal = function.coordinate.0 as usize;
        if policies[ordinal].is_none() {
            continue;
        }
        if geometry[ordinal].is_none() {
            return Err(Error::Inconsistent(
                "selected tile function is not an actual kernel root",
            ));
        }
        for definition in &inventory.definitions()[function.definitions.clone()] {
            meter.work(1)?;
            if let Some(value) = definition.value {
                next_values[ordinal] =
                    next_values[ordinal].max(value.0.checked_add(1).ok_or(Resource::Arithmetic)?);
            }
        }
    }
    let (mut roles, _) = meter.table(inventory.definitions().len())?;
    for _ in inventory.definitions() {
        meter.push(&mut roles, None)?;
    }
    let (mut actions, _) = meter.table(inventory.operations().len())?;
    for _ in inventory.operations() {
        meter.push(&mut actions, Action::Retain)?;
    }
    // Definition-indexed passes deliberately do not use physical block order.
    for (index, row) in inventory.operations().iter().enumerate() {
        meter.work(4)?;
        let ordinal = row.coordinate.block.function.0 as usize;
        let Some(layout) = policies[ordinal] else {
            continue;
        };
        if let Kind::Execution(Execution::MaskedTileLoadU32 {
            input,
            base,
            lanes,
            elements,
            ..
        }) = row.operation.kind
        {
            if geometry[ordinal] != Some(lanes) {
                return Err(Error::Inconsistent(
                    "tile geometry differs from actual launch",
                ));
            }
            let schedule = ExecutionTileScheduleV1::new(layout, lanes, elements)
                .map_err(|_| Error::Inconsistent("tile geometry"))?;
            let recipe = ExecutionTileScalarLoweringV1::new(
                schedule,
                input,
                base,
                ValueId(next_values[ordinal]),
            )
            .map_err(|_| Error::Inconsistent("tile scalar SSA range"))?;
            next_values[ordinal] = recipe.next_value().0;
            if row.results.len() != 1 {
                return Err(Error::Inconsistent("tile result arity"));
            }
            roles[row.results.start] = Some(recipe);
            actions[index] = Action::Load(recipe);
        }
    }
    for (index, row) in inventory.operations().iter().enumerate() {
        meter.work(3)?;
        if policies[row.coordinate.block.function.0 as usize].is_none() {
            continue;
        }
        if matches!(
            row.operation.kind,
            Kind::Execution(Execution::TileIntoFragmentU32 { .. })
        ) {
            let incoming = inventory.uses()[row.operands.start].definition;
            let recipe = roles[incoming].ok_or(Error::Inconsistent(
                "fragment producer is not a selected tile",
            ))?;
            if row.results.len() != 1 {
                return Err(Error::Inconsistent("fragment result arity"));
            }
            roles[row.results.start] = Some(recipe);
            actions[index] = Action::Fragment;
        }
    }
    for (index, row) in inventory.operations().iter().enumerate() {
        meter.work(3)?;
        if policies[row.coordinate.block.function.0 as usize].is_none() {
            continue;
        }
        match row.operation.kind {
            Kind::Execution(Execution::FragmentIntoPartsU32 { .. }) => {
                let incoming = inventory.uses()[row.operands.start].definition;
                actions[index] = Action::Parts(roles[incoming].ok_or(Error::Inconsistent(
                    "parts producer is not a selected fragment",
                ))?);
            }
            Kind::Execution(Execution::ScopeEnd { .. }) => {
                actions[index] = Action::Scope;
            }
            _ => {}
        }
    }
    Ok(Plan { actions, roles })
}

fn output_count(action: Action, original: &Operation) -> usize {
    match action {
        Action::Load(recipe) => recipe.operation_count(),
        Action::Fragment => 0,
        Action::Parts(_) => original.results.len(),
        Action::Retain | Action::Scope => 1,
    }
}

fn part(
    recipe: ExecutionTileScalarLoweringV1,
    index: usize,
    count: usize,
) -> Result<(ValueId, ValueId)> {
    if count % 2 != 0 {
        return Err(Error::Inconsistent("parts result count"));
    }
    let elements = count / 2;
    let element = if index < elements {
        index
    } else {
        index - elements
    };
    let (value, mask) = recipe
        .component(u16::try_from(element).map_err(|_| Resource::Arithmetic)?)
        .ok_or(Error::Inconsistent("parts component geometry"))?;
    Ok((if index < elements { value } else { mask }, mask))
}

#[cfg(test)]
#[path = "tile_scalar_v18_tests.rs"]
mod tests;
