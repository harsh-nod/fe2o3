use super::*;
use crate::{
    AccessMode, BasicBlock, CanonicalKernelIrWorkBudgetV1, CastKind, FunctionId, MemoryAccess,
    Signature, ValueDef, analyze_interprocedural_effects_v1, verify_module_ref,
};

pub(super) const FLOOR: usize = 17;

pub(super) fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
pub(super) fn pointer(element: Type) -> Type {
    Type::pointer(element, AddressSpace::Private, AccessMode::ReadWrite)
}
pub(super) fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}
pub(super) fn function(
    name: &str,
    operations: Vec<Operation>,
    result: Option<(ValueId, Type)>,
) -> Function {
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = operations;
    block.terminator = Some(Terminator::Return {
        values: result.as_ref().map(|(id, _)| vec![*id]).unwrap_or_default(),
    });
    Function::definition(
        name,
        Signature::new(vec![], result.map(|(_, ty)| vec![ty]).unwrap_or_default()),
        vec![],
        vec![block],
    )
}
pub(super) fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("local_frame_effects");
    module.functions = functions;
    module
}
fn slot() -> Module {
    module(vec![function(
        "slot",
        vec![
            constant(0, Constant::U32(7)),
            Operation::effect_free(
                ValueDef::new(ValueId(1), pointer(scalar())),
                OperationKind::Alloca {
                    element: scalar(),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), scalar()),
                OperationKind::Load {
                    pointer: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ],
        Some((ValueId(2), scalar())),
    )])
}
fn array(value: Constant, count: u64, cell: u64) -> Module {
    let element = value.ty();
    let alignment = u32::from(
        element
            .as_scalar()
            .unwrap()
            .bit_width()
            .unwrap()
            .div_ceil(8),
    );
    module(vec![function(
        "array",
        vec![
            constant(90, Constant::U64(count)),
            constant(80, Constant::U64(cell)),
            constant(7, value),
            Operation::effect_free(
                ValueDef::new(ValueId(100), pointer(element.clone())),
                OperationKind::Alloca {
                    element: element.clone(),
                    count: Some(ValueId(90)),
                    address_space: AddressSpace::Private,
                    alignment,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), pointer(element.clone())),
                OperationKind::GetElementPointer {
                    base: ValueId(100),
                    offset: ValueId(80),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(3),
                    value: ValueId(7),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(200), element.clone()),
                OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(3),
                    value: ValueId(200),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(201), element.clone()),
                OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                },
            ),
        ],
        Some((ValueId(201), element)),
    )])
}
fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
fn rejected(module: &Module, operation: Option<usize>, reason: LocalFrameRefusalReasonV1) {
    let verified = verify_module_ref(module).expect("negative candidate must independently verify");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let mut entered = false;
    let actual = with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
        entered = true;
        Ok(())
    });
    assert_eq!(actual, Err(refusal(0, operation, reason)));
    assert!(!entered);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn scalar_and_counted_frames_retain_all_accesses_without_changing_raw_summaries() {
    for value in [
        Constant::U8(9),
        Constant::I16(-3),
        Constant::U32(7),
        Constant::F32Bits(0x7fc0_1234),
        Constant::F64Bits(0x8000_0000_0000_0000),
    ] {
        for cell in [0, 1] {
            let module = array(value.clone(), 2, cell);
            let original = module.clone();
            let before = analyze_interprocedural_effects_v1(&module).unwrap();
            let raw = before.function(&FunctionId::new("array")).unwrap();
            assert!(raw.is_complete());
            assert!(!raw.is_complete_and_pure());
            assert!(raw.summary().reads(AddressSpace::Private));
            assert!(raw.summary().writes(AddressSpace::Private));
            let verified = verify_module_ref(&module).unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut entered = false;
            with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
                entered = true;
                assert!(std::ptr::eq(checked.module(), &module));
                assert!(std::ptr::eq(checked.function(), &module.functions[0]));
                let allocations = checked.allocations(budget)?;
                assert_eq!(allocations.len(), 1);
                assert_eq!(
                    allocations[0].location(),
                    LocalFrameLocationV1 {
                        function_ordinal: 0,
                        block: BlockId(7),
                        operation: 3
                    }
                );
                assert_eq!(allocations[0].count(), 2);
                assert_eq!(
                    allocations[0].byte_extent(),
                    2 * allocations[0].element_bytes()
                );
                assert_eq!(allocations[0].element(), value.ty().as_scalar().unwrap());
                let accesses = checked.accesses(budget)?;
                assert_eq!(accesses.len(), 4);
                assert_eq!(
                    accesses
                        .iter()
                        .map(|row| row.location().operation())
                        .collect::<Vec<_>>(),
                    [5, 6, 7, 8]
                );
                assert!(accesses.iter().all(|row| row.cell() == cell
                    && row.allocation() == 0
                    && row.pointer() == ValueId(3)));
                assert_eq!(accesses[0].kind(), LocalFrameAccessKindV1::Write);
                assert_eq!(accesses[1].kind(), LocalFrameAccessKindV1::Read);
                assert_eq!(
                    accesses[1].initializing_store(),
                    Some(accesses[0].location())
                );
                assert_eq!(
                    accesses[3].initializing_store(),
                    Some(accesses[2].location())
                );
                assert_eq!(accesses[2].value(), ValueId(200));
                assert_eq!(accesses[3].value(), ValueId(201));
                Ok(())
            })
            .unwrap();
            assert!(entered);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(module, original);
            assert_eq!(analyze_interprocedural_effects_v1(&module).unwrap(), before);
        }
    }
}

