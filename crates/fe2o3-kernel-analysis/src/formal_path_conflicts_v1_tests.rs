use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, FunctionOperationLocation, IntrinsicOperation, Kernel,
    LaunchExtent, MemoryAccess, Module, Signature, SwitchCase, ValueDef, verify_module_ref,
};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn literal(id: u32, value: u64) -> Operation {
    op(
        id,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
    )
}
fn branch(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn fixture(bound: u64) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        literal(3, bound),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        literal(5, 0),
        op(
            6,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            7,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(6),
                offset: ValueId(5),
            },
        ),
    ];
    entry.terminator = Some(branch(4, 1, 2));
    let mut yes = BasicBlock::new(BlockId(1));
    yes.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(7),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("path-conflict-analysis");
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
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
fn analyze(module: &Module, extent: u64) -> FormalPathConflictsV1<'_> {
    analyze_limits(module, extent, Limits::default()).unwrap()
}
fn analyze_limits(
    module: &Module,
    extent: u64,
    limits: Limits,
) -> Result<FormalPathConflictsV1<'_>> {
    FormalPathConflictsV1::derive(
        verify_module_ref(module).expect("genuine verified source"),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(extent),
        FormalIndexWidth::Bits64,
        limits,
    )
}
fn singleton_decision<'r>(report: &'r FormalPathConflictsV1<'_>) -> &'r Decision {
    assert!(report.formal().is_complete(), "{:?}", report.formal());
    assert_eq!(
        report
            .formal()
            .obligations()
            .inter_invocation_conflicts()
            .len(),
        1
    );
    let [decision] = report.decisions() else {
        panic!("one exact original conflict");
    };
    decision
}
fn assert_possible(report: &FormalPathConflictsV1<'_>) {
    assert!(
        matches!(singleton_decision(report), Decision::PossibleOverlap { left, right } if left != right)
    );
}

