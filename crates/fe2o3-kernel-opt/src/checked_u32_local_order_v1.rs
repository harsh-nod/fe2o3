//! Closed diagnostic local scheduling on actual immutable V12 owners.
//!
//! This executes a bounded permutation and independently checks its exact
//! transition. It is not a persistent source recipe, a production policy,
//! source admission, a final machine order guarantee or artifact authority.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirTransitionErrorV1, check_canonical_kir_transition_receipt_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate, CanonicalKirTransitionReceiptErrorV1,
    InertCanonicalKirTransitionReceiptV1, Module, OperationKind, ScalarType, Type,
    VerifiedCanonicalKernelIrIdentityV12, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "checked_u32_local_order_v1_rows.rs"]
mod rows;

/// Two reviewed local preferences, never caller-selected passes or callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum U32LocalOrderPreferenceV1 {
    SourceOrder,
    /// Among ready operations, choose the greatest original region position.
    /// Original coordinates break ties only within this exact owner invocation.
    ReverseReady,
}

/// Inert exact-input selector; these ordinals are not cross-build source anchors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U32LocalOrderRegionV1 {
    pub expected_input: VerifiedCanonicalKernelIrIdentityV12,
    pub block: Block,
    pub first_operation: u32,
    pub operation_count: u32,
}

#[derive(Debug)]
pub enum CheckedU32LocalOrderErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Admission(CanonicalKernelIrReplayAdmissionErrorV12),
    Receipt(CanonicalKirTransitionReceiptErrorV1),
    Transition(CanonicalKirTransitionErrorV1),
    InputIdentity,
    RegionBounds,
    UnsupportedOperation,
    InconsistentOwner,
    ExactOutputMismatch,
}
impl From<Resource> for CheckedU32LocalOrderErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CheckedU32LocalOrderErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked u32 local order: {self:?}")
    }
}
impl std::error::Error for CheckedU32LocalOrderErrorV1 {}
type Error = CheckedU32LocalOrderErrorV1;
type Result<T> = std::result::Result<T, Error>;
const MAX_OPERATIONS: usize = 64;

/// Actual scheduled owner and checked transition, borrowing its exact original.
/// No public construction, mutation, Policy3 conversion or source authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedU32LocalOrderOutputV1;
/// fn detach<'a>(value: CheckedU32LocalOrderOutputV1<'a>)
///     -> CheckedU32LocalOrderOutputV1<'static> { value }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedU32LocalOrderOutputV1;
/// fn mutate(value: &mut CheckedU32LocalOrderOutputV1<'_>) {
///     value.output.module().functions.clear();
/// }
/// ```
pub struct CheckedU32LocalOrderOutputV1<'input> {
    input: &'input Owner,
    output: Owner,
    receipt: InertCanonicalKirTransitionReceiptV1,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    retained: usize,
}
impl<'input> CheckedU32LocalOrderOutputV1<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn transition_receipt(&self) -> &InertCanonicalKirTransitionReceiptV1 {
        &self.receipt
    }
    pub const fn region(&self) -> U32LocalOrderRegionV1 {
        self.region
    }
    pub const fn preference(&self) -> U32LocalOrderPreferenceV1 {
        self.preference
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Recompute this preference's permutation, check all unchanged payloads,
    /// then independently replay the existing receipt against both actual owners.
    /// Caller reserves this output and its borrowed input. No ledger is reset.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(budget, |budget| {
            let (input, input_storage) =
                Inventory::derive(self.input, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(input_storage.retained_storage())?;
            budget.reserve_storage(size_of::<Permutation>())?;
            let permutation = plan(&input, self.region, self.preference, budget)?;
            check_exact_output(self.input, &self.output, self.region, &permutation, budget)?;
            let (output, output_storage) =
                Inventory::derive(&self.output, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(output_storage.retained_storage())?;
            check_receipt(&input, &output, &self.receipt, budget)
        })
    }
}

/// Execute one of the two closed local preferences and independently replay it.
/// Only 2..=64 contiguous U32 AND/OR/XOR operations are eligible. Every other
/// payload stays identical, including exact SSA IDs, source references and CFG.
/// Work/storage charges accumulate before controlled work/allocation. All Result
/// exits restore entry storage; on success reserve `retained_storage()` before
/// further controlled allocation, with the original's reservation still live.
pub fn schedule_checked_u32_local_order_v1<'input>(
    input: &'input Owner,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    budget: &mut Budget<'_>,
) -> Result<CheckedU32LocalOrderOutputV1<'input>> {
    scoped(budget, |budget| schedule(input, region, preference, budget))
}

