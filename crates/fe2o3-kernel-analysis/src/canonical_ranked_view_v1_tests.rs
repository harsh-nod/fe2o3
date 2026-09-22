use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, Function, IntegerSwitchCase, Kernel, LaunchDomain, LaunchExtent, MemoryAccess,
    Module, Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type,
    ValueDef, ValueId,
};

pub(super) fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}

pub(super) fn noop() -> Module {
    let mut block = BasicBlock::new(BlockId(900));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("ranked-noop");
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

pub(super) fn mixed() -> Module {
    let scalar = Type::Scalar(ScalarType::U64);
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(80),
                Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(80),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(81), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(80),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(82), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(83), Type::Scalar(ScalarType::F32)),
            OperationKind::Constant(Constant::F32Bits(0x8000_0000)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(84), Type::Scalar(ScalarType::F32)),
            OperationKind::Constant(Constant::F32Bits(0x7fc0_0021)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(85), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(81),
                rhs: ValueId(2),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(86), Type::Scalar(ScalarType::F32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(83),
                rhs: ValueId(84),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(82),
        then_target: BlockId(50),
        then_arguments: vec![ValueId(81)],
        else_target: BlockId(50),
        else_arguments: vec![ValueId(85)],
    });
    let mut join = BasicBlock::new(BlockId(50));
    join.parameters
        .push(ValueDef::new(ValueId(99), scalar.clone()));
    join.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(99),
        cases: vec![IntegerSwitchCase {
            value: Constant::U64(0),
            target: BlockId(50),
            arguments: vec![ValueId(99)],
        }],
        default_target: BlockId(7),
        default_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(7));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("ranked-mixed");
    // Physical order after the entry is deliberately opposite execution order.
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![scalar], vec![]),
        vec![ValueId(2)],
        vec![entry, exit, join],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