#[test]
fn singleton_path_proof_retains_the_exact_fresh_full_witness_report() {
    let module = fixture(1);
    let owner = verify_module_ref(&module).unwrap();
    let fresh = derive_kernel_memory_obligations_from_verified(
        owner,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    let report = analyze(&module, 64);
    assert_eq!(singleton_decision(&report), &Decision::Disjoint);
    assert_eq!(report.formal(), &fresh);
    assert_eq!(report.queries(), 2);
    assert!(report.belongs_to(owner));
    let [access] = report.formal().obligations().accesses() else {
        panic!()
    };
    assert_eq!(access.invocations().start(), 0);
    assert_eq!(access.invocations().end_exclusive(), 64);
    assert_eq!(
        access.location(),
        FunctionOperationLocation::new(BlockId(1), 0)
    );
    assert_eq!(access.byte_offset(), ByteExpression::constant(0));
}

#[test]
fn two_writers_and_unknown_runtime_guards_retain_the_conflict() {
    assert_possible(&analyze(&fixture(2), 64));
    let mut module = fixture(1);
    blocks(&mut module)[0].operations[1] = op(
        3,
        Type::INDEX,
        OperationKind::SliceLength { slice: ValueId(0) },
    );
    assert_possible(&analyze(&module, 64));
}

#[test]
fn separate_single_writer_domains_still_conflict_across_distinct_invocations() {
    let mut module = fixture(1);
    let blocks = blocks(&mut module);
    let first_store = blocks[1].operations[0].clone();
    blocks[2].operations.extend([
        literal(8, 1),
        op(
            9,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
    ]);
    blocks[2].terminator = Some(branch(9, 3, 4));
    let mut second = BasicBlock::new(BlockId(3));
    second.operations.push(first_store);
    second.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(4));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    blocks.extend([second, exit]);
    let report = analyze(&module, 64);
    assert!(report.formal().is_complete());
    let conflicts = report.formal().obligations().inter_invocation_conflicts();
    assert_eq!(conflicts.len(), 3);
    for (conflict, decision) in conflicts.iter().zip(report.decisions()) {
        if conflict.left() == conflict.right() {
            assert_eq!(decision, &Decision::Disjoint);
        } else {
            assert!(matches!(
                decision,
                Decision::PossibleOverlap { left: 0, right: 1 }
            ));
        }
    }
}

#[test]
fn no_user_name_or_nonzero_constant_address_changes_the_path_proof() {
    let mut module = fixture(1);
    module.functions[0].id = "ordinary_renamed_entry".into();
    module.kernels[0].entry = "ordinary_renamed_entry".into();
    blocks(&mut module)[0].operations[3] = literal(5, 11);
    let report = analyze(&module, 64);
    assert_eq!(singleton_decision(&report), &Decision::Disjoint);
    assert_eq!(
        report.formal().obligations().accesses()[0].byte_offset(),
        ByteExpression::constant(44)
    );
}

#[test]
fn complementary_branch_and_every_comparison_polarity_are_respected() {
    let cases = [
        (ComparePredicate::Equal, 7, true, true),
        (ComparePredicate::Equal, 7, false, false),
        (ComparePredicate::NotEqual, 7, false, true),
        (ComparePredicate::NotEqual, 7, true, false),
        (ComparePredicate::LessThan, 1, true, true),
        (ComparePredicate::LessThan, 63, false, true),
        (ComparePredicate::LessThanOrEqual, 0, true, true),
        (ComparePredicate::LessThanOrEqual, 62, false, true),
        (ComparePredicate::GreaterThan, 62, true, true),
        (ComparePredicate::GreaterThan, 0, false, true),
        (ComparePredicate::GreaterThanOrEqual, 63, true, true),
        (ComparePredicate::GreaterThanOrEqual, 1, false, true),
    ];
    for (predicate, bound, truth, disjoint) in cases {
        let mut module = fixture(bound);
        blocks(&mut module)[0].operations[2].kind = OperationKind::Compare {
            predicate,
            lhs: ValueId(2),
            rhs: ValueId(3),
        };
        if !truth {
            blocks(&mut module)[0].terminator = Some(branch(4, 2, 1));
        }
        let report = analyze(&module, 64);
        assert_eq!(
            matches!(singleton_decision(&report), Decision::Disjoint),
            disjoint,
            "{predicate:?} {truth}"
        );
    }
}

#[test]
fn bool_not_and_exact_zero_extended_switch_carriers_preserve_domains() {
    for switching in [false, true] {
        let mut module = fixture(1);
        let entry = &mut blocks(&mut module)[0];
        entry.operations.push(op(
            8,
            Type::BOOL,
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(4),
            },
        ));
        entry.terminator = Some(if switching {
            entry.operations.push(op(
                9,
                Type::Scalar(ScalarType::U64),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(8),
                    to: Type::Scalar(ScalarType::U64),
                },
            ));
            Terminator::Switch {
                selector: ValueId(9),
                cases: vec![SwitchCase {
                    value: 1,
                    target: BlockId(2),
                    arguments: vec![],
                }],
                default_target: BlockId(1),
                default_arguments: vec![],
            }
        } else {
            branch(8, 2, 1)
        });
        assert_eq!(
            singleton_decision(&analyze(&module, 64)),
            &Decision::Disjoint
        );
    }
}

#[test]
fn duplicate_targets_bypass_and_reconverged_paths_do_not_supply_edge_dominance() {
    for mode in 0..3 {
        let mut module = fixture(1);
        let blocks = blocks(&mut module);
        if mode == 0 {
            blocks[0].terminator = Some(branch(4, 1, 1));
        }
        if mode == 1 {
            blocks[2].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
        }
        if mode == 2 {
            let store = blocks[1].operations.pop().unwrap();
            blocks[1].terminator = Some(Terminator::Branch {
                target: BlockId(3),
                arguments: vec![],
            });
            blocks[2].terminator = Some(Terminator::Branch {
                target: BlockId(3),
                arguments: vec![],
            });
            let mut join = BasicBlock::new(BlockId(3));
            join.operations.push(store);
            join.terminator = Some(Terminator::Return { values: vec![] });
            blocks.push(join);
        }
        assert_possible(&analyze(&module, 64));
    }
}

