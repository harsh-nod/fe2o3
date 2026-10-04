use super::*;
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation, ComparePredicate, LocalFrameControlKindV1,
    with_checked_local_frame_chain_function_v1,
};
use std::mem::size_of;

// These are verified physical KIR fixtures, not admitted Rust/source relations.
fn assert_chain(index_mode: u8) -> Module {
    let mut entry = BasicBlock::new(BlockId(7));
    entry.operations.push(constant(10, Constant::Index(8)));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), pointer()),
        OperationKind::Alloca {
            element: scalar(),
            count: Some(ValueId(10)),
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    for (cell, value) in [11, 13, 17, 19, 23, 29, 31, 37].into_iter().enumerate() {
        let cell = cell as u32;
        entry.operations.extend([
            constant(100 + cell, Constant::U32(value)),
            constant(120 + cell, Constant::Index(u64::from(cell))),
            Operation::effect_free(
                ValueDef::new(ValueId(140 + cell), pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(11),
                    offset: ValueId(120 + cell),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(140 + cell),
                    value: ValueId(100 + cell),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    entry.operations.push(constant(20, Constant::U64(8)));
    if index_mode == 1 {
        entry.operations.extend([
            constant(24, Constant::U32(0)),
            Operation::effect_free(
                ValueDef::new(ValueId(25), pointer()),
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
                    pointer: ValueId(25),
                    value: ValueId(24),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    append_index(&mut entry, index_mode, 21, 26);
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(22), Type::INDEX),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(21),
            to: Type::INDEX,
        },
    ));
    append_condition(&mut entry, 23, 21, BlockId(19));

    let mut write = BasicBlock::new(BlockId(19));
    write.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(30), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(22),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(30),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ]);
    append_index(&mut write, index_mode, 31, 34);
    write.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(32), Type::INDEX),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(31),
            to: Type::INDEX,
        },
    ));
    append_condition(&mut write, 33, 31, BlockId(3));

    let mut read = BasicBlock::new(BlockId(3));
    read.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(40), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(32),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(41), scalar()),
            OperationKind::Load {
                pointer: ValueId(40),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        constant(42, Constant::U64(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(43), Type::INDEX),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(42),
                to: Type::INDEX,
            },
        ),
    ]);
    append_condition(&mut read, 44, 42, BlockId(42));

    let mut exit = BasicBlock::new(BlockId(42));
    exit.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(50), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(43),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(50),
                value: ValueId(41),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ]);
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut sink = BasicBlock::new(BlockId(99));
    sink.operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    sink.terminator = Some(Terminator::Unreachable);
    let parameters = if index_mode == 2 {
        vec![ValueId(0), ValueId(1)]
    } else {
        vec![ValueId(0)]
    };
    let mut helper = Function::internal_helper(
        "assert_chain",
        Signature::new(vec![scalar(); parameters.len()], vec![]),
        parameters,
        vec![entry, exit, read, write, sink],
    );
    helper.required_capabilities = AmdGpuDiagnosticOperation::Trap.required_capabilities();
    let mut module = Module::new("retained_assert_chain");
    module.functions = vec![helper, AmdGpuDiagnosticOperation::Trap.declaration()];
    module
}

fn append_index(block: &mut BasicBlock, mode: u8, result: u32, loaded: u32) {
    if mode == 0 {
        block.operations.push(constant(result, Constant::U64(0)));
    } else {
        let value = if mode == 1 {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(loaded), scalar()),
                OperationKind::Load {
                    pointer: ValueId(25),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ));
            ValueId(loaded)
        } else {
            ValueId(1)
        };
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U64)),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value,
                to: Type::Scalar(ScalarType::U64),
            },
        ));
    }
}

fn append_condition(block: &mut BasicBlock, result: u32, index: u32, target: BlockId) {
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(result), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(index),
            rhs: ValueId(20),
        },
    ));
    block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(result),
        then_target: target,
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
}

type Rows = (
    Vec<RetainedLocalAllocationV1>,
    Vec<RetainedLocalAccessV1>,
    Vec<RetainedLocalControlV1>,
    Vec<RetainedLocalEdgeBindingV1>,
);