#[test]
fn parameter_stores_and_same_numeric_ids_in_distinct_functions_are_qualified() {
    let mut module = slot();
    operations(&mut module).remove(0);
    module.functions[0].signature.parameters = vec![scalar()];
    module.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(0)];
    let mut second = module.functions[0].clone();
    second.id = FunctionId::new("second");
    module.functions.push(second);
    let verified = verify_module_ref(&module).unwrap();
    for ordinal in 0..2 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_checked_local_frame_function_v1(verified, ordinal, &mut budget, |checked, budget| {
            assert_eq!(checked.function_ordinal(), ordinal);
            let allocation = checked.allocations(budget)?[0];
            assert_eq!(allocation.location().function_ordinal(), ordinal);
            assert_eq!(allocation.pointer(), ValueId(1));
            let accesses = checked.accesses(budget)?;
            assert_eq!(accesses[0].value(), ValueId(0));
            assert_eq!(
                accesses[1].initializing_store().unwrap().function_ordinal(),
                ordinal
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn every_load_requires_a_prior_store_to_the_exact_allocation_and_cell() {
    use LocalFrameRefusalReasonV1::UninitializedRead;
    let mut before_store = slot();
    operations(&mut before_store).swap(2, 3);
    rejected(&before_store, Some(2), UninitializedRead);
    let mut wrong_cell = array(Constant::U32(7), 2, 1);
    if let OperationKind::Load { pointer, .. } = &mut operations(&mut wrong_cell)[6].kind {
        *pointer = ValueId(100);
    }
    rejected(&wrong_cell, Some(6), UninitializedRead);
    let mut other_allocation = slot();
    operations(&mut other_allocation).insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer(scalar())),
            OperationKind::Alloca {
                element: scalar(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    if let OperationKind::Load { pointer, .. } = &mut operations(&mut other_allocation)[4].kind {
        *pointer = ValueId(9);
    }
    rejected(&other_allocation, Some(4), UninitializedRead);
}

#[test]
fn extent_offset_and_alignment_are_checked_before_any_receipt() {
    use LocalFrameRefusalReasonV1 as Reason;
    rejected(&array(Constant::U32(7), 0, 0), Some(3), Reason::Allocation);
    rejected(
        &array(Constant::U32(7), u64::MAX, 0),
        Some(3),
        Reason::Allocation,
    );
    rejected(&array(Constant::U32(7), 2, 2), Some(4), Reason::Index);
    let mut negative = array(Constant::U32(7), 2, 0);
    operations(&mut negative)[1] = constant(80, Constant::I64(-1));
    rejected(&negative, Some(4), Reason::Index);
    let mut dynamic = array(Constant::U32(7), 2, 0);
    operations(&mut dynamic).remove(0);
    dynamic.functions[0].signature.parameters = vec![Type::Scalar(ScalarType::U64)];
    dynamic.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(90)];
    rejected(&dynamic, Some(2), Reason::Index);
    for allocation_alignment in [4, 8] {
        let mut misaligned = array(Constant::U32(7), 2, 1);
        if let OperationKind::Alloca { alignment, .. } = &mut operations(&mut misaligned)[3].kind {
            *alignment = allocation_alignment;
        }
        if let OperationKind::Store { access, .. } = &mut operations(&mut misaligned)[5].kind {
            access.alignment = 8;
        }
        rejected(&misaligned, Some(5), Reason::Alignment);
    }
}

#[test]
fn casts_pointer_selects_returns_and_chained_geps_are_not_escape_shortcuts() {
    use LocalFrameRefusalReasonV1 as Reason;
    let mut cast = slot();
    let readonly = Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadOnly);
    operations(&mut cast).push(Operation::effect_free(
        ValueDef::new(ValueId(8), readonly.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(1),
            to: readonly,
        },
    ));
    rejected(&cast, Some(4), Reason::Operation);
    let mut select = slot();
    operations(&mut select).push(constant(8, Constant::Bool(true)));
    operations(&mut select).push(Operation::effect_free(
        ValueDef::new(ValueId(9), pointer(scalar())),
        OperationKind::Select {
            condition: ValueId(8),
            true_value: ValueId(1),
            false_value: ValueId(1),
        },
    ));
    rejected(&select, Some(5), Reason::PointerUse);
    let mut returned = slot();
    returned.functions[0].signature.results = vec![pointer(scalar())];
    returned.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    rejected(&returned, None, Reason::Signature);
    let mut chain = array(Constant::U32(7), 2, 0);
    operations(&mut chain).push(Operation::effect_free(
        ValueDef::new(ValueId(999), pointer(scalar())),
        OperationKind::GetElementPointer {
            base: ValueId(3),
            offset: ValueId(80),
        },
    ));
    rejected(&chain, Some(9), Reason::PointerUse);
}

#[test]
fn volatile_external_memory_calls_and_nonleaf_control_remain_unsupported() {
    use LocalFrameRefusalReasonV1 as Reason;
    let mut volatile = slot();
    if let OperationKind::Store { access, .. } = &mut operations(&mut volatile)[2].kind {
        access.volatile = true;
    }
    rejected(&volatile, Some(2), Reason::Effects);
    let global = Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut external = function(
        "external_memory",
        vec![Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
        None,
    );
    external.signature.parameters = vec![global, scalar()];
    external.body.as_mut().unwrap().parameters = vec![ValueId(0), ValueId(1)];
    rejected(&module(vec![external]), None, Reason::Signature);
    let calls = module(vec![
        function(
            "caller",
            vec![Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("pure"),
                    arguments: vec![],
                },
            )],
            None,
        ),
        function("pure", vec![], None),
    ]);
    assert!(
        analyze_interprocedural_effects_v1(&calls)
            .unwrap()
            .function(&FunctionId::new("caller"))
            .unwrap()
            .is_complete_and_pure()
    );
    rejected(&calls, Some(0), Reason::Operation);
    let mut escaped_call = slot();
    operations(&mut escaped_call).push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("consume_pointer"),
            arguments: vec![ValueId(1)],
        },
    ));
    let mut callee = function("consume_pointer", vec![], None);
    callee.signature.parameters = vec![pointer(scalar())];
    callee.body.as_mut().unwrap().parameters = vec![ValueId(0)];
    escaped_call.functions.push(callee);
    rejected(&escaped_call, Some(4), Reason::Operation);
    let mut branch = slot();
    let body = branch.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(9));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    body.blocks.push(exit);
    rejected(&branch, None, Reason::ControlFlow);
    let mut looped = module(vec![function("looped", vec![], None)]);
    looped.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    rejected(&looped, None, Reason::ControlFlow);
}