#[test]
fn nested_independent_guards_intersect_without_importing_a_join_predicate() {
    let mut module = fixture(8);
    let blocks = blocks(&mut module);
    let store = blocks[1].operations.pop().unwrap();
    blocks[1].operations.extend([
        literal(8, 7),
        op(
            9,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::GreaterThanOrEqual,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
    ]);
    blocks[1].terminator = Some(branch(9, 3, 2));
    let mut inner = BasicBlock::new(BlockId(3));
    inner.operations.push(store);
    inner.terminator = Some(Terminator::Return { values: vec![] });
    blocks.push(inner);
    assert_eq!(
        singleton_decision(&analyze(&module, 64)),
        &Decision::Disjoint
    );
}

#[test]
fn invariant_guard_covers_loop_body_without_claiming_loop_termination() {
    let mut module = fixture(1);
    let blocks = blocks(&mut module);
    let store = blocks[1].operations.pop().unwrap();
    blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut looping = BasicBlock::new(BlockId(3));
    looping.operations.push(store);
    looping.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    blocks.push(looping);
    assert_eq!(
        singleton_decision(&analyze(&module, 64)),
        &Decision::Disjoint
    );
}

#[test]
fn local_invocation_index_cannot_stand_in_for_global_invocation_identity() {
    let mut module = fixture(1);
    blocks(&mut module)[0].operations[0].kind = OperationKind::Intrinsic(IntrinsicOperation::new(
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local,
            axis: Axis::X,
        },
        Type::INDEX,
    ));
    assert_possible(&analyze(&module, 128));
}

#[test]
fn affine_arithmetic_uses_whole_launch_no_wrap_bounds() {
    for binary in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
        let mut module = fixture(1);
        let entry = &mut blocks(&mut module)[0];
        let compare = entry.operations.remove(2);
        entry
            .operations
            .push(literal(8, if binary == BinaryOp::Subtract { 0 } else { 1 }));
        entry.operations.push(op(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: binary,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ));
        let mut compare = compare;
        compare.kind = OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(9),
            rhs: ValueId(3),
        };
        if binary == BinaryOp::Add {
            entry.operations[1] = literal(3, 2);
        }
        entry.operations.push(compare);
        assert_eq!(
            singleton_decision(&analyze(&module, 64)),
            &Decision::Disjoint
        );
    }
}

#[test]
fn wrapping_and_underflowing_expressions_cannot_narrow_the_domain() {
    for binary in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
        let mut module = fixture(1);
        let entry = &mut blocks(&mut module)[0];
        entry.operations.remove(2);
        entry.operations.push(literal(
            8,
            if binary == BinaryOp::Subtract {
                1
            } else {
                u64::MAX
            },
        ));
        entry.operations.push(op(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: binary,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ));
        entry.operations.push(op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(9),
                rhs: ValueId(3),
            },
        ));
        assert_possible(&analyze(&module, 64));
    }
}

#[test]
fn exact_unsigned_index_representation_bridge_is_not_signed_reinterpretation() {
    let mut module = fixture(1);
    let entry = &mut blocks(&mut module)[0];
    entry.operations.remove(2);
    entry.operations.push(op(
        8,
        Type::Scalar(ScalarType::U64),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(2),
            to: Type::Scalar(ScalarType::U64),
        },
    ));
    entry.operations.push(op(
        9,
        Type::Scalar(ScalarType::U64),
        OperationKind::Constant(Constant::U64(1)),
    ));
    entry.operations.push(op(
        4,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(8),
            rhs: ValueId(9),
        },
    ));
    assert_eq!(
        singleton_decision(&analyze(&module, 64)),
        &Decision::Disjoint
    );
}

