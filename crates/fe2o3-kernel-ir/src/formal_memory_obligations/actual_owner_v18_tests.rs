use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, FunctionRole, IntrinsicOperation,
    Kernel, Signature, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1,
    StorageLayoutLimitsV1, StorageLayoutV1, StorageOperationV1, Terminator, ValueDef,
};

fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn fixture(looping: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        value(3, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        value(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        value(
            5,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value(6, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        value(
            7,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut write = BasicBlock::new(BlockId(1));
    write.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(7),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    write.terminator = Some(Terminator::Branch {
        target: BlockId(if looping { 1 } else { 2 }),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("actual_owner_formal");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, write, exit],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn add_metadata(module: &mut Module) {
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
}
fn with_owner<R>(module: &Module, run: impl FnOnce(&VerifiedCanonicalKernelIrModuleV18) -> R) -> R {
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 20_000_000);
    budget.reserve_storage(37).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let result = run(&owner);
    // The new scope is explicitly inert. Unchanged owner ledger does not imply
    // the independent effects/formal allocations have been paid on this ledger.
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    result
}
fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [64, 1, 1],
    }
}

#[test]
fn actual_owner_memory_branch_join_and_loop_preserve_real_conflict_rows() {
    for looping in [false, true] {
        let mut module = fixture(looping);
        let expected = derive_kernel_memory_obligations_for_launch(
            &module,
            &KernelId::new("root"),
            launch(),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(expected.is_complete());
        assert_eq!(expected.obligations().accesses().len(), 1);
        assert_eq!(expected.obligations().inter_invocation_conflicts().len(), 1);
        add_metadata(&mut module);
        assert!(
            verify_module_ref(&module).is_err(),
            "the V18 storage owner is not a legacy token"
        );
        with_owner(&module, |owner| {
            let bytes = owner.canonical_bytes().to_vec();
            let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            let report = scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            assert!(std::ptr::eq(scope.owner(), owner));
            assert!(std::ptr::eq(report.owner(), owner));
            assert_eq!(report.analysis(), &expected);
            assert_eq!(report.launch(), launch());
            assert_eq!(owner.module(), &module);
            assert_eq!(owner.canonical_bytes(), bytes);
        });
    }
}

#[test]
fn storage_operations_retain_every_original_unsupported_effect() {
    let mut module = fixture(false);
    add_metadata(&mut module);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let first = block.operations.len();
    let address = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    block.operations.extend([
        value(
            8,
            address,
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(8),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        value(
            9,
            Type::Scalar(ScalarType::U32),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ]);
    with_owner(&module, |owner| {
        let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
        let report = scope
            .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
            .unwrap();
        let expected: Vec<_> = (first + 1..first + 3)
            .map(
                |operation_index| FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                    location: FunctionOperationLocation::new(BlockId(0), operation_index),
                },
            )
            .collect();
        assert_eq!(report.analysis().incomplete_reasons(), expected);
        assert!(matches!(
            owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[first].kind,
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            }
        ));
        assert!(!report.analysis().is_complete());
        assert_eq!(report.analysis().obligations().accesses().len(), 1);
        assert_eq!(
            report
                .analysis()
                .obligations()
                .inter_invocation_conflicts()
                .len(),
            1
        );
        assert!(report.belongs_to(owner));
    });
}

#[test]
fn pure_helper_calls_and_unavailable_call_effects_reuse_the_real_summary() {
    for external in [false, true] {
        let mut module = fixture(false);
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(
                0,
                Operation::new(
                    vec![],
                    OperationKind::Call {
                        callee: "helper".into(),
                        arguments: vec![],
                    },
                ),
            );
        let helper = if external {
            Function::external_import("helper", Signature::new(vec![], vec![]))
        } else {
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            Function::definition(
                "helper",
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            )
        };
        module.functions.push(helper);
        add_metadata(&mut module);
        with_owner(&module, |owner| {
            let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            let report = scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            let expected = if external {
                vec![FormalMemoryIncompleteReason::CallEffectsUnavailable {
                    location: FunctionOperationLocation::new(BlockId(0), 0),
                    callee: "helper".into(),
                }]
            } else {
                vec![]
            };
            assert_eq!(report.analysis().incomplete_reasons(), expected);
            assert_eq!(report.analysis().is_complete(), !external);
            assert_eq!(
                report
                    .analysis()
                    .obligations()
                    .inter_invocation_conflicts()
                    .len(),
                1
            );
            assert_eq!(
                owner.module().functions[1].role,
                if external {
                    FunctionRole::ExternalImport
                } else {
                    FunctionRole::InternalHelper
                }
            );
        });
    }
}

#[test]
fn unknown_launch_and_index_width_preserve_exact_incomplete_reasons() {
    let mut module = fixture(false);
    add_metadata(&mut module);
    with_owner(&module, |owner| {
        for (extent, width, reason) in [
            (
                ExplicitLaunchExtent::Unknown,
                FormalIndexWidth::Bits64,
                FormalMemoryIncompleteReason::LaunchExtentUnknown,
            ),
            (
                launch(),
                FormalIndexWidth::Bits32,
                FormalMemoryIncompleteReason::UnsupportedIndexWidth {
                    width: FormalIndexWidth::Bits32,
                },
            ),
        ] {
            let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            let report = scope.derive(&KernelId::new("root"), extent, width).unwrap();
            assert_eq!(report.analysis().incomplete_reasons(), [reason]);
            assert!(!report.analysis().is_complete());
        }
    });
}

#[test]
fn equal_canonical_bytes_do_not_make_another_owner_current() {
    let mut module = fixture(false);
    add_metadata(&mut module);
    with_owner(&module, |owner| {
        with_owner(&module, |foreign| {
            assert_eq!(owner.identity(), foreign.identity());
            let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            let report = scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            assert!(report.belongs_to(owner));
            assert!(!report.belongs_to(foreign));
        })
    });
}

#[test]
fn changed_output_gets_a_fresh_report_not_original_owner_evidence() {
    let mut original = fixture(false);
    add_metadata(&mut original);
    let mut changed = original.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .clear();
    with_owner(&original, |owner| {
        with_owner(&changed, |output| {
            let mut source_scope =
                CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            let source = source_scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            assert!(!source.belongs_to(output));
            assert_eq!(
                source
                    .analysis()
                    .obligations()
                    .inter_invocation_conflicts()
                    .len(),
                1
            );
            let mut output_scope =
                CanonicalOwnerFormalScopeV18::new(output, Default::default()).unwrap();
            let fresh = output_scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            assert!(fresh.belongs_to(output));
            assert!(fresh.analysis().is_complete());
            assert!(fresh.analysis().obligations().accesses().is_empty());
        })
    });
}

#[test]
fn exact_and_one_short_whole_transport_and_function_caps_are_independent() {
    let mut module = fixture(false);
    add_metadata(&mut module);
    let mut dead = BasicBlock::new(BlockId(99));
    dead.operations.push(value(
        99,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(123)),
    ));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks.push(dead);
    with_owner(&module, |owner| {
        let bytes = owner.canonical_bytes().len();
        assert!(
            CanonicalOwnerFormalScopeV18::new(
                owner,
                CanonicalOwnerFormalLimitsV18 {
                    canonical_bytes: bytes,
                    functions: 1,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        assert!(
            matches!(CanonicalOwnerFormalScopeV18::new(owner, CanonicalOwnerFormalLimitsV18 { canonical_bytes: bytes - 1, ..Default::default() }), Err(CanonicalOwnerFormalErrorV18::Limit { resource: "canonical bytes", actual, limit }) if actual == bytes && limit == bytes - 1)
        );
        assert!(matches!(
            CanonicalOwnerFormalScopeV18::new(
                owner,
                CanonicalOwnerFormalLimitsV18 {
                    functions: 0,
                    ..Default::default()
                }
            ),
            Err(CanonicalOwnerFormalErrorV18::Limit {
                resource: "functions",
                actual: 1,
                limit: 0
            })
        ));
        assert_eq!(owner.module().storage_layouts.len(), 2);
        assert_eq!(
            owner.module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .len(),
            4
        );
    });
}

#[test]
fn root_query_replays_are_cumulative_and_failed_queries_consume_allowance() {
    let module = fixture(false);
    with_owner(&module, |owner| {
        let mut scope = CanonicalOwnerFormalScopeV18::new(
            owner,
            CanonicalOwnerFormalLimitsV18 {
                queries: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            matches!(scope.derive(&KernelId::new("absent"), launch(), FormalIndexWidth::Bits64), Err(CanonicalOwnerFormalErrorV18::Formal(FormalMemoryObligationError::MissingKernel { kernel })) if kernel.as_str() == "absent")
        );
        assert!(
            scope
                .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap()
                .analysis()
                .is_complete()
        );
        for _ in 0..2 {
            assert!(matches!(
                scope.derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64),
                Err(CanonicalOwnerFormalErrorV18::Limit {
                    resource: "queries",
                    actual: 3,
                    limit: 2
                })
            ));
        }
        let mut zero = CanonicalOwnerFormalScopeV18::new(
            owner,
            CanonicalOwnerFormalLimitsV18 {
                queries: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            zero.derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64),
            Err(CanonicalOwnerFormalErrorV18::Limit {
                resource: "queries",
                actual: 1,
                limit: 0
            })
        ));
    });
}

#[test]
fn oversized_query_identity_refuses_before_allocating_a_missing_root_error() {
    let module = fixture(false);
    with_owner(&module, |owner| {
        let limit = owner.canonical_bytes().len();
        let name = KernelId::new("x".repeat(limit + 1));
        let mut scope = CanonicalOwnerFormalScopeV18::new(
            owner,
            CanonicalOwnerFormalLimitsV18 {
                queries: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            matches!(scope.derive(&name, launch(), FormalIndexWidth::Bits64), Err(CanonicalOwnerFormalErrorV18::Limit { resource: "query identity", actual, limit: found }) if actual == limit + 1 && found == limit)
        );
        assert!(matches!(
            scope.derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64),
            Err(CanonicalOwnerFormalErrorV18::Limit {
                resource: "queries",
                actual: 2,
                limit: 1
            })
        ));
    });
}

#[test]
fn unguarded_many_store_owner_refuses_pair_growth_before_effects_or_formal_extraction() {
    for count in [44usize, 45, 256] {
        let mut module = fixture(false);
        add_metadata(&mut module);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0]
            .operations
            .retain(|op| op.results[0].id.0 >= 5);
        body.blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        body.blocks[1].operations = vec![body.blocks[1].operations[0].clone(); count];
        with_owner(&module, |owner| {
            if count == 44 {
                let mut scope = CanonicalOwnerFormalScopeV18::new(
                    owner,
                    CanonicalOwnerFormalLimitsV18 {
                        candidate_pairs: 990,
                        ..Default::default()
                    },
                )
                .unwrap();
                let report = scope
                    .derive(&KernelId::new("root"), launch(), FormalIndexWidth::Bits64)
                    .unwrap();
                assert!(report.analysis().is_complete());
                assert_eq!(report.analysis().obligations().accesses().len(), 44);
                assert_eq!(
                    report
                        .analysis()
                        .obligations()
                        .inter_invocation_conflicts()
                        .len(),
                    990
                );
                assert!(matches!(
                    CanonicalOwnerFormalScopeV18::new(
                        owner,
                        CanonicalOwnerFormalLimitsV18 {
                            candidate_pairs: 989,
                            ..Default::default()
                        }
                    ),
                    Err(CanonicalOwnerFormalErrorV18::CandidatePairs(
                        FormalMemoryCandidatePairErrorV1::Limit {
                            actual: 990,
                            limit: 989
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    CanonicalOwnerFormalScopeV18::new(
                        owner,
                        CanonicalOwnerFormalLimitsV18 {
                            candidate_pairs: usize::MAX,
                            ..Default::default()
                        }
                    ),
                    Err(CanonicalOwnerFormalErrorV18::CandidatePairs(
                        FormalMemoryCandidatePairErrorV1::Limit {
                            actual: 1035,
                            limit: 1024
                        }
                    ))
                ));
            }
        });
    }
}
