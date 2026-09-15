// A source-generated runtime failure is a control effect, not an ordinary Call
// allowance. This bridge classifies it only; assertion discharge and the ranked
// control checks remain separate mandatory consumers.

fn source_output_census_trap_payload_v1(
    original: &BasicBlock,
    output: &BasicBlock,
    output_operation: usize,
    roles: (fe2o3_kernel_ir::FunctionRole, fe2o3_kernel_ir::FunctionRole),
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    if roles
        != (
            fe2o3_kernel_ir::FunctionRole::KernelEntry,
            fe2o3_kernel_ir::FunctionRole::KernelEntry,
        )
    {
        return Err(Error::Invalid(
            "runtime failure trap requires the exact entry",
        ));
    }
    budget.charge_work(8).map_err(Error::Resource)?;
    if !original.parameters.is_empty()
        || original.operations.len() != 1
        || !matches!(original.terminator, Some(Terminator::Unreachable))
        || output_operation.checked_add(1) != Some(output.operations.len())
        || !matches!(output.terminator, Some(Terminator::Unreachable))
    {
        return Err(Error::Invalid("runtime failure trap block shape differs"));
    }
    let before = &original.operations[0];
    let after = &output.operations[output_operation];
    let (
        OperationKind::Call {
            callee: before_callee,
            arguments: before_arguments,
        },
        OperationKind::Call {
            callee: after_callee,
            arguments: after_arguments,
        },
    ) = (&before.kind, &after.kind)
    else {
        return Err(Error::Invalid(
            "runtime failure is not a retained trap Call",
        ));
    };
    budget.charge_work(4).map_err(Error::Resource)?;
    if !before_arguments.is_empty()
        || !after_arguments.is_empty()
        || !before.results.is_empty()
        || !after.results.is_empty()
    {
        return Err(Error::Invalid(
            "runtime failure trap arguments or results differ",
        ));
    }
    // The replay-sealed synthetic rule already checked the canonical N trap.
    // Compare its exact identity without creating a String or diagnostic payload.
    if !private_array_equal_bytes_v1(
        before_callee.as_str().as_bytes(),
        after_callee.as_str().as_bytes(),
        &mut PrivateArrayQueryWorkV1 { budget },
    )
    .map_err(Error::PrivateArray)?
    {
        return Err(Error::Invalid("runtime failure trap identity differs"));
    }
    Ok(())
}

fn source_output_census_trap_span_v1(
    spans: &[SemanticKirSyntheticOperationSpanV1],
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: BlockId,
    operation: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut found = false;
    for span in spans {
        budget.charge_work(4).map_err(Error::Resource)?;
        if span.correspondence_owner != owner
            || span.semantic_function != function
            || span.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        {
            continue;
        }
        budget.charge_work(5).map_err(Error::Resource)?;
        if found
            || span.kernel_ir_block != block
            || operation != 0
            || span.first_operation_ordinal != 0
            || span.operation_count != 1
        {
            return Err(Error::Invalid("runtime failure trap source span differs"));
        }
        found = true;
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    if !found {
        return Err(Error::Invalid(
            "Call has no source-generated runtime failure rule",
        ));
    }
    Ok(())
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    fn census_runtime_failure_trap_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        use fe2o3_kernel_ir::CanonicalKirOperationOriginV1;
        budget.charge_work(1).map_err(Error::Resource)?;
        // These borrowed rows were independently checked against this exact B/O
        // pair before the view was sealed. No caller-supplied rows enter here.
        let rows = self.checked_output.candidate();
        let row = assert_origin_find_v1(rows.operations, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.output.cmp(&coordinate))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("runtime failure trap output origin absent"))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        let CanonicalKirOperationOriginV1::Retained(original) = rows.operations[row].origin else {
            return Err(Error::Invalid("runtime failure trap is not retained"));
        };
        if original.block.function != coordinate.block.function || original.operation != 0 {
            return Err(Error::Invalid(
                "runtime failure trap original coordinate differs",
            ));
        }
        budget.charge_work(6).map_err(Error::Resource)?;
        let before_function = self
            .source
            .executable()
            .module()
            .functions
            .get(original.block.function.0 as usize)
            .ok_or(Error::Invalid("runtime failure original function absent"))?;
        let after_function = self
            .output()
            .module()
            .functions
            .get(coordinate.block.function.0 as usize)
            .ok_or(Error::Invalid("runtime failure output function absent"))?;
        let before = before_function
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(original.block.block as usize))
            .ok_or(Error::Invalid("runtime failure original block absent"))?;
        let after = after_function
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
            .ok_or(Error::Invalid("runtime failure output block absent"))?;
        source_output_census_trap_span_v1(
            self.source.correspondence.synthetic_operation_spans(),
            owner,
            function,
            before.id,
            original.operation,
            budget,
        )?;
        source_output_census_trap_payload_v1(
            before,
            after,
            coordinate.operation as usize,
            (before_function.role, after_function.role),
            budget,
        )
    }
}