/// Input accounting is child-contract composition, not a claim about source/rustc.
pub(super) fn with_fixture<T>(
    module: &Module,
    run: impl FnOnce(&Inventory<'_>, &mut Budget<'_>) -> T,
) -> T {
    let (owner, owner_bytes) = admit(module);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(owner_bytes).unwrap();
    let (inventory, receipt) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let result = run(&inventory, &mut budget);
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(owner_bytes).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

pub(super) fn candidate<'i, 'g, 'm>(
    inventory: &'i Inventory<'g>,
    metadata: &'i Metadata<'g, 'm>,
    budget: &mut Budget<'_>,
) -> (Candidate<'i, 'g, 'm>, usize) {
    let metadata_bytes = metadata.storage_extent(budget).unwrap();
    budget.reserve_storage(metadata_bytes).unwrap();
    let (candidate, receipt) =
        build_canonical_ranked_candidate_v1(inventory, metadata, budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    (candidate, receipt.retained_storage() + metadata_bytes)
}

#[test]
fn canonical_ranked_noop_has_literal_complete_rows_and_only_pending_obligations() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let expected = [
            row(Subject::Module, Role::Module, Obligations::NONE),
            row(
                Subject::Kernel(0),
                Role::Kernel,
                Obligations::NONE.with(Obligation::Launch),
            ),
            row(
                Subject::Function(0),
                Role::DefinedFunction,
                Obligations::NONE,
            ),
            row(
                Subject::Block(0),
                Role::Block(TerminatorClass::Return),
                Obligations::NONE.with(Obligation::Control),
            ),
        ];
        assert_eq!(candidate.rows(), &expected);
        let before = budget.storage();
        with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |view, budget| {
                assert_eq!(view.row_count(budget)?, 4);
                for (i, expected) in expected.iter().enumerate() {
                    assert_eq!(view.row(i, budget)?, expected);
                }
                Ok::<_, Error>(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), before);
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_mixed_edges_cyclic_cfg_private_effects_and_ieee_bits_are_original() {
    with_fixture(&mixed(), |inventory, budget| {
        assert_eq!(inventory.edges().len(), 4);
        assert_eq!(inventory.edge_arguments().len(), 3);
        assert_eq!(inventory.effects().len(), 3);
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let edge_rows: Vec<_> = candidate
            .rows()
            .iter()
            .filter_map(|r| match r.role {
                Role::Edge(kind) => Some(kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            edge_rows,
            [
                EdgeClass::True,
                EdgeClass::False,
                EdgeClass::IntegerCase(0),
                EdgeClass::IntegerDefault
            ]
        );
        with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |view, budget| {
                assert_eq!(view.edge(0, budget)?.arguments, &[ValueId(81)]);
                assert_eq!(view.edge(1, budget)?.arguments, &[ValueId(85)]);
                let first_target = view.edge_argument(0, budget)?.target_definition;
                let second_target = view.edge_argument(1, budget)?.target_definition;
                assert_eq!(first_target, second_target);
                for i in 0..8 {
                    assert!(std::ptr::eq(
                        view.operation(i, budget)?,
                        inventory.operations()[i].operation
                    ));
                }
                assert_eq!(
                    view.operation(4, budget)?.kind,
                    OperationKind::Constant(Constant::F32Bits(0x8000_0000))
                );
                assert_eq!(
                    view.operation(5, budget)?.kind,
                    OperationKind::Constant(Constant::F32Bits(0x7fc0_0021))
                );
                assert_eq!(
                    view.operation(7, budget)?.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(83),
                        rhs: ValueId(84)
                    }
                );
                Ok::<_, Error>(())
            },
        )
        .unwrap();
        let binary = candidate
            .rows()
            .iter()
            .find(|r| r.subject == Subject::Operation(6))
            .unwrap();
        assert!(
            binary
                .obligations
                .contains(Obligation::ExactScalarSemantics)
        );
        assert!(binary.obligations.contains(Obligation::TrapBehavior));
        for r in candidate
            .rows()
            .iter()
            .filter(|r| matches!(r.subject, Subject::Effect(_)))
        {
            assert!(r.obligations.contains(Obligation::Provenance));
            assert!(r.obligations.contains(Obligation::Lifetime));
        }
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_all_requirement_owners_and_metadata_claims_stay_pending() {
    let mut module = mixed();
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    with_fixture(&module, |inventory, budget| {
        let facts = [Fact::Identity([0; 32]), Fact::Unsigned(64)];
        let annotations = [CanonicalRankedMetadataRowV1 {
            subject: Subject::Kernel(0),
            kind: CanonicalRankedMetadataKindV1::Launch,
            facts: &facts,
        }];
        let metadata = Metadata::new(inventory.owner(), &annotations);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let owners: Vec<_> = candidate
            .rows()
            .iter()
            .filter_map(|r| match r.subject {
                Subject::Requirement { owner, .. } => Some(owner),
                _ => None,
            })
            .collect();
        assert!(owners.contains(&RequirementOwner::Module));
        assert!(owners.contains(&RequirementOwner::Kernel(0)));
        assert!(owners.contains(&RequirementOwner::Function(0)));
        with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |view, budget| {
                // Even a zero identity remains an INERT claim, never source authority.
                assert_eq!(view.metadata(0, budget)?.facts, &facts);
                let last = view.row_count(budget)? - 1;
                assert!(
                    view.row(last, budget)?
                        .obligations
                        .contains(Obligation::SourceMetadata)
                );
                Ok::<_, Error>(())
            },
        )
        .unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_calls_and_declarations_do_not_become_pure_from_empty_local_effects() {
    let mut module = noop();
    module.functions.insert(
        0,
        Function::external_import("external", Signature::new(vec![], vec![])),
    );
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "external".into(),
                arguments: vec![],
            },
        ));
    with_fixture(&module, |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        assert!(inventory.effects().is_empty());
        for subject in [
            Subject::Function(0),
            Subject::Operation(0),
            Subject::Call(0),
        ] {
            let row = candidate
                .rows()
                .iter()
                .find(|row| row.subject == subject)
                .unwrap();
            assert!(row.obligations.contains(Obligation::CallEffects));
            assert!(row.obligations.contains(Obligation::CallControl));
        }
        with_checked_canonical_ranked_view_v1(inventory, &metadata, &candidate, budget, |_, _| {
            Ok::<_, Error>(())
        })
        .unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_noop_builder_has_independent_atomic_work_and_storage_oracle() {
    with_fixture(&noop(), |inventory, outer| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let floor = outer.storage() + size_of::<Metadata<'_, '_>>();
        let mut reference = Vec::<Row>::new();
        reference.try_reserve_exact(4).unwrap();
        let retained = size_of::<Candidate<'_, '_, '_>>() + reference.capacity() * size_of::<Row>();
        drop(reference);
        // Nine single visits, one four-row allocation charge, eight single visits.
        let charges: Vec<_> = [vec![1usize; 9], vec![4], vec![1usize; 8]].concat();
        assert_eq!(charges.iter().sum::<usize>(), 21);
        for limit in 0..=21 {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, floor + retained);
            budget.reserve_storage(floor).unwrap();
            let result = build_canonical_ranked_candidate_v1(inventory, &metadata, &mut budget);
            let mut accepted = 0;
            let rejected = charges.iter().find_map(|charge| {
                if accepted + charge > limit {
                    Some(accepted + charge)
                } else {
                    accepted += charge;
                    None
                }
            });
            assert_eq!(budget.work(), accepted);
            assert_eq!(budget.storage(), floor);
            if let Some(actual) = rejected {
                match result {
                    Err(Error::Resource(Resource::Work(error))) => {
                        assert_eq!(error.actual(), actual);
                        assert_eq!(error.limit(), limit);
                    }
                    _ => panic!("expected exact work denial"),
                }
            } else {
                let (candidate, receipt) = result.unwrap();
                budget.reserve_storage(retained).unwrap();
                assert_eq!(receipt.retained_storage(), retained);
                assert_eq!(candidate.rows.len(), 4);
                drop(candidate);
                budget.release_storage(retained).unwrap();
            }
        }
        let mut work = Work::new(21);
        let mut budget = Budget::new(&mut work, floor + retained - 1);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&budget);
        let Err(Error::Resource(Resource::Storage(error))) =
            build_canonical_ranked_candidate_v1(inventory, &metadata, &mut budget)
        else {
            panic!("expected exact candidate payload storage denial");
        };
        assert_eq!(error.actual(), floor + retained);
        assert_eq!(error.limit(), floor + retained - 1);
        assert_eq!(budget.storage_limit(), floor + retained - 1);
        assert_eq!(budget.storage(), floor);
        // The header reservation precedes the rejected payload reservation.
        assert_eq!(
            budget.peak_storage(),
            floor + size_of::<Candidate<'_, '_, '_>>()
        );
        assert_eq!(budget.failed_storage(), Some(floor + retained));
        assert_eq!(budget.work(), 9);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(std::ptr::eq(std::ptr::from_ref(&budget), slot));
        drop(budget);
        assert_eq!(work.limit(), 21);
        assert_eq!(work.failed_work(), None);
    });
}

#[test]
fn canonical_ranked_scope_oracle_exact_work_storage_and_repeated_constant_cost_queries() {
    with_fixture(&noop(), |inventory, outer| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, outer);
        let floor = outer.storage();
        let headers = size_of::<control::Accounting>()
            + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
        for (work_limit, storage_limit, success) in [
            (13, floor + headers, true),
            (12, floor + headers, false),
            (13, floor + headers - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget);
            let result = with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &candidate,
                &mut budget,
                |view, budget| {
                    assert_eq!(view.row_count(budget)?, 4);
                    Ok::<_, Error>(())
                },
            );
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.storage_limit(), storage_limit);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(std::ptr::eq(std::ptr::from_ref(&budget), slot));
            if success {
                assert_eq!(budget.work(), 13);
                assert_eq!(budget.peak_storage(), floor + headers);
                assert_eq!(budget.failed_storage(), None);
            } else if work_limit == 12 {
                let Err(Error::Resource(Resource::Work(error))) = result else {
                    panic!("expected exact checked-view query work denial");
                };
                assert_eq!(error.actual(), 13);
                assert_eq!(error.limit(), 12);
                assert_eq!(budget.work(), 12);
                assert_eq!(budget.peak_storage(), floor + headers);
                assert_eq!(budget.failed_storage(), None);
            } else {
                let Err(Error::Resource(Resource::Storage(error))) = result else {
                    panic!("expected exact checked-view guard storage denial");
                };
                assert_eq!(error.actual(), floor + headers);
                assert_eq!(error.limit(), floor + headers - 1);
                assert_eq!(budget.work(), 2);
                assert_eq!(budget.peak_storage(), floor);
                assert_eq!(budget.failed_storage(), Some(floor + headers));
            }
            drop(budget);
            assert_eq!(work.limit(), work_limit);
            assert_eq!(
                work.failed_work(),
                if work_limit == 12 { Some(13) } else { None }
            );
        }
        with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            outer,
            |view, budget| {
                let before = budget.work();
                for _ in 0..1024 {
                    assert_eq!(view.row_count(budget)?, 4);
                }
                assert_eq!(budget.work() - before, 1024);
                Ok::<_, Error>(())
            },
        )
        .unwrap();
        drop(candidate);
        outer.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_other_terminators_retain_switch_defaults_and_abnormal_exits() {
    let mut module = mixed();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(50),
        arguments: vec![ValueId(85)],
    });
    body.blocks[1].terminator = Some(Terminator::Unreachable);
    body.blocks[2].terminator = Some(Terminator::Switch {
        selector: ValueId(99),
        cases: vec![fe2o3_kernel_ir::SwitchCase {
            value: 0,
            target: BlockId(50),
            arguments: vec![ValueId(99)],
        }],
        default_target: BlockId(7),
        default_arguments: vec![],
    });
    with_fixture(&module, |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let edges: Vec<_> = candidate
            .rows()
            .iter()
            .filter_map(|row| match row.role {
                Role::Edge(edge) => Some(edge),
                _ => None,
            })
            .collect();
        assert_eq!(
            edges,
            [
                EdgeClass::Branch,
                EdgeClass::SwitchCase(0),
                EdgeClass::SwitchDefault
            ]
        );
        let trap = candidate
            .rows()
            .iter()
            .find(|r| r.subject == Subject::Block(1))
            .unwrap();
        assert_eq!(trap.role, Role::Block(TerminatorClass::Unreachable));
        assert!(trap.obligations.contains(Obligation::TrapBehavior));
        with_checked_canonical_ranked_view_v1(inventory, &metadata, &candidate, budget, |_, _| {
            Ok::<_, Error>(())
        })
        .unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_nonlexical_roots_and_bounded_long_reverse_chain_use_actual_inventory() {
    let mut module = noop();
    let mut blocks = Vec::new();
    for i in 0..128 {
        let mut block = BasicBlock::new(BlockId(4_000_000_000 - i));
        block.terminator = Some(if i == 127 {
            Terminator::Return { values: vec![] }
        } else {
            Terminator::Branch {
                target: BlockId(4_000_000_000 - i - 1),
                arguments: vec![],
            }
        });
        blocks.push(block);
    }
    blocks[1..].reverse();
    module.functions.push(Function::kernel_entry(
        "a_first",
        Signature::new(vec![], vec![]),
        vec![],
        blocks,
    ));
    module.kernels.insert(
        0,
        Kernel::new(
            "a_first",
            "a_first",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ),
    );
    with_fixture(&module, |inventory, budget| {
        assert_eq!(inventory.kernels()[0].entry.0, 1);
        assert_eq!(inventory.kernels()[1].entry.0, 0);
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let snapshot = candidate.rows().to_vec();
        with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |view, budget| {
                assert!(std::ptr::eq(view.inventory(budget)?, inventory));
                assert_eq!(view.inventory(budget)?.edges().len(), 127);
                Ok::<_, Error>(())
            },
        )
        .unwrap();
        let (again, receipt) =
            build_canonical_ranked_candidate_v1(inventory, &metadata, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(snapshot, again.rows());
        // Observe no second graph, recursive CFG walk or helper expansion.
        assert_eq!(
            receipt.retained_storage(),
            again.retained_storage().unwrap()
        );
        drop(again);
        budget.release_storage(receipt.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_atomic_context_capability_and_barrier_convergence_are_not_omitted() {
    use fe2o3_kernel_ir::{
        Atomic, AtomicKind, BarrierSemantics, Convergence, MemoryOrdering, SynchronizationScope,
        WorkgroupBarrier,
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), scalar),
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Load,
                pointer: ValueId(0),
                value: None,
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Acquire,
                failure_ordering: None,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("ranked-atomic");
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    with_fixture(&module, |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let base = budget.storage();
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let atomic = candidate
            .rows()
            .iter()
            .position(|row| {
                row.subject
                    == Subject::Requirement {
                        owner: RequirementOwner::AtomicPointer(0),
                        ordinal: 0,
                    }
            })
            .unwrap();
        assert!(candidate.rows().iter().any(|row| matches!(
            row.subject,
            Subject::Requirement {
                owner: RequirementOwner::Operation(1),
                ..
            }
        )));
        let barrier = candidate
            .rows()
            .iter()
            .find(|row| row.subject == Subject::Operation(1))
            .unwrap();
        assert!(barrier.obligations.contains(Obligation::Convergence));
        assert!(barrier.obligations.contains(Obligation::Ordering));
        with_checked_canonical_ranked_view_v1(inventory, &metadata, &candidate, budget, |_, _| {
            Ok::<_, Error>(())
        })
        .unwrap();
        let mut bad = candidate.rows().to_vec();
        bad.remove(atomic);
        let donor = Candidate::from_rows(inventory, &metadata, bad);
        let mut work = Work::new(100_000);
        let mut child = Budget::new(&mut work, 1_000_000);
        let input =
            donor.retained_storage().unwrap() + metadata.storage_extent(&mut child).unwrap();
        child.reserve_storage(base + input).unwrap();
        assert!(matches!(
            with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &donor,
                &mut child,
                |_, _| Ok::<_, Error>(())
            ),
            Err(Error::MismatchedRow { .. })
        ));
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}
