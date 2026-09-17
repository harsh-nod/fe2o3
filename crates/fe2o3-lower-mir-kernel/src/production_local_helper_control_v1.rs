// Source control is checked independently of the physical success-chain audit.
fn check_unit_local_control_v1(
    input: &UnitLocalAssociationInputV1<'_>,
    source_block: SemanticBlockIdV1,
    cursor: &mut SourceLocalCursorV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<UnitLocalControlStepV1, ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::LocalFrameControlKindV1 as Core;
    use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
    use fe2o3_pliron::{
        ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Role,
    };
    budget.charge_work(12)?;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let bad = |detail| {
        unsupported(
            input.key.function.index(),
            Some(source_block.index()),
            None,
            detail,
        )
    };
    let source = input
        .source
        .blocks()
        .get(source_block.index() as usize)
        .ok_or_else(mismatch)?;
    let (native_ordinal, native) = cursor.native_block(source_block, budget)?;
    let native_id = native.id;
    let span = *cursor.terminator_span(source_block, budget)?;
    if span.correspondence_owner != input.key.root
        || span.semantic_function != input.key.function
        || span.semantic_block != source_block
        || span.kernel_ir_block != native_id
    {
        return Err(mismatch());
    }
    let source_edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(source_block.index()), 0);
    let site = Site::Terminator {
        block: SsaBlockIdV1::new(source_block.index()),
    };

    // The core records its first inactive sink immediately before Selected.
    // It is claimed once, not mistaken for a source block or re-scanned per Assert.
    if let Some((index, row)) = cursor.peek_core_control(budget)?
        && matches!(row.kind(), Core::InactiveTrap)
    {
        if !matches!(
            source.terminator().kind(),
            SemanticTerminatorKindV1::Assert { .. }
        ) {
            return Err(mismatch());
        }
        check_unit_local_inactive_trap_v1(input, index, row, cursor, budget)?;
    }
    let (core_index, core) = cursor.peek_core_control(budget)?.ok_or_else(mismatch)?;
    budget.charge_work(2)?;
    if core.function_ordinal() != input.key.physical || core.block() != native_id {
        return Err(mismatch());
    }
    let kind = match source.terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge) => {
            budget.charge_work(6)?;
            let (_, destination) = cursor.native_block(edge.target(), budget)?;
            let target = destination.id;
            let (_, actual) = cursor.native_block(source_block, budget)?;
            if edge.role() != SemanticEdgeRoleV1::Goto
                || !matches!(&actual.terminator, Some(Terminator::Branch { target: found, .. }) if *found == target)
                || core.kind() != (Core::Branch { target })
            {
                return Err(mismatch());
            }
            let bindings = cursor.core_edge_bindings(native_id, 0, target, budget)?;
            let staged = cursor.stage_edge_bindings(source_edge, bindings, budget)?;
            UnitLocalControlKindV1::Goto {
                edge: source_edge,
                target: edge.target(),
                physical_target: target,
                core_bindings: bindings,
                staged_values: staged,
            }
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target,
            unwind,
        } => {
            budget.charge_work(16)?;
            if matches!(unwind, SemanticUnwindActionV1::Cleanup(_))
                || target.role() != SemanticEdgeRoleV1::AssertSuccess
            {
                return Err(bad("local helper assertion has an unsupported source edge"));
            }
            let SemanticAssertMessageV1::BoundsCheck { length, index } = message else {
                return Err(bad("local helper assertion requires a bounds diagnostic"));
            };
            let (_, success) = cursor.native_block(target.target(), budget)?;
            let success = success.id;
            let (_, actual) = cursor.native_block(source_block, budget)?;
            let Some(Terminator::ConditionalBranch {
                condition: native_condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            }) = &actual.terminator
            else {
                return Err(mismatch());
            };
            let native_condition = *native_condition;
            let (actual_success, inactive, inactive_arguments) = if *expected {
                (*then_target, *else_target, else_arguments)
            } else {
                (*else_target, *then_target, then_arguments)
            };
            let successor = u8::from(!*expected);
            if actual_success != success
                || !inactive_arguments.is_empty()
                || core.kind()
                    != (Core::Selected {
                        condition: native_condition,
                        value: *expected,
                        successor,
                        target: success,
                        inactive,
                    })
            {
                return Err(mismatch());
            }
            let origins = SemanticKirAssertOriginsV1 {
                executable: input.subject.executable,
                semantic_ssa: input.subject.semantic_ssa,
                origins: input.origins,
            };
            let origin = origins.assert_condition(
                input.key.root,
                input.key.function,
                source_block,
                budget,
            )?;
            let block_coordinate = fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(input.key.physical).map_err(|_| mismatch())?,
                ),
                block: u32::try_from(native_ordinal).map_err(|_| mismatch())?,
            };
            if origin.expected() != *expected
                || origin.semantic_success() != target.target()
                || !matches!(origin.outcome(), SemanticKirAssertConditionOutcomeV1::Emitted {
                    condition_use: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand { block, operand: 0 },
                    success_edge, failure_edge, ..
                } if block == block_coordinate && success_edge.source == block_coordinate
                    && success_edge.successor == u32::from(successor)
                    && failure_edge.source == block_coordinate
                    && failure_edge.successor == u32::from(1 - successor))
            {
                return Err(mismatch());
            }
            let predicate = cursor.resolve_operand(
                site,
                Role::AssertCondition,
                condition,
                UnitLocalOperandUseV1::Native(native_condition),
                budget,
            )?;
            budget.charge_work(4)?;
            let row = cursor.value(predicate).ok_or_else(mismatch)?;
            if row.key != input.key
                || row.known_bits != Some(u64::from(*expected))
                || !matches!(row.native, UnitLocalNativeValueV1::Scalar {
                    value, scalar: ScalarType::Bool, ..
                } if value == native_condition)
            {
                return Err(bad(
                    "local helper assertion predicate is not independently known",
                ));
            }
            // Adapter order is condition, diagnostic length, diagnostic index.
            let length = cursor.resolve_operand(
                site,
                Role::AssertMessage(0),
                length,
                UnitLocalOperandUseV1::Diagnostic,
                budget,
            )?;
            let index = cursor.resolve_operand(
                site,
                Role::AssertMessage(1),
                index,
                UnitLocalOperandUseV1::Diagnostic,
                budget,
            )?;
            for value in [length, index] {
                budget.charge_work(6)?;
                let row = cursor.value(value).ok_or_else(mismatch)?;
                let shape = input
                    .subject
                    .semantic_ssa
                    .source_semantic()
                    .types()
                    .get(row.source_type.index() as usize)
                    .ok_or_else(mismatch)?
                    .shape();
                let scalar = match shape {
                    SemanticTypeShapeV1::Scalar(scalar) => *scalar,
                    SemanticTypeShapeV1::ValidityScalar(validity) => validity.scalar(),
                    _ => return Err(bad("local helper bounds diagnostic is not unsigned")),
                };
                if row.key != input.key
                    || !matches!(row.native, UnitLocalNativeValueV1::DiagnosticOnly)
                    || !matches!(
                        scalar,
                        SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 8 | 16 | 32 | 64
                        }
                    )
                {
                    return Err(bad("local helper bounds diagnostic is not unsigned"));
                }
            }
            let bindings = cursor.core_edge_bindings(native_id, successor, success, budget)?;
            let staged = cursor.stage_edge_bindings(source_edge, bindings, budget)?;
            UnitLocalControlKindV1::Assert {
                edge: source_edge,
                target: target.target(),
                physical_target: success,
                predicate,
                length,
                index,
                expected: *expected,
                selected_successor: u32::from(successor),
                inactive_block: inactive,
                core_bindings: bindings,
                staged_values: staged,
            }
        }
        SemanticTerminatorKindV1::Return => {
            budget.charge_work(5)?;
            let (_, actual) = cursor.native_block(source_block, budget)?;
            if core.kind() != Core::Return
                || span.operation_count != 0
                || !matches!(&actual.terminator, Some(Terminator::Return { values }) if values.is_empty())
            {
                return Err(mismatch());
            }
            let unit_value = cursor.append_unit_return(source_block, budget)?;
            UnitLocalControlKindV1::Return {
                local: input.unit_return_local,
                unit_type: input.unit_type,
                unit_value,
            }
        }
        _ => {
            return Err(bad(
                "local helper source control is outside Goto/Assert/Return",
            ));
        }
    };
    cursor.claim_core_control(core_index, budget)?;
    let control = cursor.append_control(
        UnitLocalControlRowV1 {
            association: input.association,
            source_block: Some(source_block),
            physical_block: native_id,
            physical_control: core_index,
            kind,
        },
        budget,
    )?;
    match kind {
        UnitLocalControlKindV1::Goto {
            target,
            staged_values,
            ..
        }
        | UnitLocalControlKindV1::Assert {
            target,
            staged_values,
            ..
        } => Ok(UnitLocalControlStepV1::Continue {
            target,
            staged_values,
        }),
        UnitLocalControlKindV1::Return { .. } => Ok(UnitLocalControlStepV1::Return { control }),
        UnitLocalControlKindV1::InactiveTrap { .. } => Err(mismatch()),
    }
}