#[test]
fn effect_audit_component_does_not_confuse_empty_physical_effects_with_ordering() {
    let operation = Operation::new(
        vec![],
        OperationKind::VerificationContract(
            crate::VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: crate::VerificationContractKeyV12::new(0),
                kind: crate::WorkgroupPipelineEventKindV12::Commit,
                storage: ValueId(1),
                epoch: ValueId(2),
            },
        ),
    );
    assert!(operation.memory_effects().is_empty());
    assert!(!operation.compiler_ordering_effects_v12().is_empty());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(
        audit_effects(&operation, 4, 5, &mut budget),
        Err(refusal(4, Some(5), LocalFrameRefusalReasonV1::Effects))
    );
    assert_eq!(budget.work(), 3);
    assert_eq!(budget.storage(), 0);
}

pub(super) fn header_bytes() -> usize {
    size_of::<Workspace<'_>>() + size_of::<CheckedLocalFrameV1<'_, '_>>()
}
pub(super) fn capacity_bytes<T>(count: usize) -> usize {
    let mut rows: Vec<T> = Vec::new();
    rows.try_reserve_exact(count).unwrap();
    rows.capacity() * size_of::<T>()
}
fn slot_bytes() -> usize {
    header_bytes()
        + capacity_bytes::<Definition<'_>>(3)
        + capacity_bytes::<LocalFrameAllocationV1>(1)
        + capacity_bytes::<LocalFrameAccessV1>(2)
        + capacity_bytes::<CellVisit>(2)
}