fn scoped<T>(budget: &mut Budget<'_>, f: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    let floor = budget.storage();
    let result = f(budget);
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

fn schedule<'input>(
    input: &'input Owner,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    budget: &mut Budget<'_>,
) -> Result<CheckedU32LocalOrderOutputV1<'input>> {
    budget.charge_work(1)?;
    let header = size_of::<CheckedU32LocalOrderOutputV1<'_>>()
        .checked_sub(size_of::<Owner>())
        .and_then(|n| n.checked_sub(size_of::<InertCanonicalKirTransitionReceiptV1>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header)?;
    let (inventory, inventory_storage) =
        Inventory::derive(input, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(inventory_storage.retained_storage())?;
    budget.reserve_storage(size_of::<Permutation>())?;
    let permutation = plan(&inventory, region, preference, budget)?;
    let (mut candidate, candidate_storage) = input
        .copy_module_for_transformation_v12(budget)
        .map_err(Error::Admission)?;
    budget.reserve_storage(candidate_storage.retained_storage())?;
    apply(
        &mut candidate,
        region,
        &permutation.source_for_output,
        budget,
    )?;
    let (output, output_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&candidate, budget)
            .map_err(Error::Admission)?;
    budget.reserve_storage(output_storage.retained_storage())?;
    drop(candidate);
    budget.release_storage(candidate_storage.retained_storage())?;
    check_exact_output(input, &output, region, &permutation, budget)?;
    let (out_inventory, out_storage) =
        Inventory::derive(&output, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(out_storage.retained_storage())?;
    let rows = rows::Rows::new(&inventory, &out_inventory, region, &permutation, budget)?;
    let (receipt, receipt_storage) =
        InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            &inventory.identity(),
            &out_inventory.identity(),
            rows.candidate(),
            budget,
        )
        .map_err(Error::Receipt)?;
    budget.reserve_storage(receipt_storage.retained_storage())?;
    check_receipt(&inventory, &out_inventory, &receipt, budget)?;
    let retained = header
        .checked_add(output_storage.retained_storage())
        .and_then(|n| n.checked_add(receipt_storage.retained_storage()))
        .ok_or(Resource::Arithmetic)?;
    Ok(CheckedU32LocalOrderOutputV1 {
        input,
        output,
        receipt,
        region,
        preference,
        retained,
    })
}

fn check_receipt(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    receipt: &InertCanonicalKirTransitionReceiptV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let storage = {
        let (_checked, storage) =
            check_canonical_kir_transition_receipt_v1(input, output, receipt, budget)
                .map_err(Error::Transition)?;
        budget.reserve_storage(storage.retained_storage())?;
        storage
    };
    budget.release_storage(storage.retained_storage())?;
    Ok(())
}

struct Permutation {
    source_for_output: [usize; MAX_OPERATIONS],
    output_for_source: [usize; MAX_OPERATIONS],
}

fn plan(
    inventory: &Inventory<'_>,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    budget: &mut Budget<'_>,
) -> Result<Permutation> {
    budget.charge_work(40)?;
    if inventory.identity() != region.expected_input {
        return Err(Error::InputIdentity);
    }
    if !(2..=MAX_OPERATIONS as u32).contains(&region.operation_count) {
        return Err(Error::RegionBounds);
    }
    budget.charge_work(inventory.blocks().len())?;
    let block = inventory
        .blocks()
        .iter()
        .find(|b| b.coordinate == region.block)
        .ok_or(Error::RegionBounds)?;
    let first = usize::try_from(region.first_operation).map_err(|_| Error::RegionBounds)?;
    let count = region.operation_count as usize;
    let end = first.checked_add(count).ok_or(Error::RegionBounds)?;
    if end > block.operations.len() {
        return Err(Error::RegionBounds);
    }
    budget.reserve_storage(size_of::<[u64; MAX_OPERATIONS]>())?;
    let result = plan_inner(
        inventory,
        region,
        block.operations.start + first,
        count,
        preference,
        budget,
    );
    budget.release_storage(size_of::<[u64; MAX_OPERATIONS]>())?;
    result
}

fn plan_inner(
    inventory: &Inventory<'_>,
    region: U32LocalOrderRegionV1,
    start: usize,
    count: usize,
    preference: U32LocalOrderPreferenceV1,
    budget: &mut Budget<'_>,
) -> Result<Permutation> {
    budget.charge_work(MAX_OPERATIONS * 3)?;
    let mut dependencies = [0_u64; MAX_OPERATIONS];
    let mut permutation = Permutation {
        source_for_output: [0; MAX_OPERATIONS],
        output_for_source: [0; MAX_OPERATIONS],
    };
    for (local, dependency) in dependencies.iter_mut().enumerate().take(count) {
        budget.charge_work(1)?;
        let row = &inventory.operations()[start + local];
        if !matches!(
            row.operation.kind,
            OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                ..
            }
        ) || !row.effects.is_empty()
            || !row.compiler_ordering().is_empty()
            || row.results.len() != 1
            || row.operands.len() != 2
            || inventory.definitions()[row.results.start].ty != &Type::Scalar(ScalarType::U32)
        {
            return Err(Error::UnsupportedOperation);
        }
        for operand in row.operands.clone() {
            budget.charge_work(1)?;
            let definition = &inventory.definitions()[inventory.uses()[operand].definition];
            if definition.ty != &Type::Scalar(ScalarType::U32) {
                return Err(Error::UnsupportedOperation);
            }
            if let Definition::Result { operation, .. } = definition.coordinate
                && operation.block == region.block
                && operation.operation >= region.first_operation
            {
                let index = (operation.operation - region.first_operation) as usize;
                if index < count {
                    *dependency |= 1_u64 << index;
                }
            }
        }
    }
    let mut completed = 0_u64;
    for output in 0..count {
        let mut chosen = None;
        for (source, dependency) in dependencies.iter().enumerate().take(count) {
            budget.charge_work(1)?;
            if completed & (1_u64 << source) == 0 && dependency & !completed == 0 {
                chosen = Some(source);
                if preference == U32LocalOrderPreferenceV1::SourceOrder {
                    break;
                }
            }
        }
        let source = chosen.ok_or(Error::InconsistentOwner)?;
        permutation.source_for_output[output] = source;
        permutation.output_for_source[source] = output;
        completed |= 1_u64 << source;
    }
    Ok(permutation)
}

