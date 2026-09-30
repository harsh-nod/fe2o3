use super::*;
use crate::AmdGpuDiagnosticOperation;

fn trap_fixture(read: bool) -> Module {
    let mut module = fixture(Axis::X, AccessMode::ReadWrite, read);
    let function = &mut module.functions[0];
    function.required_capabilities = AmdGpuDiagnosticOperation::Trap.required_capabilities();
    let body = function.body.as_mut().unwrap();
    if read {
        body.blocks[1].operations.remove(2);
    }
    body.blocks[2]
        .operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    body.blocks[2].terminator = Some(Terminator::Unreachable);
    module
        .functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    module
}

fn measure_trap_batch(
    read: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<Option<usize>>, usize, usize) {
    let (graph, credit) = owner(&trap_fixture(read));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        &graph,
        Default::default(),
        &mut budget,
        |reads, budget| {
            with_canonical_guarded_global_stores_v24(
                &graph,
                Default::default(),
                budget,
                |stores, budget| {
                    let expected_reads = if read { (1, 0, 0) } else { (0, 1, 0) };
                    let expected_stores = if read { (0, 1, 0) } else { (1, 0, 0) };
                    assert_eq!(
                        reads.function_effects(FunctionCoordinate(0), budget)?,
                        expected_reads
                    );
                    assert_eq!(
                        stores.function_effects(FunctionCoordinate(0), budget)?,
                        expected_stores
                    );
                    crate::with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[ExplicitLaunchExtent::Exact {
                            rank: 3,
                            extents: [64, 1, 1],
                        }; 2],
                        FormalIndexWidth::Bits64,
                        budget,
                        |batch, budget| {
                            assert!(!batch.runtime_requirements_are_discharged());
                            assert!(!batch.grants_artifact_or_launch_authority());
                            assert!(matches!(
                                graph.module().functions[0].body.as_ref().unwrap().blocks[2]
                                    .terminator,
                                Some(Terminator::Unreachable)
                            ));
                            batch.access_count(budget)
                        },
                    )
                },
            )
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn registered_trap_retains_control_but_not_unknown_memory_call_census() {
    for read in [false, true] {
        assert_eq!(
            measure_trap_batch(read, 100_000_000, 100_000_000)
                .0
                .unwrap(),
            Some(1)
        );
    }
}

#[test]
fn registered_trap_census_exact_and_one_short_budgets() {
    for read in [false, true] {
        let (result, work, peak) = measure_trap_batch(read, 100_000_000, 100_000_000);
        assert_eq!(result.unwrap(), Some(1));
        assert_eq!(measure_trap_batch(read, work, peak).0.unwrap(), Some(1));
        assert!(matches!(
            measure_trap_batch(read, work - 1, peak).0,
            Err(Failure::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            measure_trap_batch(read, work, peak - 1).0,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
    }
}

#[test]
fn unknown_and_other_diagnostic_calls_remain_unresolved() {
    for diagnostic in [
        None,
        Some(AmdGpuDiagnosticOperation::DebugTrap),
        Some(AmdGpuDiagnosticOperation::Clock32),
    ] {
        let mut module = trap_fixture(true);
        module.functions.pop();
        let (declaration, operation) = match diagnostic {
            Some(diagnostic) => {
                let result = diagnostic.result_type().map(|_| ValueId(99));
                (diagnostic.declaration(), diagnostic.operation(result))
            }
            None => {
                let id = crate::FunctionId::new("opaque_nonmemory_claim_is_not_authority");
                (
                    Function::external_import(id.clone(), Signature::new(vec![], vec![])),
                    Operation::new(
                        vec![],
                        OperationKind::Call {
                            callee: id,
                            arguments: vec![],
                        },
                    ),
                )
            }
        };
        module.functions[0].body.as_mut().unwrap().blocks[2].operations[0] = operation;
        module.functions[0].body.as_mut().unwrap().blocks[2].terminator =
            Some(Terminator::Return { values: vec![] });
        module.functions.push(declaration);
        let (graph, credit) = owner(&module);
        run(&graph, credit, |stores, budget| {
            assert_eq!(
                stores.function_effects(FunctionCoordinate(0), budget)?,
                (0, 1, 1)
            );
            with_canonical_guarded_global_reads_v18(
                &graph,
                Default::default(),
                budget,
                |reads, budget| {
                    assert_eq!(
                        reads.function_effects(FunctionCoordinate(0), budget)?,
                        (1, 0, 1)
                    );
                    assert!(
                        crate::with_canonical_conditional_slice_domains_v26(
                            reads,
                            stores,
                            &[ExplicitLaunchExtent::Exact {
                                rank: 3,
                                extents: [64, 1, 1]
                            }; 2],
                            FormalIndexWidth::Bits64,
                            budget,
                            |_, _| Ok(())
                        )
                        .unwrap()
                        .is_none()
                    );
                    Ok(())
                },
            )
        })
        .unwrap();
    }
}

#[test]
fn forged_reserved_trap_declarations_and_continuations_cannot_make_verified_owner() {
    for fault in 0..6 {
        let mut module = trap_fixture(true);
        match fault {
            0 => module.functions[1].required_capabilities.clear(),
            1 => module.functions[1]
                .signature
                .parameters
                .push(Type::Scalar(ScalarType::U32)),
            2 => module.functions[1].role = crate::FunctionRole::InternalHelper,
            3 => {
                module.functions[0].body.as_mut().unwrap().blocks[2].terminator =
                    Some(Terminator::Return { values: vec![] })
            }
            4 => {
                let OperationKind::Call { arguments, .. } =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.push(ValueId(1));
            }
            5 => module.functions[0].body.as_mut().unwrap().blocks[2].operations[0]
                .results
                .push(ValueDef::new(ValueId(99), Type::Scalar(ScalarType::U32))),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        assert!(
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                StorageLayoutLimitsV1 {
                    rows: 8,
                    edges: 16,
                    containment_depth: 8,
                    object_bytes: 1024
                },
                &mut budget
            )
            .is_err(),
            "fault={fault}"
        );
        assert_eq!(budget.storage(), 0);
    }
}