#[test]
fn exact_source_derived_work_and_actual_capacity_boundaries_preserve_the_floor() {
    let module = slot();
    let verified = verify_module_ref(&module).unwrap();
    // Wrapper2 + entry8 + signature1 + census4 + reserve8 + fill13 + sort24 +
    // unique2 + operation totals(15,21,46,41) + return6 + cell sort24 + rows10.
    // V356 adds row initialization6, literal fact7, and two scalar-slot joins14.
    const EXACT: usize = 2 + 8 + 1 + 4 + 8 + 13 + 24 + 2 + 15 + 21 + 46 + 41 + 6 + 24 + 10;
    assert_eq!(EXACT, 225);
    for limit in [EXACT - 1, EXACT] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, FLOOR + slot_bytes());
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
            entered = true;
            Ok(())
        });
        if limit == EXACT {
            assert_eq!(result, Ok(()));
            assert!(entered);
            assert_eq!(budget.work(), EXACT);
        } else {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == EXACT && error.limit() == limit)
            );
            assert!(!entered);
            assert_eq!(budget.work(), EXACT - 5);
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + slot_bytes());
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let exact_storage = FLOOR + slot_bytes();
    let mut budget = Budget::new(&mut work, exact_storage - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
        panic!("storage denial must precede callback")
    });
    assert!(
        matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Storage(error))) if error.actual() == exact_storage && error.limit() == exact_storage - 1)
    );
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn empty_scope_has_exact_header_and_after_allocation_work_denials() {
    let module = module(vec![function("empty", vec![], None)]);
    let verified = verify_module_ref(&module).unwrap();
    // Wrapper2 + entry8 + four reservation checks8 + terminator1.
    for (work_limit, storage_limit) in [
        (19, FLOOR + header_bytes()),
        (18, FLOOR + header_bytes()),
        (19, FLOOR + header_bytes() - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
            entered = true;
            Ok(())
        });
        if storage_limit < FLOOR + header_bytes() {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Storage(error))) if error.actual() == FLOOR + header_bytes())
            );
            assert_eq!(budget.work(), 2);
            assert_eq!(budget.peak_storage(), FLOOR);
        } else if work_limit == 18 {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == 19 && error.limit() == 18)
            );
            assert_eq!(budget.work(), 18);
            assert_eq!(budget.peak_storage(), FLOOR + header_bytes());
        } else {
            assert_eq!(result, Ok(()));
            assert!(entered);
            assert_eq!(budget.work(), 19);
        }
        if result.is_err() {
            assert!(!entered);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn storage_is_bounded_by_actual_operations_not_declared_cell_count() {
    let mut observations = Vec::new();
    for count in [2, u64::MAX / 4] {
        let module = array(Constant::U32(7), count, 1);
        let verified = verify_module_ref(&module).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
            assert_eq!(checked.allocations(budget)?[0].count(), count);
            assert_eq!(checked.accesses(budget)?.len(), 4);
            Ok(())
        })
        .unwrap();
        observations.push((budget.work(), budget.peak_storage()));
        assert_eq!(budget.storage(), 0);
    }
    assert_eq!(observations[0], observations[1]);
}