fn apply(
    module: &mut Module,
    region: U32LocalOrderRegionV1,
    source_for_output: &[usize; MAX_OPERATIONS],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(3 + MAX_OPERATIONS)?;
    let count = region.operation_count as usize;
    if !(2..=MAX_OPERATIONS).contains(&count) {
        return Err(Error::RegionBounds);
    }
    let block = module
        .functions
        .get_mut(region.block.function.0 as usize)
        .and_then(|f| f.body.as_mut())
        .and_then(|b| b.blocks.get_mut(region.block.block as usize))
        .ok_or(Error::RegionBounds)?;
    let first = region.first_operation as usize;
    let end = first.checked_add(count).ok_or(Error::RegionBounds)?;
    let operations = block
        .operations
        .get_mut(first..end)
        .ok_or(Error::RegionBounds)?;
    budget.reserve_storage(size_of::<[usize; MAX_OPERATIONS]>())?;
    let result = (|| {
        let mut current = std::array::from_fn::<_, MAX_OPERATIONS, _>(|i| i);
        for (output, expected_source) in source_for_output.iter().enumerate().take(count) {
            let mut position = None;
            for (index, source) in current.iter().enumerate().take(count).skip(output) {
                budget.charge_work(1)?;
                if source == expected_source {
                    position = Some(index);
                    break;
                }
            }
            let position = position.ok_or(Error::InconsistentOwner)?;
            budget.charge_work(2)?;
            operations.swap(output, position);
            current.swap(output, position);
        }
        Ok(())
    })();
    budget.release_storage(size_of::<[usize; MAX_OPERATIONS]>())?;
    result
}

// A bounded inverse permutation plus complete Module equality checks every
// unselected field and exact authored SSA/source payload, not just graph hashes.
fn check_exact_output(
    input: &Owner,
    output: &Owner,
    region: U32LocalOrderRegionV1,
    permutation: &Permutation,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(budget, |budget| {
        let (mut restored, storage) = output
            .copy_module_for_transformation_v12(budget)
            .map_err(Error::Admission)?;
        budget.reserve_storage(storage.retained_storage())?;
        apply(
            &mut restored,
            region,
            &permutation.output_for_source,
            budget,
        )?;
        budget.charge_work(
            input
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(output.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if &restored != input.module() {
            return Err(Error::ExactOutputMismatch);
        }
        Ok(())
    })
}

fn map_operation(
    mut coordinate: OperationCoordinate,
    region: U32LocalOrderRegionV1,
    order: &[usize; MAX_OPERATIONS],
) -> OperationCoordinate {
    if coordinate.block == region.block && coordinate.operation >= region.first_operation {
        let index = (coordinate.operation - region.first_operation) as usize;
        if index < region.operation_count as usize {
            coordinate.operation = region.first_operation + order[index] as u32;
        }
    }
    coordinate
}

#[cfg(test)]
#[path = "checked_u32_local_order_v1_tests.rs"]
mod tests;
