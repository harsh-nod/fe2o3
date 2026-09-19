// A complete-empty effect decision is deliberately not consulted. The fresh
// private chain, sealed source relation, and closed total operation grammar are
// separate obligations, all retained under the same original owner.

fn unit_deletion_core_error_v1(
    error: fe2o3_kernel_ir::LocalFrameErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_kernel_ir::LocalFrameErrorV1::Resource(error) => error.into(),
        fe2o3_kernel_ir::LocalFrameErrorV1::Unsupported { .. } => unit_deletion_refused_v1(
            "Unit-local deletion requires a total initialized private helper",
        ),
    }
}

fn unit_deletion_core_mismatch_v1(physical: usize) -> fe2o3_kernel_ir::LocalFrameErrorV1 {
    fe2o3_kernel_ir::LocalFrameErrorV1::Unsupported {
        function_ordinal: physical,
        operation: None,
        reason: fe2o3_kernel_ir::LocalFrameRefusalReasonV1::Operation,
    }
}

fn check_unit_deletion_source_silence_v1(
    stage: &ProductionUnitLocalRankedStageV1<'_>,
    certified: &mut [Option<u32>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::LocalFrameControlKindV1 as Core;
    let rows = stage.source.rows;
    let owner = stage.owner;
    let semantic = owner.semantic_ssa.source_semantic();
    for body in &rows.bodies {
        budget.charge_work(9)?;
        let physical = body.physical;
        let Some(RetainedHelperKindV1::Local {
            allocations,
            accesses,
            control,
            edge_bindings,
        }) = owner.helper_memory.functions.get(physical)
        else {
            return Err(unit_local_mismatch_v1());
        };
        if *allocations != body.allocations
            || *accesses != body.accesses
            || *control != body.control
            || *edge_bindings != body.edge_bindings
            || certified
                .get_mut(physical)
                .ok_or_else(unit_local_mismatch_v1)?
                .replace(u32::MAX)
                .is_some()
        {
            return Err(unit_local_mismatch_v1());
        }
        fe2o3_kernel_ir::with_checked_local_frame_chain_function_v1(
            owner.executable().verified_module_ref_v1(),
            physical,
            budget,
            |fresh, budget| {
                budget.charge_work(10)?;
                let failure = || unit_deletion_core_mismatch_v1(physical);
                if !std::ptr::eq(fresh.module(), owner.executable().module())
                    || fresh.function_ordinal() != physical
                    || !std::ptr::eq(
                        fresh.function(),
                        &owner.executable().module().functions[physical],
                    )
                    || fresh.function().role != fe2o3_kernel_ir::FunctionRole::InternalHelper
                    || !fresh.function().signature.parameters.is_empty()
                    || !fresh.function().signature.results.is_empty()
                {
                    return Err(failure());
                }
                let allocation_rows = fresh.allocations(budget)?;
                let access_rows = fresh.accesses(budget)?;
                let control_rows = fresh.control(budget)?;
                let binding_rows = fresh.edge_bindings(budget)?;
                // These are fixed-field records. Both sides are traversed once;
                // no graph payload is cloned into a new summary.
                budget.charge_work(argument_sum_v1(&[
                    argument_product_v1(
                        allocation_rows.len(),
                        std::mem::size_of::<RetainedLocalAllocationV1>(),
                    )?,
                    argument_product_v1(
                        access_rows.len(),
                        std::mem::size_of::<RetainedLocalAccessV1>(),
                    )?,
                    argument_product_v1(
                        control_rows.len(),
                        std::mem::size_of::<RetainedLocalControlV1>(),
                    )?,
                    argument_product_v1(
                        binding_rows.len(),
                        std::mem::size_of::<RetainedLocalEdgeBindingV1>(),
                    )?,
                ])?)?;
                if owner
                    .helper_memory
                    .allocations
                    .get(allocations.0..allocations.1)
                    != Some(allocation_rows)
                    || owner.helper_memory.accesses.get(accesses.0..accesses.1) != Some(access_rows)
                    || owner.helper_memory.control.get(control.0..control.1) != Some(control_rows)
                    || owner
                        .helper_memory
                        .edge_bindings
                        .get(edge_bindings.0..edge_bindings.1)
                        != Some(binding_rows)
                {
                    return Err(failure());
                }
                let mut returns = 0usize;
                for control in control_rows {
                    budget.charge_work(4)?;
                    if control.kind() == Core::Return {
                        returns = returns
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                    let block = stage
                        .source
                        .inventory
                        .block_for_id(
                            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                u32::try_from(physical)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            control.block(),
                            budget,
                        )
                        .map_err(|error| match error {
                            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(
                                error,
                            ) => fe2o3_kernel_ir::LocalFrameErrorV1::Resource(error),
                            _ => failure(),
                        })?
                        .ok_or_else(failure)?;
                    if control.kind() == Core::InactiveTrap {
                        // The fresh chain has checked the singleton canonical
                        // trap and independently proved its edge is inactive.
                        continue;
                    }
                    for operation in &block.block.operations {
                        budget.charge_work(2)?;
                        if !matches!(
                            operation.kind,
                            OperationKind::Constant(
                                Constant::Bool(_)
                                    | Constant::U8(_)
                                    | Constant::U16(_)
                                    | Constant::U32(_)
                                    | Constant::U64(_)
                                    | Constant::Index(_)
                            ) | OperationKind::Cast { .. }
                                | OperationKind::Compare { .. }
                                | OperationKind::Alloca { .. }
                                | OperationKind::GetElementPointer { .. }
                                | OperationKind::Load { .. }
                                | OperationKind::Store { .. }
                        ) {
                            return Err(failure());
                        }
                    }
                }
                if returns != 1 {
                    return Err(failure());
                }
                Ok(())
            },
        )
        .map_err(unit_deletion_core_error_v1)?;
    }
    for (ordinal, association) in rows.associations.iter().enumerate() {
        budget.charge_work(12)?;
        let declaration = semantic
            .functions()
            .get(association.key.function.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        let physical = owner
            .executable()
            .module()
            .functions
            .get(association.key.physical)
            .ok_or_else(unit_local_mismatch_v1)?;
        if certified.get(association.key.physical) != Some(&Some(u32::MAX))
            || declaration.identity() != association.source_identity
            || declaration.abi().canon_abi()
                != fe2o3_mir_model::semantic_mir_v1::SemanticCanonAbiV1::Rust
            || declaration.abi().extern_abi()
                != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
            || association.call_count == 0
        {
            return Err(unit_local_mismatch_v1());
        }
        let (unit_local, unit_type) = check_unit_local_unit_signature_v1(
            association.key.function,
            semantic.types(),
            declaration,
            physical,
            budget,
        )?;
        if unit_local != association.unit_return_local || unit_type != association.unit_type {
            return Err(unit_local_mismatch_v1());
        }
        let values = rows
            .values
            .get(association.values.0..association.values.1)
            .ok_or_else(unit_local_mismatch_v1)?;
        for value in values {
            budget.charge_work(5)?;
            if value.key != association.key {
                return Err(unit_local_mismatch_v1());
            }
            match value.recipe {
                UnitLocalValueRecipeV1::Unit
                | UnitLocalValueRecipeV1::Literal { .. }
                | UnitLocalValueRecipeV1::Copy { .. }
                | UnitLocalValueRecipeV1::Compare { .. }
                | UnitLocalValueRecipeV1::ArrayLength { .. }
                | UnitLocalValueRecipeV1::Load { .. }
                | UnitLocalValueRecipeV1::Edge { .. }
                | UnitLocalValueRecipeV1::Cast {
                    kind: fe2o3_mir_model::semantic_mir_v1::SemanticCastKindV1::Integer,
                    ..
                } => {}
                _ => {
                    return Err(unit_deletion_refused_v1(
                        "Unit-local deletion has an unsupported source value recipe",
                    ));
                }
            }
        }
        let controls = rows
            .control
            .get(association.control.0..association.control.1)
            .ok_or_else(unit_local_mismatch_v1)?;
        let mut returns = 0usize;
        for (offset, control) in controls.iter().enumerate() {
            budget.charge_work(9)?;
            let native = owner
                .helper_memory
                .control
                .get(control.physical_control)
                .ok_or_else(unit_local_mismatch_v1)?;
            if control.association != ordinal
                || native.function_ordinal() != association.key.physical
                || native.block() != control.physical_block
            {
                return Err(unit_local_mismatch_v1());
            }
            match control.kind {
                UnitLocalControlKindV1::Goto {
                    physical_target, ..
                } => {
                    if native.kind()
                        != (Core::Branch {
                            target: physical_target,
                        })
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                }
                UnitLocalControlKindV1::Assert {
                    predicate,
                    expected,
                    selected_successor,
                    physical_target,
                    inactive_block,
                    ..
                } => {
                    let value = rows
                        .values
                        .get(predicate)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let UnitLocalNativeValueV1::Scalar {
                        value: condition,
                        scalar: ScalarType::Bool,
                        ..
                    } = value.native
                    else {
                        return Err(unit_local_mismatch_v1());
                    };
                    if value.key != association.key
                        || value.known_bits != Some(u64::from(expected))
                        || native.kind()
                            != (Core::Selected {
                                condition,
                                value: expected,
                                successor: u8::try_from(selected_successor)
                                    .map_err(|_| unit_local_mismatch_v1())?,
                                target: physical_target,
                                inactive: inactive_block,
                            })
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                }
                UnitLocalControlKindV1::Return {
                    local,
                    unit_type: returned_type,
                    unit_value,
                } => {
                    let value = rows
                        .values
                        .get(unit_value)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    if native.kind() != Core::Return
                        || local != unit_local
                        || returned_type != unit_type
                        || value.key != association.key
                        || value.source_type != unit_type
                        || !matches!(value.recipe, UnitLocalValueRecipeV1::Unit)
                        || !matches!(value.native, UnitLocalNativeValueV1::IgnoredUnit)
                        || association.return_control
                            != argument_sum_v1(&[association.control.0, offset])?
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    returns = argument_sum_v1(&[returns, 1])?;
                }
                UnitLocalControlKindV1::InactiveTrap { .. } => {
                    if native.kind() != Core::InactiveTrap || control.source_block.is_some() {
                        return Err(unit_local_mismatch_v1());
                    }
                }
            }
        }
        if returns != 1 {
            return Err(unit_local_mismatch_v1());
        }
    }
    Ok(())
}
