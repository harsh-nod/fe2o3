use super::*;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity, Atomic,
    AtomicKind, BarrierSemantics, CastKind, Convergence, CopyNonOverlappingContract,
    InlineAssembly, InlineAssemblyTarget, Kernel, LaunchDomain, LaunchExtent, MemoryAccess,
    MemoryElementType, MemoryIntrinsicOperation, MemoryOrdering, SynchronizationScope,
    WorkgroupBarrier,
};

fn load(result: u32, address: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(address),
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        },
    )
}

fn with_report(
    source: Module,
    inspect: impl FnOnce(&CanonicalKirPhysicalOccurrencesV1<'_, '_>, &mut Budget<'_>),
) {
    with_checked(source, &[], &[], |inventory, checked, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        let (report, storage) =
            CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        inspect(&report, &mut budget);
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn diamond_and_select_forwarding_require_one_grounded_allocation() {
    for different in [false, true] {
        let mut entry = block(70, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = conditional(18, 0, 19, u32::from(different));
        let mut left = block(18, Some(400));
        left.terminator = branch(20, 400);
        let mut right = block(19, Some(500));
        right.terminator = branch(20, 500);
        let mut join = block(20, Some(600));
        join.operations.push(load(700, 600));
        inspect_addresses(
            module(vec![entry, left, right, join], true),
            &[(600, (!different).then_some(0))],
        );
    }
    let mut entry = block(70, None);
    entry.operations = vec![memory(0), memory(1)];
    for (result, right) in [(400, 0), (401, 1), (402, 98)] {
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(result), pointer()),
            OperationKind::Select {
                condition: ValueId(99),
                true_value: ValueId(0),
                false_value: ValueId(right),
            },
        ));
        entry.operations.push(load(result + 100, result));
    }
    with_report(module(vec![entry], true), |report, _| {
        let inventory = report.inventory();
        let targets = report
            .pointer_uses()
            .iter()
            .filter_map(|row| match row.role() {
                CanonicalKirPhysicalPointerRoleV1::Forwarding {
                    target_definition,
                    edge_argument: None,
                    same_allocation,
                } => Some((
                    inventory.definitions()[target_definition].value.unwrap().0,
                    same_allocation,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            targets,
            vec![
                (400, true),
                (400, true),
                (401, false),
                (401, false),
                (402, false),
                (402, false)
            ]
        );
        for (value, known) in [(400, true), (401, false), (402, false)] {
            let row = report
                .pointer_uses()
                .iter()
                .find(|row| inventory.uses()[row.use_index()].value == ValueId(value))
                .unwrap();
            if known {
                assert_eq!(
                    row.address(),
                    CanonicalKirPhysicalAddressV1::Allocation(definition(inventory, 0))
                );
            } else {
                assert_eq!(
                    row.address(),
                    CanonicalKirPhysicalAddressV1::Unresolved(
                        CanonicalKirPhysicalAddressIssueV1::UnsupportedProducer
                    )
                );
            }
        }
    });
}

#[test]
fn offsets_casts_and_calls_are_not_silently_equated_to_allocations() {
    let mut entry = block(70, None);
    let read_only = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadOnly,
    );
    entry.operations = vec![
        memory(0),
        Operation::effect_free(
            ValueDef::new(ValueId(400), read_only.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(0),
                to: read_only,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(500), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(97),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(501), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(500),
                offset: ValueId(97),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(600), pointer()),
            OperationKind::Call {
                callee: "identity".into(),
                arguments: vec![ValueId(0)],
            },
        ),
        load(700, 400),
        load(701, 500),
        load(702, 501),
        load(703, 600),
    ];
    let mut source = module(vec![entry], true);
    let mut helper = block(8, None);
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(8)],
    });
    source.functions.push(Function::internal_helper(
        "identity",
        Signature::new(vec![pointer()], vec![pointer()]),
        vec![ValueId(8)],
        vec![helper],
    ));
    with_report(source, |report, budget| {
        let inventory = report.inventory();
        let allocation = definition(inventory, 0);
        for row in report.pointer_uses() {
            let value = inventory.uses()[row.use_index()].value.0;
            match value {
                400 | 600 => assert_eq!(
                    row.address(),
                    CanonicalKirPhysicalAddressV1::Unresolved(
                        CanonicalKirPhysicalAddressIssueV1::UnsupportedProducer
                    )
                ),
                501 => assert_eq!(
                    row.address(),
                    CanonicalKirPhysicalAddressV1::Unresolved(
                        CanonicalKirPhysicalAddressIssueV1::UnsupportedOffsetBase
                    )
                ),
                500 => {
                    let CanonicalKirPhysicalAddressV1::ElementOffset {
                        allocation: actual,
                        operation,
                        offset_use,
                    } = row.address()
                    else {
                        panic!("missing explicit element offset");
                    };
                    assert_eq!(actual, allocation);
                    assert_eq!(operation, 2);
                    assert_eq!(inventory.uses()[offset_use].value, ValueId(97));
                }
                8 => {
                    assert_eq!(
                        row.address(),
                        CanonicalKirPhysicalAddressV1::Unresolved(
                            CanonicalKirPhysicalAddressIssueV1::UnknownOrigin
                        )
                    );
                    assert_eq!(
                        row.role(),
                        CanonicalKirPhysicalPointerRoleV1::Escape(
                            CanonicalKirPhysicalEscapeV1::Return
                        )
                    );
                }
                0 => assert_eq!(
                    row.address(),
                    CanonicalKirPhysicalAddressV1::Allocation(allocation)
                ),
                _ => panic!("unexpected pointer use"),
            }
        }
        let escape_roles = report
            .pointer_uses()
            .iter()
            .filter_map(|row| match row.role() {
                CanonicalKirPhysicalPointerRoleV1::Escape(escape) => Some(escape),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            escape_roles,
            vec![
                CanonicalKirPhysicalEscapeV1::UnsupportedUse,
                CanonicalKirPhysicalEscapeV1::Call,
                CanonicalKirPhysicalEscapeV1::Return
            ]
        );
        let calls = report
            .occurrences()
            .iter()
            .filter(|row| {
                matches!(
                    row.kind(),
                    CanonicalKirPhysicalOccurrenceKindV1::Call { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].kind(),
            CanonicalKirPhysicalOccurrenceKindV1::Call { call: 0 }
        );
        assert!(std::ptr::eq(
            report.operation_for(calls[0], budget).unwrap().operation,
            inventory.operations()[4].operation
        ));
    });
}

#[test]
fn stored_and_returned_pointers_remain_explicit_escapes() {
    let mut entry = block(70, None);
    entry.operations = vec![
        memory(0),
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(pointer(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: pointer(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut source = module(vec![entry], false);
    source.functions[0].signature.results.push(pointer());
    with_report(source, |report, _| {
        let roles = report
            .pointer_uses()
            .iter()
            .map(|row| row.role())
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            vec![
                CanonicalKirPhysicalPointerRoleV1::AccessAddress,
                CanonicalKirPhysicalPointerRoleV1::Escape(
                    CanonicalKirPhysicalEscapeV1::StoredValue
                ),
                CanonicalKirPhysicalPointerRoleV1::Escape(CanonicalKirPhysicalEscapeV1::Return)
            ]
        );
        assert_eq!(
            report.pointer_uses()[0].address(),
            CanonicalKirPhysicalAddressV1::OutsideWorkgroupModel(AddressSpace::Private)
        );
        assert_eq!(
            report.occurrences()[1].kind(),
            CanonicalKirPhysicalOccurrenceKindV1::Allocation {
                effect: 1,
                allocation: None,
                binding: None
            }
        );
        assert!(!report.grants_authority());
    });
}

#[test]
fn predicates_copy_order_atomics_and_barriers_borrow_exact_attributes() {
    let mut entry = block(70, None);
    let mut volatile = MemoryAccess::new(AddressSpace::Workgroup, 4);
    volatile.volatile = true;
    let element = MemoryElementType::Scalar(ScalarType::U32);
    entry.operations = vec![
        memory(0),
        memory(1),
        Operation::effect_free(
            ValueDef::new(ValueId(77), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(3)),
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(99),
                value: ValueId(77),
                access: volatile,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(78), Type::Scalar(ScalarType::U32)),
            OperationKind::GuardedLoad {
                pointer: ValueId(1),
                predicate: ValueId(99),
                fallback: ValueId(77),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(1),
                count: ValueId(97),
                element,
                source_address_space: AddressSpace::Workgroup,
                destination_address_space: AddressSpace::Workgroup,
                layout: element.expected_layout(),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(79), Type::Scalar(ScalarType::U32)),
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(77)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
                scope: SynchronizationScope::Workgroup,
                ordering: MemoryOrdering::Relaxed,
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
    let mut source = module(vec![entry], true);
    for capability in [
        TargetCapability::WorkgroupBarrier,
        TargetCapability::Atomic {
            width_bits: 32,
            address_space: AddressSpace::Workgroup,
            max_scope: SynchronizationScope::Workgroup,
        },
    ] {
        source.required_capabilities.insert(capability.clone());
        source.functions[0].required_capabilities.insert(capability);
    }
    with_report(source, |report, budget| {
        let inventory = report.inventory();
        assert_eq!(report.occurrences().len(), 8);
        assert_eq!(inventory.effects().len(), 8);
        let access = report
            .occurrences()
            .iter()
            .filter_map(|row| match row.kind() {
                CanonicalKirPhysicalOccurrenceKindV1::Access {
                    effect,
                    pointer_use: Some(pointer),
                    footprint,
                } => Some((
                    effect,
                    inventory.uses()[report.pointer_uses()[pointer].use_index()]
                        .value
                        .0,
                    footprint,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            access,
            vec![
                (2, 0, CanonicalKirPhysicalFootprintV1::TypedAccess),
                (3, 1, CanonicalKirPhysicalFootprintV1::TypedAccess),
                (4, 0, CanonicalKirPhysicalFootprintV1::RepeatedElements),
                (5, 1, CanonicalKirPhysicalFootprintV1::RepeatedElements),
                (6, 0, CanonicalKirPhysicalFootprintV1::AtomicAccess)
            ]
        );
        let stored = report
            .operation_for(&report.occurrences()[2], budget)
            .unwrap();
        assert!(matches!(
            stored.operation.kind,
            OperationKind::GuardedStore {
                predicate: ValueId(99),
                access: MemoryAccess {
                    volatile: true,
                    alignment: 4,
                    ..
                },
                ..
            }
        ));
        assert_eq!(
            report.occurrences()[7].kind(),
            CanonicalKirPhysicalOccurrenceKindV1::Synchronization { effect: 7 }
        );
        let synchronized = report
            .operation_for(&report.occurrences()[7], budget)
            .unwrap();
        let OperationKind::WorkgroupBarrier(barrier) = &synchronized.operation.kind else {
            panic!("lost barrier");
        };
        assert_eq!(barrier.semantics.ordering, MemoryOrdering::AcquireRelease);
        assert_eq!(
            barrier.convergence,
            Convergence::uniform(SynchronizationScope::Workgroup)
        );
    });
}

#[test]
fn entry_views_do_not_duplicate_or_expand_shared_helper_effects() {
    let mut shared = block(7, None);
    shared.operations.push(memory(0));
    let make_root = |id: &str| {
        let mut body = block(9, None);
        body.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "storage".into(),
                arguments: vec![],
            },
        ));
        Function::kernel_entry(id, Signature::new(vec![], vec![]), vec![], vec![body])
    };
    let mut source = module(vec![shared], false);
    source.functions.insert(0, make_root("root_b"));
    source.functions.push(make_root("root_a"));
    source.kernels = vec![
        Kernel::new(
            "a",
            "root_a",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ),
        Kernel::new(
            "b",
            "root_b",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ),
    ];
    with_report(source, |report, budget| {
        assert_eq!(report.occurrences().len(), 3);
        assert_eq!(report.function(1, budget).unwrap().occurrences().len(), 1);
        let retained = budget.storage();
        let before = budget.work();
        for (kernel, entry) in [(0, 2), (1, 0)] {
            let view = report.kernel_entry(kernel, budget).unwrap();
            assert_eq!(view.entry().function_ordinal(), entry);
            assert_eq!(view.entry().occurrences().len(), 1);
            assert!(matches!(
                view.entry().occurrences()[0].kind(),
                CanonicalKirPhysicalOccurrenceKindV1::Call { .. }
            ));
            assert!(std::ptr::eq(
                view.kernel(),
                &report.inventory().kernels()[kernel as usize]
            ));
        }
        assert_eq!(budget.work(), before + 12);
        assert_eq!(budget.storage(), retained);
        assert!(matches!(
            report.kernel_entry(2, budget),
            Err(Error::InvalidCoordinate)
        ));
    });
}

#[test]
fn effect_free_inline_assembly_is_still_an_opaque_physical_occurrence() {
    let mut entry = block(7, None);
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_add_u32".into(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
            ],
            options: [
                AssemblyOption::NoMemory,
                AssemblyOption::Pure,
                AssemblyOption::NoStack,
            ]
            .into_iter()
            .collect(),
            declared_effects: Default::default(),
        }),
    ));
    let mut source = Module::new("opaque");
    source.functions.push(Function::internal_helper(
        "assembly",
        Signature::new(vec![Type::Scalar(ScalarType::U32); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    with_report(source, |report, budget| {
        assert!(report.inventory().effects().is_empty());
        assert!(report.pointer_uses().is_empty());
        assert_eq!(report.occurrences().len(), 1);
        assert_eq!(
            report.occurrences()[0].kind(),
            CanonicalKirPhysicalOccurrenceKindV1::OpaqueAssembly
        );
        assert!(matches!(
            report
                .operation_for(&report.occurrences()[0], budget)
                .unwrap()
                .operation
                .kind,
            OperationKind::InlineAssembly(_)
        ));
        assert!(!report.grants_authority());
    });
}

#[test]
fn other_address_spaces_are_outside_the_model_not_disjoint_or_missing() {
    for space in [
        AddressSpace::Global,
        AddressSpace::Private,
        AddressSpace::Generic,
    ] {
        let mut entry = block(7, None);
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(space, 4),
            },
        ));
        let mut source = Module::new("outside-workgroup-model");
        source.functions.push(Function::internal_helper(
            "load",
            Signature::new(
                vec![Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    space,
                    AccessMode::ReadOnly,
                )],
                vec![],
            ),
            vec![ValueId(0)],
            vec![entry],
        ));
        with_checked(source, &[], &[], |inventory, checked, floor| {
            let retained = size_of::<CanonicalKirPhysicalOccurrencesV1<'_, '_>>()
                + size_of::<FunctionRanges>()
                + size_of::<CanonicalKirPhysicalOccurrenceV1>()
                + size_of::<CanonicalKirPhysicalPointerUseV1>();
            // No alias solve:21+2O+2U+F+B+8P+4N+three reservations=42.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(PREFIX + 42);
            let mut budget = Budget::new(&mut work, floor + retained);
            budget.charge_work(PREFIX).unwrap();
            budget.reserve_storage(floor).unwrap();
            let (report, storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            assert_eq!(budget.work(), PREFIX + 42);
            assert_eq!(budget.peak_storage(), floor + retained);
            assert_eq!(storage.retained_storage(), retained);
            assert_eq!(
                report.pointer_uses()[0].address(),
                CanonicalKirPhysicalAddressV1::OutsideWorkgroupModel(space)
            );
            assert_eq!(
                report.occurrences()[0].kind(),
                CanonicalKirPhysicalOccurrenceKindV1::Access {
                    effect: 0,
                    pointer_use: Some(0),
                    footprint: CanonicalKirPhysicalFootprintV1::TypedAccess
                }
            );
            budget.reserve_storage(retained).unwrap();
            drop(report);
            budget.release_storage(retained).unwrap();
        });
    }
}

#[test]
fn metadata_sparse_ids_and_prior_failure_history_do_not_change_structural_cost() {
    let mut source = direct();
    source.id = "m".repeat(2048).into();
    source.functions[0].id = "f".repeat(2048).into();
    let entry = &mut source.functions[0].body.as_mut().unwrap().blocks[0];
    entry.id = BlockId(u32::MAX);
    entry.operations = vec![
        index(u32::MAX - 1),
        memory(u32::MAX - 2),
        marker(0, u32::MAX - 2, u32::MAX - 1),
    ];
    with_checked(
        source,
        &[contract(0)],
        &[binding(u32::MAX - 2, 0, 1)],
        |inventory, checked, floor| {
            assert_eq!(
                (
                    inventory.operations().len(),
                    inventory.uses().len(),
                    inventory.definitions().len()
                ),
                (3, 2, 2)
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.charge_work(PREFIX).unwrap();
            budget.reserve_storage(floor).unwrap();
            let Err(Resource::Work(failed_work)) = budget.charge_work(usize::MAX) else {
                panic!("missing seeded work failure");
            };
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let failed_storage = budget.failed_storage();
            budget.reserve_storage(8192).unwrap();
            budget.release_storage(8192).unwrap();
            let prior_peak = budget.peak_storage();
            let (report, storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            assert_eq!(budget.work(), PREFIX + 91);
            assert_eq!(budget.peak_storage(), prior_peak);
            assert_eq!(budget.failed_storage(), failed_storage);
            assert_eq!(
                storage.retained_storage(),
                size_of::<CanonicalKirPhysicalOccurrencesV1<'_, '_>>()
                    + size_of::<FunctionRanges>()
                    + 2 * size_of::<CanonicalKirPhysicalOccurrenceV1>()
                    + size_of::<CanonicalKirPhysicalPointerUseV1>()
            );
            budget.reserve_storage(storage.retained_storage()).unwrap();
            drop(report);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(work.failed_work(), Some(failed_work.actual()));
        },
    );
}