#[test]
fn queries_reject_foreign_ledgers_and_retained_floor_tampering() {
    let module = slot();
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(4);
        let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
        foreign.reserve_storage(budget.storage()).unwrap();
        assert!(matches!(
            checked.allocations(&mut foreign),
            Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
        ));
        assert_eq!(foreign.work(), 4);
        assert_eq!(checked.allocations(budget)?.len(), 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    for mode in 0..3 {
        let mut entered = false;
        let result =
            with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
                entered = true;
                let retained = budget.storage() - FLOOR;
                assert!(retained > 0);
                budget.release_storage(retained).unwrap();
                assert!(matches!(
                    checked.accesses(budget),
                    Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                ));
                match mode {
                    0 => Ok(()),
                    1 => Err(refusal(99, None, LocalFrameRefusalReasonV1::Operation)),
                    _ => panic!("tampered local frame callback"),
                }
            });
        assert!(entered);
        assert_eq!(
            result,
            Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn paid_row_query_has_an_exact_boundary_after_a_complete_derivation() {
    let module = slot();
    let verified = verify_module_ref(&module).unwrap();
    // The independently enumerated slot path costs225; the query adds guard4
    // then allocation-roster length1. This is not a whole-engine threshold.
    for limit in [229, 230] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result =
            with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
                entered = true;
                assert_eq!(checked.allocations(budget)?.len(), 1);
                Ok(())
            });
        assert!(entered);
        if limit == 229 {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == 230 && error.limit() == 229)
            );
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(budget.work(), limit);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn callback_error_and_original_panic_survive_clean_cleanup_and_allow_reentry() {
    let module = slot();
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let expected = refusal(99, Some(4), LocalFrameRefusalReasonV1::Operation);
    let mut entered = false;
    assert_eq!(
        with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
            entered = true;
            assert_eq!(checked.accesses(budget)?.len(), 2);
            Err(expected)
        }),
        Err(expected)
    );
    assert!(entered);
    assert_eq!(budget.storage(), FLOOR);
    entered = false;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ =
            with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
                entered = true;
                assert_eq!(checked.allocations(budget)?.len(), 1);
                panic!("original local frame callback");
            });
    }))
    .unwrap_err();
    assert!(entered);
    assert_eq!(
        panic.downcast_ref::<&str>().copied(),
        Some("original local frame callback")
    );
    assert_eq!(budget.storage(), FLOOR);
    with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| Ok(())).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn cast_index(input: Constant, kind: CastKind, to: ScalarType, count: u64) -> Module {
    let mut module = array(Constant::U32(7), count, 0);
    operations(&mut module)[1] = constant(80, input);
    operations(&mut module).insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(81), Type::Scalar(to)),
            OperationKind::Cast {
                kind,
                value: ValueId(80),
                to: Type::Scalar(to),
            },
        ),
    );
    if let OperationKind::GetElementPointer { offset, .. } = &mut operations(&mut module)[5].kind {
        *offset = ValueId(81);
    }
    module
}