#[cfg(test)]
mod source_output_runtime_trap_tests_v1 {
    use super::*;

    fn payload(
        original: &BasicBlock,
        output: &BasicBlock,
        index: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        source_output_census_trap_payload_v1(
            original,
            output,
            index,
            (
                fe2o3_kernel_ir::FunctionRole::KernelEntry,
                fe2o3_kernel_ir::FunctionRole::KernelEntry,
            ),
            budget,
        )
    }

    fn trap_block() -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(2));
        block
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        block.terminator = Some(Terminator::Unreachable);
        block
    }

    fn with_budget<T>(body: impl FnOnce(&mut AssertOriginBudgetV1<'_>) -> T) -> T {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        let result = body(&mut budget);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
        result
    }

    #[test]
    fn runtime_trap_payload_is_exact_and_terminal_but_allows_checked_merge_prefix() {
        let original = trap_block();
        with_budget(|budget| payload(&original, &original, 0, budget)).unwrap();
        let mut merged = original.clone();
        merged.operations.insert(
            0,
            Operation::new(
                vec![ValueDef::new(ValueId(0), Type::INDEX)],
                OperationKind::Constant(Constant::Index(0)),
            ),
        );
        with_budget(|budget| payload(&original, &merged, 1, budget)).unwrap();
        for fault in 0..7 {
            let mut changed = original.clone();
            match fault {
                0 => changed.operations[0] = AmdGpuDiagnosticOperation::DebugTrap.operation(None),
                1 => {
                    let OperationKind::Call { arguments, .. } = &mut changed.operations[0].kind
                    else {
                        unreachable!()
                    };
                    arguments.push(ValueId(0));
                }
                2 => changed.operations[0]
                    .results
                    .push(ValueDef::new(ValueId(0), Type::INDEX)),
                3 => changed.terminator = Some(Terminator::Return { values: vec![] }),
                4 => changed
                    .operations
                    .push(AmdGpuDiagnosticOperation::Trap.operation(None)),
                5 => changed.operations.clear(),
                6 => changed.terminator = None,
                _ => unreachable!(),
            }
            assert!(
                with_budget(|budget| payload(&original, &changed, 0, budget)).is_err(),
                "fault {fault}"
            );
        }
    }

    #[test]
    fn same_named_trap_without_exact_synthetic_source_rule_is_refused() {
        let span = SemanticKirSyntheticOperationSpanV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(0),
            semantic_function: SemanticFunctionIdV1::from_index(0),
            rule: SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
            kernel_ir_block: BlockId(2),
            first_operation_ordinal: 0,
            operation_count: 1,
        };
        let check = |spans: &[SemanticKirSyntheticOperationSpanV1], operation| {
            with_budget(|budget| {
                source_output_census_trap_span_v1(
                    spans,
                    span.correspondence_owner,
                    span.semantic_function,
                    BlockId(2),
                    operation,
                    budget,
                )
            })
        };
        check(&[span], 0).unwrap();
        assert!(check(&[], 0).is_err());
        assert!(check(&[span, span], 0).is_err());
        assert!(check(&[span], 1).is_err());
        for fault in 0..6 {
            let mut changed = span;
            match fault {
                0 => changed.correspondence_owner = SemanticFunctionIdV1::from_index(1),
                1 => changed.semantic_function = SemanticFunctionIdV1::from_index(1),
                2 => changed.rule = SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
                3 => changed.kernel_ir_block = BlockId(3),
                4 => changed.first_operation_ordinal = 1,
                5 => changed.operation_count = 2,
                _ => unreachable!(),
            }
            assert!(check(&[changed], 0).is_err(), "fault {fault}");
        }
    }

    #[test]
    fn runtime_trap_payload_exact_under_budget_keeps_history_and_floor() {
        let block = trap_block();
        let OperationKind::Call { callee, .. } = &block.operations[0].kind else {
            unreachable!()
        };
        // Roles 2, shape 8, argument/result 4, exact-byte comparison length+1.
        let exact = 14 + callee.as_str().len() + 1;
        for available in [exact, exact - 1] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7 + available);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 11);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(11).unwrap();
            let result = payload(&block, &block, 0, &mut budget);
            assert_eq!(result.is_ok(), available == exact);
            assert_eq!(budget.storage(), 11);
            assert_eq!(budget.peak_storage(), 11);
            assert_eq!(work.failed_work(), (available < exact).then_some(7 + exact));
        }
    }

    #[test]
    fn runtime_trap_requires_both_exact_entry_roles() {
        use fe2o3_kernel_ir::FunctionRole;
        let block = trap_block();
        for roles in [
            (FunctionRole::InternalHelper, FunctionRole::KernelEntry),
            (FunctionRole::KernelEntry, FunctionRole::ExternalImport),
        ] {
            assert!(
                with_budget(|budget| source_output_census_trap_payload_v1(
                    &block, &block, 0, roles, budget
                ))
                .is_err()
            );
        }
    }
}
