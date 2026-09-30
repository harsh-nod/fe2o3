use super::*;
use crate::{
    CanonicalGuardedGlobalStoreOutcomeV24, CastKind, with_canonical_guarded_global_stores_v24,
};

// The same complete guarded graph, with a real pointer/base carried through an
// unchanged helper-like block argument instead of deleting the block boundary.
fn carried(
    base: bool,
    generic: bool,
    depth: usize,
    writing: bool,
) -> (Module, Coordinate, ValueId) {
    let mut module = fixture(false);
    let access = if writing {
        AccessMode::ReadWrite
    } else {
        AccessMode::ReadOnly
    };
    let element = Type::Scalar(ScalarType::U32);
    let global = Type::pointer(element.clone(), AddressSpace::Global, access);
    let space = if generic {
        AddressSpace::Generic
    } else {
        AddressSpace::Global
    };
    let pointer = Type::pointer(element.clone(), space, access);
    module.functions[0].signature.parameters[0] =
        Type::slice(element.clone(), AddressSpace::Global, access);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations[0].results[0].ty = global.clone();
    body.blocks[1].operations[1].results[0].ty = global.clone();
    body.blocks[1].operations.pop();
    if base {
        body.blocks[1].operations.pop();
    }
    let formed = if base { ValueId(10) } else { ValueId(11) };
    let first = if generic {
        body.blocks[1].operations.push(op(
            20,
            pointer.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: formed,
                to: pointer.clone(),
            },
        ));
        ValueId(20)
    } else {
        formed
    };
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![first],
    });
    let mut final_pointer = first;
    for level in 0..depth {
        let value = ValueId(21 + level as u32);
        let mut block = BasicBlock::new(BlockId(40 + level as u32));
        block.parameters.push(ValueDef::new(value, pointer.clone()));
        if level + 1 < depth {
            block.terminator = Some(Terminator::Branch {
                target: BlockId(41 + level as u32),
                arguments: vec![value],
            });
        } else {
            final_pointer = if base {
                block.operations.push(op(
                    11,
                    pointer.clone(),
                    OperationKind::GetElementPointer {
                        base: value,
                        offset: ValueId(5),
                    },
                ));
                ValueId(11)
            } else {
                value
            };
            if writing {
                block.operations.push(op(
                    90,
                    element.clone(),
                    OperationKind::Constant(crate::Constant::U32(7)),
                ));
                block.operations.push(Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: final_pointer,
                        value: ValueId(90),
                        access: MemoryAccess::new(space, 1),
                    },
                ));
            } else {
                block.operations.push(op(
                    12,
                    element.clone(),
                    OperationKind::Load {
                        pointer: final_pointer,
                        access: MemoryAccess::new(space, 1),
                    },
                ));
            }
            block.terminator = Some(Terminator::Return { values: vec![] });
        }
        body.blocks.push(block);
    }
    let at = coordinate(0, 2 + depth as u32, u32::from(base) + u32::from(writing));
    (module, at, final_pointer)
}

#[test]
fn guarded_v18_pointer_and_slice_data_carriers_preserve_actual_read_and_store_coordinates() {
    for base in [false, true] {
        for generic in [false, true] {
            for depth in [1, 2] {
                for writing in [false, true] {
                    let (module, at, pointer) = carried(base, generic, depth, writing);
                    let (graph, credit) = owner(&module);
                    let mut work = crate::CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                    let mut budget = Budget::new(&mut work, 32 << 20);
                    budget.reserve_storage(credit + 19).unwrap();
                    let floor = budget.storage();
                    if writing {
                        with_canonical_guarded_global_stores_v24(&graph, Default::default(), &mut budget, |view, budget| {
                            let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(fact) = view.store_at(at, budget)? else { panic!("base={base} generic={generic} depth={depth}: genuine carried Store"); };
                            assert_eq!(fact.domain().pointer(), pointer);
                            assert_eq!(fact.domain().slice(), ValueId(0));
                            assert_eq!(view.function_effects(FunctionCoordinate(0), budget)?, (1, 0, 0));
                            Ok(())
                        }).unwrap();
                    } else {
                        with_canonical_guarded_global_reads_v18(&graph, Default::default(), &mut budget, |view, budget| {
                            let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact) = view.read_at(at, budget)? else { panic!("base={base} generic={generic} depth={depth}: genuine carried Load"); };
                            assert_eq!(fact.domain().pointer(), pointer);
                            assert_eq!(fact.domain().slice(), ValueId(0));
                            assert_eq!(view.function_effects(FunctionCoordinate(0), budget)?, (1, 0, 0));
                            Ok(())
                        }).unwrap();
                    }
                    assert_eq!(budget.storage(), floor);
                }
            }
        }
    }
}