fn checked_target_cell(module: &Module, expected: u64) {
    let verified = verify_module_ref(module).expect("transport fixture must independently verify");
    let source_operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let expected_memory = source_operations
        .iter()
        .enumerate()
        .filter_map(|(at, operation)| {
            matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            )
            .then_some(at)
        })
        .collect::<Vec<_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let mut entered = false;
    with_checked_local_frame_function_v1(verified, 0, &mut budget, |checked, budget| {
        entered = true;
        let accesses = checked.accesses(budget)?;
        assert_eq!(
            accesses
                .iter()
                .map(|row| row.location().operation())
                .collect::<Vec<_>>(),
            expected_memory
        );
        let target = accesses
            .iter()
            .filter(|row| row.pointer() == ValueId(3))
            .collect::<Vec<_>>();
        assert_eq!(target.len(), 4);
        assert!(target.iter().all(|row| row.cell() == expected));
        assert_eq!(target[1].initializing_store(), Some(target[0].location()));
        assert_eq!(target[3].initializing_store(), Some(target[2].location()));
        Ok(())
    })
    .unwrap();
    assert!(entered);
    assert_eq!(budget.storage(), FLOOR);
    assert!(
        !analyze_interprocedural_effects_v1(module)
            .unwrap()
            .function(&module.functions[0].id)
            .unwrap()
            .is_complete_and_pure()
    );
}

#[test]
fn verified_unsigned_casts_bind_exact_truncated_and_extended_index_values() {
    for (input, kind, to, expected) in [
        (
            Constant::U8(255),
            CastKind::ZeroExtend,
            ScalarType::U16,
            255,
        ),
        (
            Constant::U16(65535),
            CastKind::Truncate,
            ScalarType::U8,
            255,
        ),
        (Constant::U32(256), CastKind::Truncate, ScalarType::U8, 0),
        (
            Constant::U64(0x1_0000_0001),
            CastKind::Truncate,
            ScalarType::U32,
            1,
        ),
        (Constant::U32(1), CastKind::ZeroExtend, ScalarType::Index, 1),
        (Constant::U64(1), CastKind::Bitcast, ScalarType::Index, 1),
        (Constant::Index(1), CastKind::Bitcast, ScalarType::U64, 1),
        (
            Constant::Bool(true),
            CastKind::ZeroExtend,
            ScalarType::U8,
            1,
        ),
    ] {
        checked_target_cell(&cast_index(input, kind, to, 256), expected);
    }
    let mut chain = cast_index(Constant::U8(1), CastKind::ZeroExtend, ScalarType::U32, 2);
    operations(&mut chain).insert(
        3,
        Operation::effect_free(
            ValueDef::new(ValueId(82), Type::INDEX),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(81),
                to: Type::INDEX,
            },
        ),
    );
    if let OperationKind::GetElementPointer { offset, .. } = &mut operations(&mut chain)[6].kind {
        *offset = ValueId(82);
    }
    checked_target_cell(&chain, 1);
}

