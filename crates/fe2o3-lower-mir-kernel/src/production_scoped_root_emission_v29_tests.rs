use super::*;

pub(super) fn assertion_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let boolean = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        None,
    );
    let mut functions = semantic.functions().to_vec();
    let provider = &functions[HELPER.index() as usize];
    let mut blocks = provider.blocks().to_vec();
    let call = blocks[1].terminator().kind().clone();
    blocks[1] = block(
        91,
        vec![],
        SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                boolean,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            )),
            expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(3),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    blocks.push(block(96, vec![], call));
    blocks.push(block(
        97,
        vec![],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(4),
        )),
    ));
    functions[HELPER.index() as usize] = function(
        100,
        SemanticFunctionRoleV1::InternalHelper,
        provider.abi().clone(),
        provider.locals().to_vec(),
        blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn emit_checked(
    source: &ExecutionLifecycleSourceV29<'_>,
    launch: &ProductionSourceLaunchRosterV1,
    root: RootInput<'_>,
    groups: u32,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let outer_private = PrivateArrayPayloadV1 {
        occupied: 40,
        capacity: 80,
    };
    let result = crate::with_checked_context_root_v29(
        source.owner,
        launch,
        root,
        budget,
        |checked, budget| {
            let mut closure = ReachableClosureBudgetV1::new(limits.max_blocks);
            let mut private = PrivateArrayLazyBudgetV1::new(1, limits.max_operations);
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
            let mut foreign_budget = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
            assert!(matches!(
                emit_pending_scoped_root_v29(
                    &checked,
                    source,
                    limits,
                    &mut closure,
                    &mut private,
                    outer_private,
                    &mut foreign_budget,
                ),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!(foreign_budget.storage(), 0);
            assert_eq!(foreign_work.work(), 0);
            let foreign_owner = lifecycle_owner(false);
            let foreign_source = ExecutionLifecycleSourceV29 {
                owner: &foreign_owner,
                input: source.input,
                ledger: source.ledger,
            };
            assert!(
                emit_pending_scoped_root_v29(
                    &checked,
                    &foreign_source,
                    limits,
                    &mut closure,
                    &mut private,
                    outer_private,
                    budget,
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(closure.consumed, 0);
            assert_eq!(closure.argument_rows, 0);
            let output = emit_pending_scoped_root_v29(
                &checked,
                source,
                limits,
                &mut closure,
                &mut private,
                outer_private,
                budget,
            );
            assert!(
                private.active.is_none(),
                "array-free roots must keep lazy accounting inactive"
            );
            Ok(output)
        },
    )
    .map_err(|error| match error {
        crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
        _ => execution_lifecycle_error_v29(),
    })?;
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            assert_eq!(
                budget.storage(),
                floor,
                "failed assembly must release only its own reservations"
            );
            return Err(error);
        }
    };
    assert!(output.ledger == budget.work_ledger_identity_v1());
    assert_eq!(output.private_payload.occupied, outer_private.occupied);
    assert_eq!(output.private_payload.capacity, outer_private.capacity);
    assert_eq!(budget.storage() - floor, output.retained_emission_storage);
    assert!(output.pending.additional_storage_bytes <= output.retained_emission_storage);
    assert_eq!(output.kernel.entry.as_str(), "lifecycle_fixture");
    assert_eq!(output.pending.function.id, output.kernel.entry);
    assert_eq!(
        output.kernel.workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    assert_eq!(
        output.kernel.domain,
        LaunchDomain::D1 {
            x: if groups == 1 {
                LaunchExtent::Static(64)
            } else {
                LaunchExtent::Dynamic
            },
        }
    );
    assert_eq!(output.pending.sidecars.rows.len(), 3);
    assert_eq!(output.pending.coordinates.root, ROOT);
    let body = output.pending.function.body.as_ref().unwrap();
    let mut blocks = BTreeSet::new();
    let mut values: BTreeSet<_> = body.parameters.iter().copied().collect();
    for block in &body.blocks {
        assert!(blocks.insert(block.id));
        for value in &block.parameters {
            assert!(values.insert(value.id));
        }
        for operation in &block.operations {
            if let OperationKind::Call { callee, .. } = &operation.kind {
                assert!(!callee.as_str().starts_with("__fe2o3_execution_"));
            }
            for value in &operation.results {
                assert!(values.insert(value.id));
            }
        }
    }
    let mut observations = Vec::new();
    let mut operations = 0;
    for (index, row) in output.pending.sidecars.rows.iter().enumerate() {
        assert_eq!(row.source_call_instance.unwrap().index(), index);
        assert!(row.instance_assert_origins.is_some());
        let events = row.lifecycle_events.as_ref().unwrap();
        assert_eq!(events.instance.index(), index);
        observations.extend_from_slice(&events.rows);
        operations += row.emitted_operations + events.rows.len();
    }
    assert!(operations <= limits.max_operations);
    assert!(matches!(
        observations[0].kind,
        DeferredLifecycleKindV29::Issue { .. }
    ));
    assert_eq!(observations[0].original_block, BlockId(0));
    if source.owner.source_semantic().functions()[HELPER.index() as usize]
        .blocks()
        .iter()
        .any(|block| {
            matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            )
        })
    {
        let provider = &output.pending.sidecars.rows[1];
        let first = provider
            .lifecycle_events
            .as_ref()
            .unwrap()
            .placement
            .first_block;
        assert_eq!(provider.synthetic_operation_spans.len(), 1);
        let failure = &provider.synthetic_operation_spans[0];
        assert_eq!(
            failure.rule,
            SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        );
        assert_eq!(failure.kernel_ir_block, BlockId(first + 5));
        assert_eq!(
            output.pending.sidecars.rows[2]
                .lifecycle_events
                .as_ref()
                .unwrap()
                .placement
                .first_block,
            first + 6
        );
        let trap = body
            .blocks
            .iter()
            .find(|block| block.id == failure.kernel_ir_block)
            .unwrap();
        assert_eq!(trap.terminator, Some(Terminator::Unreachable));
        assert_eq!(provider.blocks.len(), 4);
        assert!(
            provider.diagnostic_declarations.values().any(|function| {
                function.id == AmdGpuDiagnosticOperation::Trap.declaration().id
            })
        );
    }
    let retained = output.retained_emission_storage;
    drop(output);
    budget.release_storage(retained)?;
    assert_eq!(budget.storage(), floor);
    Ok(observations)
}

fn fault(groups: u32, limits: ProductionSemanticKirLimitsV1) -> Fault {
    Fault::Orchestrated {
        groups,
        limits,
        assertion: false,
    }
}

#[test]
fn checked_root_placement_accounts_for_assert_failure_and_unreachable_holes() {
    run_lifecycle(
        false,
        Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            assertion: true,
        },
        10_000_000,
        10_000_000,
    )
    .0
    .unwrap();
}

#[test]
fn checked_root_orchestrates_real_launch_and_retains_lifecycle_sidecars() {
    for branches in [false, true] {
        for groups in [1, 2, 7] {
            let result = run_lifecycle(
                branches,
                fault(groups, ProductionSemanticKirLimitsV1::default()),
                10_000_000,
                10_000_000,
            )
            .0
            .unwrap();
            assert_eq!(result.len(), if branches { 4 } else { 3 });
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Issue { .. }))
                    .count(),
                1
            );
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Derive { .. }))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn checked_root_orchestration_obeys_exact_ledger_boundaries() {
    let mode = fault(2, ProductionSemanticKirLimitsV1::default());
    let (result, work, storage) = run_lifecycle(true, mode, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(run_lifecycle(true, mode, work, storage).0.is_ok());
    assert!(run_lifecycle(true, mode, work - 1, storage).0.is_err());
    assert!(run_lifecycle(true, mode, work, storage - 1).0.is_err());
}

#[test]
fn checked_root_orchestration_bounds_whole_expansion_before_emission() {
    for limits in [
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 128, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 1, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 0, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 128, 0),
    ] {
        assert!(
            matches!(
                run_lifecycle(true, fault(2, limits), 10_000_000, 10_000_000).0,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ),
            "{limits:?}"
        );
    }
}