fn check_unit_local_inactive_trap_v1(
    input: &UnitLocalAssociationInputV1<'_>,
    core_index: usize,
    core: RetainedLocalControlV1,
    cursor: &mut SourceLocalCursorV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let block = core.block();
    let (_, actual) = cursor.physical_block(block, budget)?;
    if core.function_ordinal() != input.key.physical
        || !matches!(
            core.kind(),
            fe2o3_kernel_ir::LocalFrameControlKindV1::InactiveTrap
        )
        || !actual.parameters.is_empty()
        || actual.operations.len() != 1
        || !matches!(&actual.terminator, Some(Terminator::Unreachable))
    {
        return Err(mismatch());
    }
    let operation = &actual.operations[0];
    let OperationKind::Call { callee, arguments } = &operation.kind else {
        return Err(mismatch());
    };
    if !arguments.is_empty() || !operation.results.is_empty() {
        return Err(mismatch());
    }
    // The v1 public parser inspects eight descriptors. Arity is checked before
    // constructing any Print payload, so this zero-argument call allocates none.
    budget.charge_work(
        callee
            .as_str()
            .len()
            .checked_add(2)
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(2))
            .ok_or(ArgumentResourceV1::Arithmetic)?,
    )?;
    if !matches!(
        fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
        Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)
    ) {
        return Err(mismatch());
    }
    let (synthetic_span, span) = cursor.synthetic_trap_span(block, budget)?;
    if span.correspondence_owner != input.key.root
        || span.semantic_function != input.key.function
        || span.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        || span.kernel_ir_block != block
        || span.first_operation_ordinal != 0
        || span.operation_count != 1
    {
        return Err(mismatch());
    }
    cursor.claim_native_operation(block, 0, budget)?;
    cursor.claim_core_control(core_index, budget)?;
    cursor.append_control(
        UnitLocalControlRowV1 {
            association: input.association,
            source_block: None,
            physical_block: block,
            physical_control: core_index,
            kind: UnitLocalControlKindV1::InactiveTrap { synthetic_span },
        },
        budget,
    )?;
    Ok(())
}