#[test]
fn cast_fact_component_has_full_width_and_exact_precharge_boundaries() {
    let location = LocalFrameLocationV1 {
        function_ordinal: 2,
        block: BlockId(9),
        operation: 4,
    };
    for (kind, from, to, bits, expected) in [
        (
            CastKind::ZeroExtend,
            ScalarType::U32,
            ScalarType::Index,
            u64::from(u32::MAX),
            u64::from(u32::MAX),
        ),
        (
            CastKind::Bitcast,
            ScalarType::U64,
            ScalarType::Index,
            u64::MAX,
            u64::MAX,
        ),
        (
            CastKind::Bitcast,
            ScalarType::Index,
            ScalarType::U64,
            u64::MAX,
            u64::MAX,
        ),
        (
            CastKind::Truncate,
            ScalarType::U64,
            ScalarType::U32,
            u64::MAX,
            u64::from(u32::MAX),
        ),
        (CastKind::Truncate, ScalarType::U32, ScalarType::U8, 256, 0),
    ] {
        for limit in [7, 8] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.reserve_storage(FLOOR).unwrap();
            let result = checked_cast_fact(
                kind,
                from,
                to,
                Some(KnownUnsigned { ty: from, bits }),
                location,
                &mut budget,
            );
            if limit == 7 {
                assert!(
                    matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == 8 && error.limit() == 7)
                );
                assert_eq!(budget.work(), 0);
            } else {
                assert_eq!(
                    result,
                    Ok(Some(KnownUnsigned {
                        ty: to,
                        bits: expected
                    }))
                );
                assert_eq!(budget.work(), 8);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
        }
    }
    for fact in [
        KnownUnsigned {
            ty: ScalarType::U16,
            bits: 1,
        },
        KnownUnsigned {
            ty: ScalarType::U32,
            bits: u64::MAX,
        },
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(
            checked_cast_fact(
                CastKind::ZeroExtend,
                ScalarType::U32,
                ScalarType::Index,
                Some(fact),
                location,
                &mut budget
            ),
            Err(ResourceError::Accounting.into())
        );
    }
}

