use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{CanonicalKirOperationCoordinateV1, CanonicalKirOperationOriginV1};

#[path = "production_checked_output_general_census_policy3_v1.rs"]
mod census;
#[path = "production_checked_output_exp_v1.rs"]
mod exp;
#[path = "production_checked_output_numeric_casts_v1.rs"]
mod numeric_casts;
#[path = "production_checked_output_private_memory_policy3_v1.rs"]
mod private_memory;
#[path = "production_checked_output_scalar_helpers_v1.rs"]
mod scalar_helpers;
#[path = "production_checked_output_source_roles_v1.rs"]
mod source_roles;
#[path = "production_checked_output_unsigned_division_v1.rs"]
mod unsigned_division;

#[cfg(test)]
#[path = "production_checked_output_general_arithmetic_census_v1_tests.rs"]
mod arithmetic_census_tests;

type E = ProductionCheckedOutputAdmissionErrorPolicy3V1;
type R<T> = Result<T, E>;

include!("production_checked_output_general_source_context_v1.rs");
include!("production_checked_output_erased_general_v1.rs");

fn inventory_error(error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1) -> E {
    E::SourceOutput(ProductionSourceOutputErrorV1::Inventory(error))
}
fn transition_error(error: fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1) -> E {
    E::SourceOutput(ProductionSourceOutputErrorV1::Transition(error))
}
fn arithmetic() -> E {
    E::Resource(AssertOriginResourceV1::Arithmetic)
}
fn refused(phase: &'static str, detail: &'static str) -> E {
    output_admission_unsupported_v1(phase, detail)
}
fn charge(budget: &mut AssertOriginBudgetV1<'_>, amount: usize) -> R<()> {
    output_admission_charge_v1(budget, amount)
}

// Only this transaction releases scratch. Every borrowed index and vector is
// dropped before cleanup, and a foreign Work ledger is never debited.
pub(super) fn check_general_output_v1(
    source: &ProductionSemanticKirOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_general_context_v1(
        GeneralSourceContextV1::Direct(source),
        bound,
        checked,
        budget,
    )
}

fn check_general_context_v1(
    source: GeneralSourceContextV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    charge(budget, 3)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_inner(source, bound, checked, budget)
    }));
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        return Err(E::Resource(AssertOriginResourceV1::Accounting));
    }
    budget
        .release_storage(budget.storage() - floor)
        .map_err(E::Resource)?;
    match result {
        Ok(result) => result,
        Err(_) => Err(E::SourceOutput(ProductionSourceOutputErrorV1::Panicked)),
    }
}