#[test]
fn boolean_conjunction_and_false_disjunction_keep_only_necessary_facts() {
    for disjunction in [false, true] {
        let mut module = fixture(1);
        let entry = &mut blocks(&mut module)[0];
        entry.operations.push(op(
            8,
            Type::BOOL,
            OperationKind::Constant(Constant::Bool(!disjunction)),
        ));
        let lhs = if disjunction {
            entry.operations.push(op(
                9,
                Type::BOOL,
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand: ValueId(4),
                },
            ));
            ValueId(9)
        } else {
            ValueId(4)
        };
        entry.operations.push(op(
            10,
            Type::BOOL,
            OperationKind::Binary {
                op: if disjunction {
                    BinaryOp::BitOr
                } else {
                    BinaryOp::BitAnd
                },
                lhs,
                rhs: ValueId(8),
            },
        ));
        entry.terminator = Some(if disjunction {
            branch(10, 2, 1)
        } else {
            branch(10, 1, 2)
        });
        assert_eq!(
            singleton_decision(&analyze(&module, 64)),
            &Decision::Disjoint
        );
    }
}

#[test]
fn constant_unreachable_path_is_empty_but_dynamic_phi_is_not_assumed_constant() {
    let mut module = fixture(1);
    blocks(&mut module)[0].operations[2] = op(
        4,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(false)),
    );
    assert_eq!(
        singleton_decision(&analyze(&module, 64)),
        &Decision::Disjoint
    );
    let mut module = fixture(1);
    let blocks = blocks(&mut module);
    let store = blocks[1].operations.pop().unwrap();
    blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(4)],
    });
    blocks[2].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(4)],
    });
    let mut join = BasicBlock::new(BlockId(3));
    join.parameters.push(ValueDef::new(ValueId(8), Type::BOOL));
    join.terminator = Some(branch(8, 4, 5));
    let mut write = BasicBlock::new(BlockId(4));
    write.operations.push(store);
    write.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(5));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    blocks.extend([join, write, exit]);
    assert_possible(&analyze(&module, 64));
}

#[test]
fn equal_bytes_in_a_foreign_owner_do_not_supply_source_custody() {
    let module = fixture(1);
    let clone = module.clone();
    let report = analyze(&module, 64);
    assert!(!report.belongs_to(verify_module_ref(&clone).unwrap()));
    assert!(std::ptr::eq(report.owner().module(), &module));
}

#[test]
fn transformed_owner_must_rederive_domains_after_predicate_changes() {
    let original = fixture(1);
    let optimized = fixture(2);
    let report = analyze(&original, 64);
    assert_eq!(singleton_decision(&report), &Decision::Disjoint);
    assert!(!report.belongs_to(verify_module_ref(&optimized).unwrap()));
    assert_possible(&analyze(&optimized, 64));
}

#[test]
fn dynamic_ranked_and_empty_launches_are_explicitly_unsupported() {
    let mut module = fixture(1);
    for extent in [
        ExplicitLaunchExtent1d::Unknown,
        ExplicitLaunchExtent1d::Exact(0),
    ] {
        assert!(matches!(
            FormalPathConflictsV1::derive(
                verify_module_ref(&module).unwrap(),
                &KernelId::new("kernel"),
                extent,
                FormalIndexWidth::Bits64,
                Limits::default()
            ),
            Err(Error::UnsupportedLaunch)
        ));
    }
    module.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Dynamic,
        y: LaunchExtent::Dynamic,
    };
    assert!(matches!(
        analyze_limits(&module, 64, Limits::default()),
        Err(Error::UnsupportedLaunch)
    ));
}

#[test]
fn incomplete_formal_source_cannot_acquire_discharge_decisions() {
    let module = fixture(1);
    let report = FormalPathConflictsV1::derive(
        verify_module_ref(&module).unwrap(),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits32,
        Limits::default(),
    )
    .unwrap();
    assert!(!report.formal().is_complete());
    assert!(
        report
            .decisions()
            .iter()
            .all(|d| *d == Decision::IncompleteFormalReport)
    );
    assert_eq!(report.queries(), 0);
}

