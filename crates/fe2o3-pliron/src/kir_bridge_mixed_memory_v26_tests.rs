use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateMemoryLimitsV1, check_canonical_kir_private_memory_v18,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock as Block, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, ExplicitLaunchExtent,
    FormalIndexWidth, Function, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation as IrOp, ScalarType, Signature, StorageLayoutIdV1,
    StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1, StorageOperationV1, ValueDef,
    with_canonical_conditional_slice_domains_v26, with_canonical_guarded_global_reads_v18,
    with_canonical_guarded_global_stores_v24,
};
use std::cell::Cell;

#[path = "kir_bridge_mixed_cfg_domain_v26_tests.rs"]
mod cfg_domain_v26;

const AMPLE: usize = 1 << 40;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 64,
    containment_depth: 16,
    object_bytes: 4096,
};

fn value(id: u32, ty: Type, kind: OperationKind) -> IrOp {
    IrOp::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}

fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = Block::new(BlockId(10));
    entry.operations = vec![
        value(
            10,
            Type::pointer(
                Type::StorageObject(StorageLayoutIdV1(0)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        IrOp::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(10),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        value(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        value(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        value(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut body = Block::new(BlockId(20));
    body.operations = vec![
        value(
            5,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value(
            6,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(2),
            },
        ),
        IrOp::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        value(
            7,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        value(
            11,
            scalar.clone(),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    body.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = Block::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("mixed-native-v26");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar.clone(),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(8)],
        vec![entry, body, exit],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn trap_fixture() -> Module {
    use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;
    let mut module = fixture();
    let function = &mut module.functions[0];
    function.required_capabilities = Diagnostic::Trap.required_capabilities();
    let exit = &mut function.body.as_mut().unwrap().blocks[2];
    exit.operations.push(Diagnostic::Trap.operation(None));
    exit.terminator = Some(Terminator::Unreachable);
    module.functions.push(Diagnostic::Trap.declaration());
    module
}

fn fault(graph: &mut KirPlironGraphV18<'_>, fault: usize) {
    let op = |block, operation| {
        *graph
            .coordinates
            .iter()
            .find(|(_, coordinate)| {
                **coordinate
                    == KirBridgeCoordinateV1::Operation {
                        function: 0,
                        block,
                        operation,
                    }
            })
            .unwrap()
            .0
    };
    let context = &graph.session.context;
    let val = |id| {
        *graph
            .origins
            .values
            .iter()
            .find(|(_, original)| **original == ValueId(id))
            .unwrap()
            .0
    };
    match fault {
        16 => {
            let entry = *graph
                .coordinates
                .iter()
                .find(|(_, coordinate)| {
                    **coordinate
                        == KirBridgeCoordinateV1::Terminator {
                            function: 0,
                            block: 0,
                        }
                })
                .unwrap()
                .0;
            let target = *graph
                .origins
                .blocks
                .iter()
                .find(|(_, original)| **original == (0, BlockId(40)))
                .unwrap()
                .0;
            Operation::replace_successor(entry, context, 0, target);
        }
        1 => Operation::replace_operand(op(0, 4), context, 0, val(3)),
        2 => Operation::replace_operand(op(1, 1), context, 1, val(3)),
        3 => Operation::replace_operand(op(1, 2), context, 1, val(8)),
        4 => {
            let pointer = op(0, 2);
            let OperationKind::Intrinsic(intrinsic) = graph
                .origins
                .preserved_operations
                .get_mut(&pointer)
                .unwrap()
            else {
                panic!("intrinsic")
            };
            intrinsic.kind = IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            };
        }
        5 => graph.test_ranked_mutate_and_restore_v18(),
        8 => {
            let pointer = op(1, 2);
            graph.coordinates.remove(&pointer);
        }
        10 | 15 => {
            let call = Operation::get_op::<CallOp>(op(2, 0), context).unwrap();
            let original = call
                .get_attr_gpu_call_callee(context)
                .unwrap()
                .as_str()
                .to_owned();
            call.set_attr_gpu_call_callee(
                context,
                pliron::builtin::attributes::StringAttr::new("changed".into()),
            );
            if fault == 15 {
                call.set_attr_gpu_call_callee(
                    context,
                    pliron::builtin::attributes::StringAttr::new(original),
                );
            }
        }
        11 => {
            let call = Operation::get_op::<CallOp>(op(2, 0), context).unwrap();
            let wrong = FunctionType::get(context, vec![IndexType::get(context).into()], vec![]);
            call.set_attr_gpu_call_signature(
                context,
                pliron::builtin::attributes::TypeAttr::new(wrong.into()),
            );
        }
        12 => {
            let terminal = *graph
                .coordinates
                .iter()
                .find(|(_, coordinate)| {
                    **coordinate
                        == KirBridgeCoordinateV1::Terminator {
                            function: 0,
                            block: 2,
                        }
                })
                .unwrap()
                .0;
            graph
                .origins
                .preserved_terminators
                .insert(terminal, Terminator::Return { values: vec![] });
        }
        14 => {
            let pointer = op(2, 0);
            graph.coordinates.remove(&pointer);
        }
        _ => {}
    }
}

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (owner, credit) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    (owner, credit.retained_storage())
}

fn physical_error(error: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1) -> Failure {
    match error {
        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Resource(error) => {
            Failure::Resource(error)
        }
        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Inventory(
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error),
        ) => Failure::Resource(error),
        _ => Failure::ExactGraph,
    }
}

fn run_case(
    work_limit: usize,
    storage_limit: usize,
    which: usize,
    consume: impl FnMut(&NativeCanonicalMixedAdmissionV26<'_>) -> Result<(), Failure>,
) -> (Result<(), Failure>, usize, usize, usize) {
    let module = fixture();
    run_module_case(&module, work_limit, storage_limit, which, consume)
}

fn run_module_case(
    module: &Module,
    work_limit: usize,
    storage_limit: usize,
    which: usize,
    mut consume: impl FnMut(&NativeCanonicalMixedAdmissionV26<'_>) -> Result<(), Failure>,
) -> (Result<(), Failure>, usize, usize, usize) {
    let (owner, credit) = owner(module);
    let (foreign, foreign_credit) = self::owner(module);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let mut calls = 0;
    let result = (|| {
        budget.reserve_storage(credit.checked_add(foreign_credit).unwrap())?;
        let (inventory, inventory_credit) =
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).map_err(
                |error| match error {
                    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                        Failure::Resource(error)
                    }
                    _ => Failure::ExactGraph,
                },
            )?;
        budget.reserve_storage(inventory_credit.retained_storage())?;
        let (physical, physical_credit) = check_canonical_kir_private_memory_v18(
            &inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 },
            &mut budget,
        )
        .map_err(physical_error)?;
        budget.reserve_storage(physical_credit.retained_storage())?;
        let (mut graph, graph_credit) = KirPlironGraphV18::import(&owner, &mut budget)?;
        budget.reserve_storage(graph_credit.retained_storage())?;
        let original_epoch = graph.ranked_policy_epoch_v18()?;
        fault(&mut graph, which);
        let epoch = if matches!(which, 5 | 15) {
            original_epoch
        } else {
            graph.ranked_policy_epoch_v18()?
        };
        let mut native = None;
        let subject = if which == 7 { &foreign } else { &owner };
        let domain_result = with_canonical_guarded_global_reads_v18(
            subject,
            Default::default(),
            &mut budget,
            |reads, budget| {
                with_canonical_guarded_global_stores_v24(
                    subject,
                    Default::default(),
                    budget,
                    |stores, budget| {
                        with_canonical_conditional_slice_domains_v26(
                            reads,
                            stores,
                            &vec![
                                ExplicitLaunchExtent::Exact {
                                    rank: 3,
                                    extents: [64, 1, 1],
                                };
                                module.functions.len()
                            ],
                            FormalIndexWidth::Bits64,
                            budget,
                            |globals, budget| {
                                let floor = budget.storage();
                                native = Some(graph.visit_mixed_policy_functions_v26(
                                    &physical,
                                    globals,
                                    epoch,
                                    LAYOUTS,
                                    budget,
                                    |ordinal, input| {
                                        assert_eq!(ordinal, 0);
                                        calls += 1;
                                        if which == 6 {
                                            let context = input.context();
                                            let pointer = input.function().get_operation();
                                            let attributes =
                                                pointer.deref(context).attributes.clone();
                                            pointer.deref_mut(context).attributes = attributes;
                                            assert!(!input.authenticate(context, input.function()));
                                            assert!(input.operation(context, pointer).is_none());
                                            return Ok(());
                                        }
                                        consume(input)
                                    },
                                ));
                                budget.release_storage(
                                    budget.storage().checked_sub(floor).unwrap(),
                                )?;
                                Ok(())
                            },
                        )
                    },
                )
            },
        )
        .map_err(formal_error)?;
        if domain_result.is_none() {
            return Err(Failure::NativeSchema);
        }
        native.expect("complete family reached native gate")
    })();
    (result, budget.work(), budget.peak_storage(), calls)
}

#[test]
fn mixed_native_v26_registered_trap_pair_runs_nine_stages_without_relabeling_memory() {
    let (result, _, _, calls) = run_module_case(&trap_fixture(), AMPLE, AMPLE, 0, |input| {
        let mut pair = [0; 2];
        for pointer in input.private.coordinates.keys() {
            match input.operation(input.context(), *pointer).unwrap() {
                Kind::TrapCall => pair[0] += 1,
                Kind::TrapEnd => pair[1] += 1,
                _ => {}
            }
        }
        assert_eq!(pair, [1, 1]);
        let outcome = crate::canonical_private_v1::run_mixed_v26(
            input,
            crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            None,
        )
        .unwrap();
        assert!(outcome.report.reports().is_clean());
        assert!(!outcome.report.grants_artifact_or_launch_authority());
        for stage in 0..9 {
            assert_eq!(outcome.report.global_access_counts(stage), Some([1, 1]));
            assert_eq!(
                outcome.report.private_access_counts(stage),
                Some([1, 0, 1, 1, 1])
            );
        }
        assert!(!input.globals.runtime_requirements_are_discharged());
        assert!(!input.globals.grants_artifact_or_launch_authority());
        assert!(
            input
                .run_fixed(
                    crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                    None
                )
                .is_err()
        );
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
}

#[test]
fn mixed_native_v26_trap_pair_rejects_same_count_mutations_and_old_epoch_restore() {
    for which in [10, 11, 12, 14, 15] {
        let (result, _, _, calls) = run_module_case(&trap_fixture(), AMPLE, AMPLE, which, |_| {
            panic!("changed trap admitted")
        });
        assert!(result.is_err(), "fault {which}");
        assert_eq!(calls, 0, "fault {which}");
    }
}

#[test]
fn mixed_native_v26_trap_pair_exact_and_one_short_limits_remain_transactional() {
    let module = trap_fixture();
    let (result, work, peak, calls) = run_module_case(&module, AMPLE, AMPLE, 0, |_| Ok(()));
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
    assert!(
        run_module_case(&module, work, peak, 0, |_| Ok(()))
            .0
            .is_ok()
    );
    assert!(
        run_module_case(&module, work - 1, peak, 0, |_| Ok(()))
            .0
            .is_err()
    );
    assert!(
        run_module_case(&module, work, peak - 1, 0, |_| Ok(()))
            .0
            .is_err()
    );
}

#[test]
fn mixed_native_v26_nonterminal_diagnostics_do_not_become_trap_pairs() {
    use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;
    for diagnostic in [Diagnostic::DebugTrap, Diagnostic::Clock32] {
        let mut module = fixture();
        let function = &mut module.functions[0];
        function.required_capabilities = diagnostic.required_capabilities();
        function.body.as_mut().unwrap().blocks[2]
            .operations
            .push(diagnostic.operation(diagnostic.result_type().map(|_| ValueId(99))));
        module.functions.push(diagnostic.declaration());
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, 0, |_| {
            panic!("nonterminal diagnostic admitted")
        });
        assert!(result.is_err());
        assert_eq!(calls, 0);
    }
}

#[test]
fn mixed_native_v26_partitions_real_private_and_conditional_global_occurrences() {
    let (result, _, _, calls) = run_case(AMPLE, AMPLE, 0, |input| {
        assert!(input.supports_conditional_globals_v26());
        assert_eq!(input.conditional_global_counts_v26(), Some([1, 1]));
        let mut counts = [0; 4];
        for pointer in input.private.coordinates.keys() {
            match input.operation(input.context(), *pointer).unwrap() {
                Kind::Allocate | Kind::Read | Kind::Write => counts[0] += 1,
                Kind::ConditionalGlobalReadV26 => counts[1] += 1,
                Kind::ConditionalGlobalWriteV26 => counts[2] += 1,
                Kind::ConditionalGlobalIndexV26 => counts[3] += 1,
                _ => {}
            }
        }
        assert_eq!(counts, [3, 1, 1, 1]);
        assert!(matches!(
            input.run_fixed(
                crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                None
            ),
            Err(crate::PipelineErrorV1::CanonicalPrivateInput)
        ));
        assert!(!input.globals.runtime_requirements_are_discharged());
        assert!(!input.globals.grants_artifact_or_launch_authority());
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
}

#[test]
fn mixed_native_v26_current_epoch_cannot_launder_changed_guard_address_rhs_or_intrinsic() {
    for which in [1, 2, 3, 4, 8] {
        let (result, _, _, calls) =
            run_case(AMPLE, AMPLE, which, |_| panic!("changed graph admitted"));
        assert!(result.is_err(), "fault {which}");
        assert_eq!(calls, 0, "fault {which}");
    }
}

#[test]
fn mixed_native_v26_rejects_equal_foreign_owner_and_mutate_restore() {
    for which in [5, 7] {
        let (result, _, _, calls) = run_case(AMPLE, AMPLE, which, |_| {
            panic!("foreign/stale graph admitted")
        });
        assert!(result.is_err(), "fault {which}");
        assert_eq!(calls, 0);
    }
    let (result, _, _, calls) = run_case(AMPLE, AMPLE, 6, |_| panic!("mutation case intercepts"));
    assert!(matches!(result, Err(Failure::Mutation)));
    assert_eq!(calls, 1);
}

#[test]
fn mixed_native_v26_complete_transaction_has_exact_and_one_short_work_and_storage() {
    let (result, work, storage, calls) = run_case(AMPLE, AMPLE, 0, |_| Ok(()));
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
    let (exact, _, _, _) = run_case(work, storage, 0, |_| Ok(()));
    assert!(exact.is_ok(), "{exact:?}");
    for (w, s) in [(work - 1, storage), (work, storage - 1)] {
        let (result, _, _, _) = run_case(w, s, 0, |_| Ok(()));
        assert!(result.is_err(), "one-short ({w},{s}) admitted");
        assert!(
            std::error::Error::source(result.as_ref().unwrap_err()).is_some(),
            "typed cause lost: {result:?}"
        );
    }
}

#[test]
fn mixed_native_v26_fixed_nine_retain_separate_global_counts() {
    let completed = Cell::new(false);
    let (result, _, _, calls) = run_case(AMPLE, AMPLE, 0, |input| {
        let outcome = crate::canonical_private_v1::run_mixed_v26(
            input,
            crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            None,
        )
        .expect("fixed mixed pipeline");
        let report = &outcome.report;
        assert_eq!(report.paired_stage_count(), 9);
        assert!(report.reports().is_clean());
        assert!(!report.grants_artifact_or_launch_authority());
        for stage in 0..9 {
            assert_eq!(report.global_access_counts(stage), Some([1, 1]));
            assert_eq!(report.private_access_counts(stage), Some([1, 0, 1, 1, 0]));
        }
        assert_eq!(report.global_access_counts(9), None);
        completed.set(true);
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
    assert!(completed.get());
}

#[test]
fn mixed_native_v26_fixed_nine_rejects_missing_duplicate_and_swapped_effect_coverage() {
    for position in 0..9 {
        for kind in 0..9 {
            let reached = Cell::new(false);
            let (result, _, _, calls) =
                run_case(AMPLE, AMPLE, 0, |input| {
                    let (outcome, observed) =
                        crate::canonical_private_v1::test_with_mixed_coverage_fault_v26(
                            position,
                            kind,
                            || {
                                crate::canonical_private_v1::run_mixed_v26(input,
                        crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(), None)
                            },
                        );
                    assert!(outcome.is_err(), "position {position}, fault {kind}");
                    reached.set(observed);
                    Ok(())
                });
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(calls, 1);
            assert!(
                reached.get(),
                "position {position}, fault {kind} was not injected"
            );
        }
    }
}