#[test]
fn guarded_v18_carried_pointers_reject_conflicting_parallel_edges_and_guard_bypasses() {
    for bypass in [false, true] {
        let (mut module, at, _) = carried(false, true, 1, false);
        let function = &mut module.functions[0];
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadOnly,
        );
        function.signature.parameters.push(pointer);
        let body = function.body.as_mut().unwrap();
        body.parameters.push(ValueId(99));
        if bypass {
            let Some(Terminator::ConditionalBranch {
                else_target,
                else_arguments,
                ..
            }) = &mut body.blocks[0].terminator
            else {
                unreachable!();
            };
            *else_target = BlockId(40);
            *else_arguments = vec![ValueId(99)];
        } else {
            body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(40),
                then_arguments: vec![ValueId(20)],
                else_target: BlockId(40),
                else_arguments: vec![ValueId(99)],
            });
        }
        let (graph, credit) = owner(&module);
        run(&graph, credit, |view, budget| {
            assert!(matches!(
                view.read_at(at, budget)?,
                CanonicalGuardedGlobalReadOutcomeV18::NotProved(
                    CanonicalGuardedGlobalReadReasonV1::MissingBoundOrProvenance
                )
            ));
            assert_eq!(
                view.function_effects(FunctionCoordinate(0), budget)?,
                (1, 0, 0)
            );
            Ok(())
        })
        .unwrap();
    }
    let (mut module, at, _) = carried(false, true, 1, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(40),
        then_arguments: vec![ValueId(20)],
        else_target: BlockId(40),
        else_arguments: vec![ValueId(20)],
    });
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        assert!(matches!(
            view.read_at(at, budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(_)
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn guarded_v18_carried_private_and_workgroup_origins_are_not_global_slice_proofs() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let (mut module, at, _) = carried(false, true, 1, false);
        let function = &mut module.functions[0];
        function.signature.parameters.push(Type::pointer(
            Type::Scalar(ScalarType::U32),
            space,
            AccessMode::ReadOnly,
        ));
        let body = function.body.as_mut().unwrap();
        body.parameters.push(ValueId(99));
        let OperationKind::Cast { value, .. } = &mut body.blocks[1].operations[2].kind else {
            unreachable!();
        };
        *value = ValueId(99);
        let (graph, credit) = owner(&module);
        run(&graph, credit, |view, budget| {
            assert!(matches!(
                view.read_at(at, budget)?,
                CanonicalGuardedGlobalReadOutcomeV18::NotProved(_)
            ));
            assert_eq!(
                view.function_effects(FunctionCoordinate(0), budget)?,
                (0, 0, 0)
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn guarded_v18_carried_pointer_derivation_has_exact_and_one_short_resource_limits() {
    let (module, at, _) = carried(true, true, 2, false);
    let (graph, credit) = owner(&module);
    let floor = credit + 29;
    let run = |work_limit, storage_limit| {
        let mut work = crate::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_guarded_global_reads_v18(
            &graph,
            Default::default(),
            &mut budget,
            |view, budget| {
                assert!(matches!(
                    view.read_at(at, budget)?,
                    CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(_)
                ));
                Ok(())
            },
        );
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = run(100_000_000, 32 << 20);
    result.unwrap();
    let (result, exact_work, exact_storage) = run(work, storage);
    result.unwrap();
    assert_eq!((exact_work, exact_storage), (work, storage));
    assert!(
        matches!(run(work - 1, storage).0, Err(Failure::Resource(ResourceError::Work(error))) if error.limit() == work - 1 && error.actual() > error.limit())
    );
    assert!(
        matches!(run(work, storage - 1).0, Err(Failure::Resource(ResourceError::Storage { actual, limit })) if limit == storage - 1 && actual > limit)
    );
}