fn retained_index(alias_zero: bool) -> Module {
    let mut module = array(Constant::U32(7), 2, 0);
    operations(&mut module)[1] = constant(80, Constant::U32(1));
    let mut prefix = vec![Operation::effect_free(
        ValueDef::new(ValueId(70), pointer(scalar())),
        OperationKind::Alloca {
            element: scalar(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    )];
    let store_pointer = if alias_zero {
        prefix.push(constant(73, Constant::Index(0)));
        prefix.push(Operation::effect_free(
            ValueDef::new(ValueId(74), pointer(scalar())),
            OperationKind::GetElementPointer {
                base: ValueId(70),
                offset: ValueId(73),
            },
        ));
        ValueId(74)
    } else {
        ValueId(70)
    };
    prefix.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: store_pointer,
            value: ValueId(80),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    prefix.push(Operation::effect_free(
        ValueDef::new(ValueId(71), scalar()),
        OperationKind::Load {
            pointer: ValueId(70),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    prefix.push(Operation::effect_free(
        ValueDef::new(ValueId(72), Type::INDEX),
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            value: ValueId(71),
            to: Type::INDEX,
        },
    ));
    operations(&mut module).splice(2..2, prefix);
    for operation in operations(&mut module) {
        if operation
            .results
            .first()
            .is_some_and(|result| result.id == ValueId(3))
        {
            if let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind {
                *offset = ValueId(72);
            }
        }
    }
    module
}

fn target_gep(module: &Module) -> usize {
    module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .position(|operation| {
            operation
                .results
                .first()
                .is_some_and(|result| result.id == ValueId(3))
        })
        .unwrap()
}

#[test]
fn exact_scalar_slot_load_and_direct_gep_zero_alias_transport_the_index() {
    for alias_zero in [false, true] {
        checked_target_cell(&retained_index(alias_zero), 1);
    }
    // A separate classification starts from its own function-qualified state,
    // despite reusing every numeric value ID from the prior successful call.
    let mut unknown = retained_index(false);
    operations(&mut unknown).remove(1);
    unknown.functions[0].signature.parameters = vec![scalar()];
    unknown.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(80)];
    rejected(
        &unknown,
        Some(target_gep(&unknown)),
        LocalFrameRefusalReasonV1::Index,
    );
}

#[test]
fn every_unknown_overwrite_clears_a_slot_and_known_reinitialization_restores_it() {
    for (initial_alias, unknown_alias, restored_alias) in [
        (false, false, false),
        (true, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let mut unknown = retained_index(initial_alias);
        if !initial_alias && (unknown_alias || restored_alias) {
            // Define the alias before the original direct Store without changing it.
            operations(&mut unknown).splice(
                3..3,
                [
                    constant(73, Constant::Index(0)),
                    Operation::effect_free(
                        ValueDef::new(ValueId(74), pointer(scalar())),
                        OperationKind::GetElementPointer {
                            base: ValueId(70),
                            offset: ValueId(73),
                        },
                    ),
                ],
            );
        }
        unknown.functions[0].signature.parameters = vec![scalar()];
        unknown.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(9)];
        let load = operations(&mut unknown)
            .iter()
            .position(|operation| {
                operation
                    .results
                    .first()
                    .is_some_and(|result| result.id == ValueId(71))
            })
            .unwrap();
        operations(&mut unknown).insert(
            load,
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(if unknown_alias { 74 } else { 70 }),
                    value: ValueId(9),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        );
        rejected(
            &unknown,
            Some(target_gep(&unknown)),
            LocalFrameRefusalReasonV1::Index,
        );
        operations(&mut unknown).insert(
            load + 1,
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(if restored_alias { 74 } else { 70 }),
                    value: ValueId(80),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        );
        checked_target_cell(&unknown, 1);
    }
}

#[test]
fn slot_transport_never_crosses_uninitialized_allocations_or_array_cells() {
    let mut before_init = retained_index(false);
    let store = operations(&mut before_init)
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::Store {
                    pointer: ValueId(70),
                    ..
                }
            )
        })
        .unwrap();
    operations(&mut before_init).remove(store);
    // The index consumer refuses the unknown value before the later full-cell
    // census. The old direct load-before-store test still reaches UninitializedRead.
    rejected(
        &before_init,
        Some(target_gep(&before_init)),
        LocalFrameRefusalReasonV1::Index,
    );
    let mut other = retained_index(false);
    let load = operations(&mut other)
        .iter()
        .position(|operation| {
            operation
                .results
                .first()
                .is_some_and(|result| result.id == ValueId(71))
        })
        .unwrap();
    operations(&mut other).insert(
        load,
        Operation::effect_free(
            ValueDef::new(ValueId(75), pointer(scalar())),
            OperationKind::Alloca {
                element: scalar(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    if let OperationKind::Load { pointer, .. } = &mut operations(&mut other)[load + 1].kind {
        *pointer = ValueId(75);
    }
    rejected(
        &other,
        Some(target_gep(&other)),
        LocalFrameRefusalReasonV1::Index,
    );
    for stored_cell in [0, 1] {
        let mut array_slot = retained_index(true);
        if let OperationKind::Alloca { count, .. } = &mut operations(&mut array_slot)[2].kind {
            *count = Some(ValueId(90));
        }
        operations(&mut array_slot)[3] = constant(73, Constant::Index(stored_cell));
        rejected(
            &array_slot,
            Some(target_gep(&array_slot)),
            LocalFrameRefusalReasonV1::Index,
        );
    }
}

#[test]
fn cast_unknown_signed_float_and_out_of_range_paths_do_not_produce_an_index() {
    let mut unknown = cast_index(Constant::U32(1), CastKind::ZeroExtend, ScalarType::Index, 2);
    operations(&mut unknown).remove(1);
    unknown.functions[0].signature.parameters = vec![scalar()];
    unknown.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(80)];
    rejected(
        &unknown,
        Some(target_gep(&unknown)),
        LocalFrameRefusalReasonV1::Index,
    );
    let signed = cast_index(Constant::I32(1), CastKind::SignExtend, ScalarType::I64, 2);
    rejected(&signed, Some(2), LocalFrameRefusalReasonV1::Operation);
    let float = cast_index(Constant::F32Bits(0), CastKind::Bitcast, ScalarType::U32, 2);
    rejected(&float, Some(2), LocalFrameRefusalReasonV1::Operation);
    let out_of_range = cast_index(Constant::U32(2), CastKind::ZeroExtend, ScalarType::Index, 2);
    rejected(
        &out_of_range,
        Some(target_gep(&out_of_range)),
        LocalFrameRefusalReasonV1::Index,
    );
}