fn row_bytes(rows: &Rows) -> usize {
    rows.0.capacity() * size_of::<RetainedLocalAllocationV1>()
        + rows.1.capacity() * size_of::<RetainedLocalAccessV1>()
        + rows.2.capacity() * size_of::<RetainedLocalControlV1>()
        + rows.3.capacity() * size_of::<RetainedLocalEdgeBindingV1>()
}

fn reserve_rows(counts: [usize; 4], budget: &mut ArgumentBudgetV1<'_>) -> Rows {
    (
        helper_memory_vec_v1(counts[0], budget).unwrap(),
        helper_memory_vec_v1(counts[1], budget).unwrap(),
        helper_memory_vec_v1(counts[2], budget).unwrap(),
        helper_memory_vec_v1(counts[3], budget).unwrap(),
    )
}

fn pending(executable: &VerifiedCanonicalKernelIrModuleV12) -> Vec<RetainedHelperKindV1> {
    executable
        .module()
        .functions
        .iter()
        .map(|function| {
            if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
                RetainedHelperKindV1::Pending
            } else {
                RetainedHelperKindV1::NotHelper
            }
        })
        .collect()
}

fn edge_component() -> Module {
    let mut module = local_component(false);
    let body = module.functions[0].body.as_mut().unwrap();
    let mut entry = body.blocks.remove(0);
    let load = entry.operations.pop().unwrap();
    entry.operations.push(constant(3, Constant::U32(2)));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(42),
        arguments: vec![ValueId(3), ValueId(0)],
    });
    let mut middle = BasicBlock::new(BlockId(42));
    middle.parameters = vec![
        ValueDef::new(ValueId(30), scalar()),
        ValueDef::new(ValueId(31), scalar()),
    ];
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![ValueId(31), ValueId(30)],
    });
    let mut exit = BasicBlock::new(BlockId(5));
    exit.parameters = vec![
        ValueDef::new(ValueId(40), scalar()),
        ValueDef::new(ValueId(41), scalar()),
    ];
    exit.operations.push(load);
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    body.blocks = vec![entry, exit, middle];
    module
}