#[test]
fn every_local_resource_cap_fails_closed_without_a_partial_report() {
    let module = fixture(1);
    for resource in [
        Resource::SourceItems,
        Resource::ConstructionSteps,
        Resource::Conflicts,
        Resource::Constraints,
        Resource::Queries,
    ] {
        let mut limits = Limits::default();
        match resource {
            Resource::SourceItems => limits.source_items = 0,
            Resource::ConstructionSteps => limits.construction_steps = 0,
            Resource::Conflicts => limits.conflicts = 0,
            Resource::Constraints => limits.constraints_per_access = 0,
            Resource::Queries => limits.queries = 0,
        }
        assert!(
            matches!(analyze_limits(&module, 64, limits), Err(Error::Limit { resource: found, actual: 1, limit: 0 }) if found == resource)
        );
    }
}

#[test]
fn exact_and_one_short_construction_and_query_budgets_preserve_boundaries() {
    let module = fixture(1);
    let measured = analyze(&module, 64);
    let work = measured.construction_steps();
    assert!(work > 1);
    for short in [false, true] {
        let limits = Limits {
            construction_steps: work - usize::from(short),
            queries: 2,
            ..Limits::default()
        };
        let result = analyze_limits(&module, 64, limits);
        if short {
            assert!(
                matches!(result, Err(Error::Limit { resource: Resource::ConstructionSteps, actual, limit }) if actual == work && limit == work - 1)
            );
        } else {
            assert_eq!(singleton_decision(&result.unwrap()), &Decision::Disjoint);
        }
    }
    assert!(matches!(
        analyze_limits(
            &module,
            64,
            Limits {
                queries: 1,
                ..Limits::default()
            }
        ),
        Err(Error::Limit {
            resource: Resource::Queries,
            actual: 2,
            limit: 1
        })
    ));
}

#[test]
fn presburger_resource_exhaustion_is_not_an_empty_relation() {
    let module = fixture(1);
    let report = analyze(&module, 2_000_000);
    assert!(
        matches!(singleton_decision(&report), Decision::IncompletePresburger(PresburgerFailureV1::ResourceLimit { limit: crate::MAX_PRESBURGER_WORK_UNITS_V1, actual }) if *actual == crate::MAX_PRESBURGER_WORK_UNITS_V1 + 1)
    );
}

#[test]
fn finite_exhaustive_guard_oracle_checks_every_small_extent_and_boundary() {
    for extent in 2..10 {
        for bound in 0..=extent + 1 {
            let module = fixture(bound);
            let report = analyze(&module, extent);
            let actual_writers: Vec<_> = (0..extent).filter(|index| *index < bound).collect();
            assert_eq!(
                matches!(singleton_decision(&report), Decision::Disjoint),
                actual_writers.len() <= 1,
                "extent {extent}, bound {bound}"
            );
        }
    }
}

#[test]
fn unguarded_many_store_path_refuses_pair_growth_before_fresh_formal_extraction() {
    for count in [44usize, 45, 256] {
        let mut module = fixture(1);
        let body = blocks(&mut module);
        body[0].operations.retain(|op| op.results[0].id.0 >= 5);
        body[0].terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        body[1].operations = vec![body[1].operations[0].clone(); count];
        if count == 44 {
            let report = analyze_limits(
                &module,
                2,
                Limits {
                    conflicts: 990,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(report.formal().is_complete());
            assert_eq!(report.formal().obligations().accesses().len(), 44);
            assert_eq!(
                report
                    .formal()
                    .obligations()
                    .inter_invocation_conflicts()
                    .len(),
                990
            );
            assert!(
                report
                    .decisions()
                    .iter()
                    .all(|decision| matches!(decision, Decision::PossibleOverlap { .. }))
            );
            assert!(matches!(
                analyze_limits(
                    &module,
                    2,
                    Limits {
                        conflicts: 989,
                        ..Default::default()
                    }
                ),
                Err(Error::Limit {
                    resource: Resource::Conflicts,
                    actual: 990,
                    limit: 989
                })
            ));
        } else {
            assert!(matches!(
                analyze_limits(
                    &module,
                    2,
                    Limits {
                        conflicts: usize::MAX,
                        ..Default::default()
                    }
                ),
                Err(Error::Limit {
                    resource: Resource::Conflicts,
                    actual: 1035,
                    limit: 1024
                })
            ));
        }
    }
}