fn check_inner(
    source: GeneralSourceContextV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    source.replay_and_census(budget)?;
    charge(budget, 3)?;
    let original = source.neutral()?;
    let origins = source.origins()?;
    source.ranked(budget)?;
    let (coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            original, bound, budget,
        )
        .map_err(E::Coordinates)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    let bytes = bound.canonical().canonical_bytes();
    charge(budget, bytes.len().checked_add(1).ok_or_else(arithmetic)?)?;
    if bytes != checked.native_input_audit_bytes() {
        return Err(E::SourceOutput(ProductionSourceOutputErrorV1::InputCustody));
    }
    let (input, storage) =
        CanonicalKirInventoryV1::derive(bound, budget).map_err(inventory_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    let (output, storage) =
        CanonicalKirInventoryV1::derive(checked.owner(), budget).map_err(inventory_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    let candidate = checked.occurrences().candidate();
    let (transition, storage) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
        &input, &output, candidate, budget,
    )
    .map_err(transition_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    let (control, storage) =
        fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&transition, budget)
            .map_err(transition_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    let source = match source {
        GeneralSourceContextV1::Direct(source) => source,
        GeneralSourceContextV1::Erased(source) => {
            return check_erased_source_outputs_v1(
                source,
                &coordinates,
                checked,
                &transition,
                &control,
                &input,
                &output,
                budget,
            );
        }
    };
    let (_assertions, storage) =
        source_output_assertion_transport_v1(origins, &coordinates, &control, budget)
            .map_err(|error| E::SourceOutput(ProductionSourceOutputErrorV1::Assertion(error)))?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    // Transport independently checks both branches, their argument uses and
    // the exact original predicate. Conditional is not a proof of success.
    for row in &source.correspondence.blocks {
        charge(budget, 1)?;
        let function =
            assert_origin_find_v1(&origins.origins.functions, budget, |entry, budget| {
                budget.charge_work(2)?;
                Ok((entry.owner, entry.function)
                    .cmp(&(row.correspondence_owner, row.semantic_function)))
            })
            .map_err(|e| E::SourceOutput(ProductionSourceOutputErrorV1::SourceOrigin(e)))?
            .ok_or_else(|| refused("source", "complete root-qualified block mapping"))?;
        let physical = input
            .block_for_id(
                origins.origins.functions[function].canonical,
                row.kernel_ir_block,
                budget,
            )
            .map_err(inventory_error)?
            .ok_or_else(|| refused("N", "exact source block"))?;
        control
            .block(physical.coordinate, budget)
            .map_err(transition_error)?;
    }
    let mut traps = scratch::<u8>(input.operations().len(), budget)?;
    charge(budget, input.operations().len())?;
    traps.resize(input.operations().len(), 0);
    for span in source.correspondence.synthetic_operation_spans() {
        charge(budget, 2)?;
        if span.rule() != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap {
            continue;
        }
        let function =
            assert_origin_find_v1(&origins.origins.functions, budget, |entry, budget| {
                budget.charge_work(2)?;
                Ok((entry.owner, entry.function)
                    .cmp(&(span.correspondence_owner(), span.semantic_function())))
            })
            .map_err(|e| E::SourceOutput(ProductionSourceOutputErrorV1::SourceOrigin(e)))?
            .ok_or_else(|| refused("N", "exact assertion trap function"))?;
        let block = input
            .block_for_id(
                origins.origins.functions[function].canonical,
                span.kernel_ir_block(),
                budget,
            )
            .map_err(inventory_error)?
            .ok_or_else(|| refused("N", "exact assertion trap block"))?;
        if span.operation_count() != 1 {
            return Err(refused("N", "one exact synthetic trap"));
        }
        let ordinal = block
            .operations
            .start
            .checked_add(span.first_operation_ordinal() as usize)
            .ok_or_else(arithmetic)?;
        if ordinal >= block.operations.end || traps[ordinal] != 0 {
            return Err(refused("N", "unique exact synthetic trap"));
        }
        traps[ordinal] = 1;
    }
    let private_input = private_memory::check(&input, source.limits.max_operations, budget)?;
    private_memory::source_lifetimes(source, &private_input, budget)?;
    check_native_outputs_v1(
        GeneralSourceContextV1::Direct(source),
        &input,
        &output,
        checked,
        &private_input,
        &traps,
        budget,
    )
}

fn check_native_outputs_v1(
    source: GeneralSourceContextV1<'_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    private_input: &private_memory::PrivateMemory<'_, '_>,
    traps: &[u8],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    let candidate = checked.occurrences().candidate();
    let private_output = private_memory::check(output, source.limits().max_operations, budget)?;
    let target = source.semantic().target();
    let input_division = unsigned_division::check(input, target, budget)?;
    let output_division = unsigned_division::check(output, target, budget)?;
    let input_helpers = scalar_helpers::check(input, budget)?;
    let output_helpers = scalar_helpers::check(output, budget)?;
    census::native(
        input,
        private_input,
        &input_division,
        &input_helpers,
        "B",
        |ordinal, _| Ok(traps[ordinal] == 1),
        budget,
    )?;
    census::native(
        output,
        &private_output,
        &output_division,
        &output_helpers,
        "O",
        |ordinal, coordinate| {
            let row = candidate
                .operations
                .get(ordinal)
                .ok_or_else(|| refused("O", "operation census"))?;
            if row.output != coordinate {
                return Err(refused("O", "operation coordinate"));
            }
            match row.origin {
                CanonicalKirOperationOriginV1::Retained(origin) => {
                    Ok(traps[operation_ordinal(input, origin)?] == 1)
                }
                CanonicalKirOperationOriginV1::ConstantFrom(_) => Ok(false),
            }
        },
        budget,
    )?;
    let kernels = derive_checked_output_guarded_obligations_v1(
        checked.owner(),
        source.limits().max_operations,
    )
    .map_err(E::Formal)?;
    census::formal(output, &private_output, &kernels, budget)?;
    Ok(kernels)
}

fn operation_ordinal(
    inventory: &CanonicalKirInventoryV1<'_>,
    coordinate: CanonicalKirOperationCoordinateV1,
) -> R<usize> {
    let function = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
        .ok_or_else(|| refused("transition", "function coordinate"))?;
    let block_index = function
        .blocks
        .start
        .checked_add(coordinate.block.block as usize)
        .ok_or_else(arithmetic)?;
    if block_index >= function.blocks.end {
        return Err(refused("transition", "block coordinate"));
    }
    let block = &inventory.blocks()[block_index];
    let operation = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .ok_or_else(arithmetic)?;
    if operation >= block.operations.end {
        return Err(refused("transition", "operation coordinate"));
    }
    Ok(operation)
}

/// The caller must first qualify source/B/C and independently replay C/O's
/// coordinate-preserving forwarding relation. This rederives O's physical
/// safety facts; it is not a substitute for either semantic prerequisite.
pub(crate) fn check_forwarded_output_v1(
    source: &ProductionSemanticKirOwnerV1,
    qualified_intermediate: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_forwarded_context_v1(
        GeneralSourceContextV1::Direct(source),
        qualified_intermediate,
        output,
        budget,
    )
}

fn check_forwarded_context_v1(
    source: GeneralSourceContextV1<'_>,
    qualified_intermediate: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    charge(budget, 3)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (input, storage) = CanonicalKirInventoryV1::derive(qualified_intermediate, budget)
            .map_err(inventory_error)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(E::Resource)?;
        let (actual, storage) =
            CanonicalKirInventoryV1::derive(output, budget).map_err(inventory_error)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(E::Resource)?;
        let private = private_memory::check(&actual, source.limits().max_operations, budget)?;
        let division = unsigned_division::check(&actual, source.semantic().target(), budget)?;
        let helpers = scalar_helpers::check(&actual, budget)?;
        census::native(
            &actual,
            &private,
            &division,
            &helpers,
            "forwarded O",
            |_, coordinate| {
                let old = &input.operations()[operation_ordinal(&input, coordinate)?];
                let new = &actual.operations()[operation_ordinal(&actual, coordinate)?];
                // Only the private consumer of an already-replayed C/O relation
                // may use C's prior source-authorized trap at the same coordinate.
                Ok(old.coordinate == coordinate && old.operation == new.operation)
            },
            budget,
        )?;
        let reports =
            derive_checked_output_guarded_obligations_v1(output, source.limits().max_operations)
                .map_err(E::Formal)?;
        census::formal(&actual, &private, &reports, budget)?;
        Ok(reports)
    }));
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        return Err(E::Resource(AssertOriginResourceV1::Accounting));
    }
    budget
        .release_storage(budget.storage() - floor)
        .map_err(E::Resource)?;
    match result {
        Ok(result) => result,
        Err(_) => Err(E::SourceOutput(ProductionSourceOutputErrorV1::Panicked)),
    }
}

// Actual capacity is reconciled before any push. Headers are scratch too;
// legacy inventory receipts retain their documented logical-payload boundary.
fn scratch<T>(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> R<Vec<T>> {
    charge(budget, 6)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(arithmetic)?;
    let reserve = bytes
        .checked_add(std::mem::size_of::<Vec<T>>())
        .ok_or_else(arithmetic)?;
    budget.reserve_storage(reserve).map_err(E::Resource)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| E::Resource(AssertOriginResourceV1::Allocation))?;
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(arithmetic)?;
    budget
        .reserve_storage(
            actual
                .checked_sub(bytes)
                .ok_or(E::Resource(AssertOriginResourceV1::Accounting))?,
        )
        .map_err(E::Resource)?;
    Ok(rows)
}