#[test]
fn genuine_raw_empty_multiblock_helpers_do_not_acquire_chain_authority() {
    with_source(true, |owner, inventory, budget| {
        assert!(inventory.functions().iter().any(|row| {
            row.function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper
                && row
                    .function
                    .body
                    .as_ref()
                    .is_some_and(|body| body.blocks.len() > 1)
        }));
        let associations: Vec<_> = owner
            .correspondence
            .lowered_functions
            .iter()
            .enumerate()
            .filter(|(_, row)| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .map(|(ordinal, _)| ordinal)
            .collect();
        assert_eq!(associations.len(), 3);
        assert_eq!(owner.empty_effect_helpers().iter().count(), 3);
        assert!(owner.helper_memory.control.is_empty());
        assert!(owner.helper_memory.edge_bindings.is_empty());
        owner
            .with_checked_helper_memory_v1(inventory, budget, |memory, budget| {
                for association in associations {
                    assert!(memory.local_frame(association, budget)?.is_none());
                }
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn physical_chain_transfer_preserves_all_four_families_and_simultaneous_edges() {
    let mut module = edge_component();
    let mut second = module.functions[0].clone();
    second.id = FunctionId::new("second_chain");
    module.functions.push(second);
    with_physical_component(module, |executable, inventory, effects| {
        // The graph is genuinely verified; this isolated copy meter is not
        // source-owner admission and does not account for the outer graph.
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut states = pending(executable);
        let rows = derive_retained_helper_physical_rows_v1(
            executable,
            inventory,
            effects,
            &mut states,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
            (2, 4, 6, 8)
        );
        let bytes = row_bytes(&rows);
        assert_eq!(budget.storage(), FLOOR + bytes);
        assert!(budget.peak_storage() > budget.storage());
        for function in 0..2 {
            assert_eq!(
                states[function],
                RetainedHelperKindV1::Local {
                    allocations: (function, function + 1),
                    accesses: (2 * function, 2 * function + 2),
                    control: (3 * function, 3 * function + 3),
                    edge_bindings: (4 * function, 4 * function + 4),
                }
            );
            let access = &rows.1[2 * function..2 * function + 2];
            assert_eq!(access[1].initializing_store(), Some(access[0].location()));
            assert_eq!(
                (access[0].location().block(), access[1].location().block()),
                (BlockId(7), BlockId(5))
            );
            for row in &rows.2[3 * function..3 * function + 3] {
                assert_eq!(row.function_ordinal(), function);
            }
            let control = &rows.2[3 * function..3 * function + 3];
            assert_eq!(
                control.iter().map(|row| row.block()).collect::<Vec<_>>(),
                [BlockId(7), BlockId(42), BlockId(5)]
            );
            assert_eq!(
                control[0].kind(),
                LocalFrameControlKindV1::Branch {
                    target: BlockId(42)
                }
            );
            assert_eq!(
                control[1].kind(),
                LocalFrameControlKindV1::Branch { target: BlockId(5) }
            );
            assert_eq!(control[2].kind(), LocalFrameControlKindV1::Return);
            let bindings = &rows.3[4 * function..4 * function + 4];
            for (row, (source, target, ordinal, argument, parameter, value)) in
                bindings.iter().zip([
                    (7, 42, 0, 3, 30, 2),
                    (7, 42, 1, 0, 31, 1),
                    (42, 5, 0, 31, 40, 1),
                    (42, 5, 1, 30, 41, 2),
                ])
            {
                assert_eq!(row.function_ordinal(), function);
                assert_eq!(
                    (row.source(), row.target(), row.successor(), row.ordinal()),
                    (BlockId(source), BlockId(target), 0, ordinal)
                );
                assert_eq!(
                    (
                        row.argument(),
                        row.parameter(),
                        row.ty(),
                        row.known_unsigned()
                    ),
                    (
                        ValueId(argument),
                        ValueId(parameter),
                        ScalarType::U32,
                        Some(value)
                    )
                );
            }
        }
        drop(rows);
        budget.release_storage(bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn verified_array_assert_chains_retain_actual_sink_and_latest_store_without_purity() {
    for mode in [0, 1] {
        with_physical_component(assert_chain(mode), |executable, inventory, effects| {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            assert_eq!(
                effects
                    .decision(inventory.functions()[0].coordinate, &mut budget)
                    .unwrap(),
                CanonicalKirCallEffectDecisionV1::CompleteNonempty
            );
            let raw =
                fe2o3_kernel_ir::analyze_interprocedural_effects_v1(executable.module()).unwrap();
            assert!(
                !raw.function(&executable.module().functions[0].id)
                    .unwrap()
                    .is_complete_and_pure()
            );
            let mut states = pending(executable);
            let rows = derive_retained_helper_physical_rows_v1(
                executable,
                inventory,
                effects,
                &mut states,
                &mut budget,
            )
            .unwrap();
            assert_eq!(
                (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                (
                    if mode == 0 { 1 } else { 2 },
                    if mode == 0 { 11 } else { 14 },
                    5,
                    0
                )
            );
            assert_eq!(
                rows.2
                    .iter()
                    .filter(|row| matches!(row.kind(), LocalFrameControlKindV1::Selected { .. }))
                    .count(),
                3
            );
            assert_eq!(
                rows.2
                    .iter()
                    .filter(|row| row.kind() == LocalFrameControlKindV1::InactiveTrap)
                    .count(),
                1
            );
            assert!(rows.2.iter().any(|row| row.block() == BlockId(99)
                && row.kind() == LocalFrameControlKindV1::InactiveTrap));
            for row in &rows.2 {
                assert_eq!(row.function_ordinal(), 0);
                if let LocalFrameControlKindV1::Selected {
                    value,
                    successor,
                    inactive,
                    ..
                } = row.kind()
                {
                    assert!(value);
                    assert_eq!((successor, inactive), (0, BlockId(99)));
                }
            }
            let read = rows
                .1
                .iter()
                .find(|row| {
                    row.location().block() == BlockId(3)
                        && row.kind() == LocalFrameAccessKindV1::Read
                })
                .unwrap();
            let latest = rows
                .1
                .iter()
                .find(|row| {
                    row.location().block() == BlockId(19)
                        && row.kind() == LocalFrameAccessKindV1::Write
                })
                .unwrap();
            assert_eq!(read.initializing_store(), Some(latest.location()));
            let bytes = row_bytes(&rows);
            assert_eq!(budget.storage(), FLOOR + bytes);
            assert_eq!(states[1], RetainedHelperKindV1::NotHelper);
            drop(rows);
            budget.release_storage(bytes).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        });
    }
}

fn reject_component(module: Module, operation: Option<usize>, reason: LocalFrameRefusalReasonV1) {
    with_physical_component(module, |executable, inventory, effects| {
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut states = pending(executable);
        let result = derive_retained_helper_physical_rows_v1(
            executable,
            inventory,
            effects,
            &mut states,
            &mut budget,
        );
        assert!(
            matches!(result, Err(ProductionPreRankedKirErrorV1::LocalFrame(
            LocalFrameErrorV1::Unsupported { function_ordinal: 0, operation: actual, reason: actual_reason }
        )) if actual == operation && actual_reason == reason)
        );
        // The constructor's enclosing scope owns this partial-output rollback.
        let retained = budget.storage() - FLOOR;
        assert!(retained > 0);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn real_chain_refusals_preserve_unknown_control_missing_initialization_and_effects() {
    reject_component(
        assert_chain(2),
        None,
        LocalFrameRefusalReasonV1::ControlFlow,
    );
    let mut length = assert_chain(0);
    length.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(20)))
        .unwrap()
        .kind = OperationKind::Constant(Constant::U64(0));
    reject_component(length, None, LocalFrameRefusalReasonV1::ControlFlow);
    let mut uninitialized = edge_component();
    let body = uninitialized.functions[0].body.as_mut().unwrap();
    let store = body.blocks[0].operations.remove(2);
    body.blocks[1].operations.push(store);
    reject_component(
        uninitialized,
        Some(0),
        LocalFrameRefusalReasonV1::UninitializedRead,
    );
    let mut effect = edge_component();
    let body = effect.functions[0].body.as_mut().unwrap();
    let OperationKind::Store { access, .. } = &mut body.blocks[0].operations[2].kind else {
        panic!("store")
    };
    access.volatile = true;
    reject_component(effect, Some(2), LocalFrameRefusalReasonV1::Effects);
    let mut extra = assert_chain(0);
    let mut block = BasicBlock::new(BlockId(123));
    block.terminator = Some(Terminator::Return { values: vec![] });
    extra.functions[0].body.as_mut().unwrap().blocks.push(block);
    reject_component(extra, None, LocalFrameRefusalReasonV1::ControlFlow);
}

#[test]
fn copied_chain_requires_exact_subject_ranges_and_prepaid_control_binding_capacity() {
    with_physical_component(edge_component(), |executable, _, _| {
        for hostile in 0..7 {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let counts = [
                1,
                2,
                if hostile == 3 { 0 } else { 3 },
                if hostile == 4 { 0 } else { 4 },
            ];
            let mut rows = reserve_rows(counts, &mut budget);
            let bytes = row_bytes(&rows);
            let live = budget.storage();
            let mut entered = false;
            let result = with_checked_local_frame_chain_function_v1(
                executable.verified_module_ref_v1(),
                0,
                &mut budget,
                |checked, budget| {
                    entered = true;
                    let mut ranges = [(0, 1), (0, 2), (0, 3), (0, 4)];
                    if hostile == 1 {
                        ranges[2].1 = 2;
                    }
                    if hostile == 2 {
                        ranges[3].1 = 3;
                    }
                    if hostile == 6 {
                        ranges[2] = (1, 0);
                    }
                    copy_retained_helper_rows_v1(
                        executable,
                        usize::from(hostile == 5),
                        ranges,
                        &checked,
                        (&mut rows.0, &mut rows.1, &mut rows.2, &mut rows.3),
                        budget,
                    )
                },
            );
            assert!(entered);
            if hostile == 0 {
                result.unwrap();
                assert_eq!(
                    (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                    (1, 2, 3, 4)
                );
            } else {
                assert_eq!(
                    result,
                    Err(LocalFrameErrorV1::Resource(
                        HelperMemoryResourceV1::Accounting
                    ))
                );
                match hostile {
                    3 => assert_eq!(
                        (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                        (1, 2, 0, 0)
                    ),
                    4 => assert_eq!(
                        (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                        (1, 2, 3, 0)
                    ),
                    _ => assert!(
                        rows.0.is_empty()
                            && rows.1.is_empty()
                            && rows.2.is_empty()
                            && rows.3.is_empty()
                    ),
                }
            }
            assert_eq!(budget.storage(), live);
            drop(rows);
            budget.release_storage(bytes).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
        with_physical_component(edge_component(), |foreign, _, _| {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut rows = reserve_rows([1, 2, 3, 4], &mut budget);
            let bytes = row_bytes(&rows);
            let result = with_checked_local_frame_chain_function_v1(
                executable.verified_module_ref_v1(),
                0,
                &mut budget,
                |checked, budget| {
                    copy_retained_helper_rows_v1(
                        foreign,
                        0,
                        [(0, 1), (0, 2), (0, 3), (0, 4)],
                        &checked,
                        (&mut rows.0, &mut rows.1, &mut rows.2, &mut rows.3),
                        budget,
                    )
                },
            );
            assert_eq!(
                result,
                Err(LocalFrameErrorV1::Resource(
                    HelperMemoryResourceV1::Accounting
                ))
            );
            assert!(
                rows.0.is_empty() && rows.1.is_empty() && rows.2.is_empty() && rows.3.is_empty()
            );
            assert_eq!(budget.storage(), FLOOR + bytes);
            drop(rows);
            budget.release_storage(bytes).unwrap();
        });
    });
}

#[test]
fn edge_copy_work_has_derived_partial_binding_and_final_boundaries() {
    with_physical_component(edge_component(), |executable, inventory, effects| {
        // Core: header/entry18 + signature1 + census24 + reserves14 + publish35
        // + definition sort/window103 + block sort24 + visits/lookups38 + epochs35
        // + operations153 + dispatch12 + simultaneous edges83 + return7
        // + block census3 + cells34 = 584. Eight definition keys, three blocks.
        const CORE: usize = 18 + 1 + 24 + 14 + 35 + 103 + 24 + 38 + 35 + 153 + 12 + 83 + 7 + 3 + 34;
        const EXACT: usize = 35 + 12 + 1 + CORE + 12 + 30 + 3 + 8 + 9 + 12 + 5;
        assert_eq!((CORE, EXACT), (584, 711));
        for limit in [705, EXACT - 1, EXACT] {
            let mut work = Work::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut states = pending(executable);
            let result = derive_retained_helper_physical_rows_v1(
                executable,
                inventory,
                effects,
                &mut states,
                &mut budget,
            );
            match (limit, result) {
                (
                    705,
                    Err(ProductionPreRankedKirErrorV1::LocalFrame(LocalFrameErrorV1::Resource(
                        HelperMemoryResourceV1::Work(error),
                    ))),
                ) => {
                    assert_eq!(
                        (error.actual(), error.limit(), budget.work()),
                        (706, 705, 703)
                    );
                }
                (
                    710,
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            HelperMemoryResourceV1::Work(error),
                        ),
                    )),
                ) => {
                    assert_eq!(
                        (error.actual(), error.limit(), budget.work()),
                        (711, 710, 706)
                    );
                }
                (711, Ok(rows)) => {
                    assert_eq!(
                        (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                        (1, 2, 3, 4)
                    );
                    assert_eq!(budget.work(), EXACT);
                    assert_eq!(budget.storage(), FLOOR + row_bytes(&rows));
                    drop(rows);
                }
                (_, result) => panic!("wrong source-derived binding-copy phase: {result:?}"),
            }
            let retained = budget.storage() - FLOOR;
            assert!(retained > 0);
            assert!(budget.peak_storage() > budget.storage());
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

#[test]
fn new_output_buffers_are_paid_before_the_classifier_and_use_actual_capacities() {
    with_physical_component(edge_component(), |executable, inventory, effects| {
        let mut work = Work::new(usize::MAX);
        let mut setup = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let rows = reserve_rows([1, 2, 3, 4], &mut setup);
        let before_control = rows.0.capacity() * size_of::<RetainedLocalAllocationV1>()
            + rows.1.capacity() * size_of::<RetainedLocalAccessV1>();
        let before_binding =
            before_control + rows.2.capacity() * size_of::<RetainedLocalControlV1>();
        let setup_bytes = row_bytes(&rows);
        assert_eq!(setup.storage(), setup_bytes);
        drop(rows);
        setup.release_storage(setup_bytes).unwrap();
        assert_eq!(setup.storage(), 0);
        for (previous, needed, prefix) in [
            (before_control, 3 * size_of::<RetainedLocalControlV1>(), 44),
            (
                before_binding,
                4 * size_of::<RetainedLocalEdgeBindingV1>(),
                47,
            ),
        ] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + previous + needed - 1);
            budget.reserve_storage(FLOOR).unwrap();
            let mut states = pending(executable);
            let result = derive_retained_helper_physical_rows_v1(
                executable,
                inventory,
                effects,
                &mut states,
                &mut budget,
            );
            assert!(
                matches!(result, Err(ProductionPreRankedKirErrorV1::Lowering(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(HelperMemoryResourceV1::Storage(error))
            )) if error.actual() == FLOOR + previous + needed)
            );
            assert_eq!(budget.work(), prefix);
            assert_eq!(budget.storage(), FLOOR + previous);
            assert_eq!(budget.failed_storage(), Some(FLOOR + previous + needed));
            budget.release_storage(previous).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

#[test]
fn new_producer_queries_preserve_exact_guard_and_census_work_boundaries() {
    with_physical_component(local_component(false), |executable, _, _| {
        for (control, exact) in [(true, 289), (false, 288)] {
            for limit in [exact - 1, exact] {
                let mut work = Work::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                budget.reserve_storage(FLOOR).unwrap();
                let mut entered = false;
                let result = with_checked_local_frame_chain_function_v1(
                    executable.verified_module_ref_v1(),
                    0,
                    &mut budget,
                    |checked, budget| {
                        entered = true;
                        assert_eq!(budget.work(), 283);
                        if control {
                            assert_eq!(checked.control(budget)?.len(), 1);
                        } else {
                            assert!(checked.edge_bindings(budget)?.is_empty());
                        }
                        Ok(())
                    },
                );
                assert!(entered);
                if limit == exact {
                    result.unwrap();
                    assert_eq!(budget.work(), exact);
                } else {
                    assert!(
                        matches!(result, Err(LocalFrameErrorV1::Resource(HelperMemoryResourceV1::Work(error)))
                        if error.actual() == exact && error.limit() == limit)
                    );
                    assert_eq!(budget.work(), if control { 288 } else { 283 });
                }
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    });
}

#[test]
fn successful_copy_keeps_output_rows_live_through_clean_and_accounting_callbacks() {
    with_physical_component(edge_component(), |executable, _, _| {
        for loss in 0..4 {
            for mode in 0..3 {
                let mut work = Work::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                budget.reserve_storage(FLOOR).unwrap();
                let mut rows = reserve_rows([1, 2, 3, 4], &mut budget);
                let bytes = row_bytes(&rows);
                let incoming = budget.storage();
                let mut entered = false;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    with_checked_local_frame_chain_function_v1(
                        executable.verified_module_ref_v1(),
                        0,
                        &mut budget,
                        |checked, budget| {
                            entered = true;
                            copy_retained_helper_rows_v1(
                                executable,
                                0,
                                [(0, 1), (0, 2), (0, 3), (0, 4)],
                                &checked,
                                (&mut rows.0, &mut rows.1, &mut rows.2, &mut rows.3),
                                budget,
                            )?;
                            if loss != 0 {
                                let floor = match loss {
                                    1 => budget.storage() - 1,
                                    2 => incoming - 1,
                                    _ => FLOOR - 1,
                                };
                                budget.release_storage(budget.storage() - floor)?;
                            }
                            match mode {
                                0 => Ok(()),
                                1 => Err(LocalFrameErrorV1::Unsupported {
                                    function_ordinal: 0,
                                    operation: None,
                                    reason: LocalFrameRefusalReasonV1::Index,
                                }),
                                _ => std::panic::panic_any("retained chain callback"),
                            }
                        },
                    )
                }));
                assert!(entered);
                assert_eq!(
                    (rows.0.len(), rows.1.len(), rows.2.len(), rows.3.len()),
                    (1, 2, 3, 4)
                );
                if loss != 0 {
                    assert_eq!(
                        outcome.unwrap(),
                        Err(LocalFrameErrorV1::Resource(
                            HelperMemoryResourceV1::Accounting
                        ))
                    );
                    let expected = match loss {
                        1 => incoming,
                        2 => incoming - 1,
                        _ => FLOOR - 1,
                    };
                    assert_eq!(budget.storage(), expected);
                    // Only the test repairs a deliberately stolen reservation.
                    budget.reserve_storage(incoming - expected).unwrap();
                } else if mode == 2 {
                    assert_eq!(
                        outcome.unwrap_err().downcast_ref::<&str>(),
                        Some(&"retained chain callback")
                    );
                } else if mode == 1 {
                    assert_eq!(
                        outcome.unwrap(),
                        Err(LocalFrameErrorV1::Unsupported {
                            function_ordinal: 0,
                            operation: None,
                            reason: LocalFrameRefusalReasonV1::Index,
                        })
                    );
                } else {
                    outcome.unwrap().unwrap();
                }
                assert_eq!(budget.storage(), incoming);
                drop(rows);
                budget.release_storage(bytes).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    });
}

#[test]
fn physical_transfer_keeps_latest_store_and_unknown_alias_facts_across_edges() {
    for alias in [false, true] {
        for restore in [false, true] {
            let mut module = edge_component();
            let function = &mut module.functions[0];
            function.signature.parameters.push(scalar());
            let body = function.body.as_mut().unwrap();
            body.parameters.push(ValueId(1000));
            let middle = &mut body.blocks[2];
            let pointer = if alias {
                middle.operations.extend([
                    constant(60, Constant::Index(0)),
                    Operation::effect_free(
                        ValueDef::new(ValueId(61), pointer()),
                        OperationKind::GetElementPointer {
                            base: ValueId(1),
                            offset: ValueId(60),
                        },
                    ),
                ]);
                ValueId(61)
            } else {
                ValueId(1)
            };
            middle.operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value: ValueId(1000),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ));
            if restore {
                middle.operations.push(Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(1),
                        value: ValueId(31),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                ));
            }
            let latest_operation = middle.operations.len() - 1;
            middle.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(50), scalar()),
                OperationKind::Load {
                    pointer: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ));
            middle.terminator = Some(Terminator::Branch {
                target: BlockId(5),
                arguments: vec![ValueId(50), ValueId(30)],
            });
            with_physical_component(module, |executable, inventory, effects| {
                let mut work = Work::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                budget.reserve_storage(FLOOR).unwrap();
                let mut states = pending(executable);
                let rows = derive_retained_helper_physical_rows_v1(
                    executable,
                    inventory,
                    effects,
                    &mut states,
                    &mut budget,
                )
                .unwrap();
                let latest = rows
                    .1
                    .iter()
                    .find(|row| {
                        row.location().block() == BlockId(42)
                            && row.location().operation() == latest_operation
                    })
                    .unwrap();
                assert_eq!(latest.kind(), LocalFrameAccessKindV1::Write);
                let reads: Vec<_> = rows
                    .1
                    .iter()
                    .filter(|row| row.kind() == LocalFrameAccessKindV1::Read)
                    .collect();
                assert_eq!(reads.len(), 2);
                for read in reads {
                    assert_eq!(read.initializing_store(), Some(latest.location()));
                    assert_eq!((read.allocation(), read.cell()), (0, 0));
                }
                assert_eq!(rows.3.len(), 4);
                assert_eq!(rows.3[2].known_unsigned(), restore.then_some(1));
                assert_eq!(rows.3[3].known_unsigned(), Some(2));
                let bytes = row_bytes(&rows);
                assert_eq!(budget.storage(), FLOOR + bytes);
                drop(rows);
                budget.release_storage(bytes).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            });
        }
    }
}
