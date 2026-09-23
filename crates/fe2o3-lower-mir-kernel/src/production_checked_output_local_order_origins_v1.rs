//! Exact replayed source-origin transport. Numeric coordinates alone never
//! establish retention, operand equivalence, or source/HIR authentication.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, ScalarType, Type, ValueId};
type Coordinate = CanonicalKirOperationCoordinateV1;

pub(super) fn resolve(
    prefix: &ProductionCheckedOutputOwnerPolicy6V1,
    request: SourceU32LocalOrderRequestV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<U32LocalOrderRegionV1> {
    budget.charge_work(24)?;
    let source = GeneralSourceContextV1::Direct(prefix.source_semantic_kir());
    let neutral = source.neutral()?;
    if neutral.canonical().identity() != &request.expected_source {
        return Err(refused("local-order source", "exact original N identity").into());
    }
    profile(prefix, budget)?;
    let (_coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            neutral,
            prefix.bound(),
            budget,
        )
        .map_err(E::Coordinates)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (n, storage) = CanonicalKirInventoryV1::derive(neutral, budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let n_inputs = diamond(&n, request.operations, budget)?;
    let (b, storage) =
        CanonicalKirInventoryV1::derive(prefix.bound(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    // Target binding must preserve the exact selected u32 parameter roles too.
    if diamond(&b, request.operations, budget)? != n_inputs {
        return Err(refused("local-order B", "unchanged source formal roles").into());
    }
    let p5 = prefix.checked_output().intermediate_policy5();
    let p3 = p5.intermediate_policy4().intermediate_policy3();
    let (canonical, storage) =
        CanonicalKirInventoryV1::derive(p3.owner(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let selected = retained(
        &b,
        &canonical,
        request.operations,
        p3.occurrences().candidate().operations,
        budget,
    )?;
    let (forwarded, storage) =
        CanonicalKirInventoryV1::derive(p5.owner(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    coordinate_roster(&canonical, &forwarded, budget)?;
    let (integer, storage) =
        CanonicalKirInventoryV1::derive(prefix.output(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let selected = retained(
        &forwarded,
        &integer,
        selected,
        prefix
            .checked_output()
            .continuation()
            .occurrences()
            .candidate()
            .operations,
        budget,
    )?;
    if diamond(&integer, selected, budget)? != n_inputs {
        return Err(refused("local-order I", "unchanged source formal roles").into());
    }
    binding.check(budget)?;
    Ok(U32LocalOrderRegionV1 {
        expected_input: *prefix.output().canonical().identity(),
        block: selected[0].block,
        first_operation: selected[0].operation,
        operation_count: 3,
    })
}

fn profile(
    prefix: &ProductionCheckedOutputOwnerPolicy6V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<()> {
    budget.charge_work(24)?;
    let semantic = prefix.source_semantic_kir().semantic().semantic();
    let [root] = semantic.roots() else {
        return Err(refused("local-order profile", "one Direct RawEmpty root").into());
    };
    let selected = semantic
        .select_kernel_body_v1()
        .ok_or_else(|| refused("local-order profile", "direct source body"))?;
    let [function] = semantic.functions() else {
        return Err(refused("local-order profile", "one direct semantic function").into());
    };
    let launch = function
        .kernel_entry()
        .and_then(|entry| entry.source_contract().launch())
        .ok_or_else(|| refused("local-order profile", "source launch contract"))?;
    if selected.root() != *root
        || selected.body() != *root
        || launch.required().map(|x| x.as_array()) != Some([64, 1, 1])
        || launch.maximum().map(|x| x.as_array()) != Some([64, 1, 1])
    {
        return Err(refused("local-order profile", "direct source launch64/max64").into());
    }
    // Direct Policy6 admission already refuses UnitLocal before attachment.
    // The compiler-facing entry owns target/xnack/wave/Instance authentication.
    for module in [
        prefix.source_semantic_kir().module(),
        prefix.bound().module(),
        prefix.output().module(),
    ] {
        module_profile(module, budget)?;
    }
    Ok(())
}

// The single source body may require the one canonical assertion declaration.
// This helper is called only on the retained verified N/B/I owners, whose V12
// admission checks the reserved declaration's exact signature and capabilities.
// Fresh native census independently requires each use to be a transported trap.
pub(super) fn module_profile(
    module: &fe2o3_kernel_ir::Module,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<()> {
    use fe2o3_kernel_ir::{AmdGpuDiagnosticOperation, FunctionRole, WorkgroupSize};
    budget.charge_work(12)?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(refused("local-order profile", "one kernel").into());
    };
    let Some((function, declarations)) = module.functions.split_first() else {
        return Err(refused("local-order profile", "one direct function").into());
    };
    budget.charge_work(
        kernel
            .entry
            .as_str()
            .len()
            .checked_add(function.id.as_str().len())
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    if function.role != FunctionRole::KernelEntry
        || function.body.is_none()
        || kernel.entry != function.id
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || declarations.len() > 1
    {
        return Err(refused(
            "local-order profile",
            "one direct entry and canonical trap only",
        )
        .into());
    }
    for declaration in declarations {
        budget.charge_work(
            declaration
                .id
                .as_str()
                .len()
                .checked_add(2)
                .and_then(|n| n.checked_mul(8))
                .and_then(|n| n.checked_add(8))
                .ok_or(AssertOriginResourceV1::Arithmetic)?,
        )?;
        if declaration.role != FunctionRole::ExternalImport
            || declaration.body.is_some()
            || !declaration.signature.parameters.is_empty()
            || !declaration.signature.results.is_empty()
            || !matches!(
                AmdGpuDiagnosticOperation::from_intrinsic_call(&declaration.id, &[]),
                Some(AmdGpuDiagnosticOperation::Trap)
            )
        {
            return Err(refused(
                "local-order profile",
                "canonical assertion trap declaration only",
            )
            .into());
        }
    }
    Ok(())
}

fn coordinate_roster(
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<()> {
    if input.operations().len() != output.operations().len() {
        return Err(refused("local-order P3/P5", "coordinate-preserving complete roster").into());
    }
    for (a, b) in input.operations().iter().zip(output.operations()) {
        budget.charge_work(8)?;
        if a.coordinate != b.coordinate {
            return Err(
                refused("local-order P3/P5", "coordinate-preserving complete roster").into(),
            );
        }
    }
    Ok(())
}

// Prefix replay is mandatory before this private map is consulted. One
// selected retained origin must have exactly one actual output descendant.
pub(super) fn retained(
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    selected: [Coordinate; 3],
    rows: &[fe2o3_kernel_ir::CanonicalKirOperationTransitionV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<[Coordinate; 3]> {
    budget.charge_work(24)?;
    for coordinate in selected {
        operation_ordinal(input, coordinate)?;
    }
    if rows.len() != output.operations().len() {
        return Err(refused("local-order anchors", "complete transition roster").into());
    }
    let mut found = [None; 3];
    for (row, actual) in rows.iter().zip(output.operations()) {
        budget.charge_work(24)?;
        if row.output != actual.coordinate {
            return Err(refused("local-order anchors", "actual output coordinate").into());
        }
        if let CanonicalKirOperationOriginV1::Retained(origin) = row.origin {
            for (index, coordinate) in selected.iter().enumerate() {
                if origin == *coordinate && found[index].replace(row.output).is_some() {
                    return Err(refused("local-order anchors", "unique retained descendant").into());
                }
            }
        }
    }
    let [Some(a), Some(b), Some(c)] = found else {
        return Err(refused("local-order anchors", "all source operators retained").into());
    };
    Ok([a, b, c])
}

// Return formal parameter ordinals, not raw IDs; full prefix replay supplies
// global equivalence, while this check preserves the selected source graph.
pub(super) fn diamond(
    inventory: &CanonicalKirInventoryV1<'_>,
    selected: [Coordinate; 3],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<[usize; 4]> {
    budget.charge_work(80)?;
    if selected[0].block.function.0 != 0
        || selected[0].block.block != 0
        || selected.iter().any(|x| x.block != selected[0].block)
        || selected[0].operation.checked_add(1) != Some(selected[1].operation)
        || selected[1].operation.checked_add(1) != Some(selected[2].operation)
    {
        return Err(refused(
            "local-order diamond",
            "contiguous source-ordered entry region",
        )
        .into());
    }
    let function = inventory
        .functions()
        .first()
        .ok_or_else(|| refused("local-order diamond", "one function"))?
        .function;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| refused("local-order diamond", "definition"))?;
    let mut inputs = [ValueId(0); 4];
    let mut results = [ValueId(0); 3];
    for (index, coordinate) in selected.iter().enumerate() {
        let operation =
            inventory.operations()[operation_ordinal(inventory, *coordinate)?].operation;
        let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
            return Err(refused("local-order diamond", "u32 XOR/OR/AND only").into());
        };
        let [result] = operation.results.as_slice() else {
            return Err(refused("local-order diamond", "one result").into());
        };
        let expected = [BinaryOp::BitXor, BinaryOp::BitOr, BinaryOp::BitAnd][index];
        if op != expected
            || result.ty != Type::Scalar(ScalarType::U32)
            || (index == 2 && (lhs != results[0] || rhs != results[1]))
        {
            return Err(refused("local-order diamond", "exact u32 source operand graph").into());
        }
        results[index] = result.id;
        if index < 2 {
            inputs[index * 2] = lhs;
            inputs[index * 2 + 1] = rhs;
        }
    }
    let all = [
        inputs[0], inputs[1], inputs[2], inputs[3], results[0], results[1], results[2],
    ];
    for (index, value) in all.iter().enumerate() {
        if all[index + 1..].contains(value) {
            return Err(refused("local-order diamond", "seven distinct SSA roles").into());
        }
    }
    let mut arguments = [0; 4];
    for (index, value) in inputs.iter().enumerate() {
        let mut found = None;
        for (ordinal, parameter) in body.parameters.iter().enumerate() {
            budget.charge_work(6)?;
            if parameter == value
                && (found.replace(ordinal).is_some()
                    || function.signature.parameters.get(ordinal)
                        != Some(&Type::Scalar(ScalarType::U32)))
            {
                return Err(
                    refused("local-order diamond", "distinct u32 formal parameters").into(),
                );
            }
        }
        arguments[index] =
            found.ok_or_else(|| refused("local-order diamond", "formal parameter input"))?;
    }
    Ok(arguments)
}
