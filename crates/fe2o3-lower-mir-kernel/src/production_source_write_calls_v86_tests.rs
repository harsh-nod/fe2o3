use super::*;
use scoped_raw_admission_v29::issued_role_tests_v29::{
    copied_issued_rows_v18, issued_rows_v18, run_issued_role_owner_access_v86,
};

const BOOL: SemanticTypeIdV1 = REFERENCE;
const LIMIT: usize = 1_000_000_000;

fn run_optimized_write_v87(
    count: u32,
    fault: Option<u8>,
    work: usize,
    storage: usize,
    reached: &std::cell::Cell<bool>,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_source_v87(
        write_owner_count_v87(true, true, 0, count),
        fe2o3_kernel_descriptor::AccessMode::WriteOnly,
        work,
        storage,
        &std::cell::Cell::new(None),
        |source, budget| {
            let floor = budget.storage();
            let (output, (), receipt) = source.with_checked_mixed_fixedpoint_optimization_v18(
                budget,
                |original, optimized, budget| {
                    slice_view_v1::test_optimized_writes_v87(
                        original,
                        optimized,
                        count as usize,
                        fault,
                        reached,
                        budget,
                    )?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            )?;
            assert_eq!(
                receipt.retained_storage(),
                size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
            );
            assert_eq!(output.execution().policy_version(), 11);
            drop(output);
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    )
}

#[test]
fn optimized_thread_write_transports_actual_suffix_and_commoned_producers() {
    for count in [1, 2, 4] {
        let reached = std::cell::Cell::new(false);
        run_optimized_write_v87(count, None, LIMIT, LIMIT, &reached)
            .0
            .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn optimized_thread_write_rejects_wrong_producer_index_and_root_descendants() {
    for fault in 0..4 {
        let reached = std::cell::Cell::new(false);
        assert!(
            run_optimized_write_v87(2, Some(fault), LIMIT, LIMIT, &reached)
                .0
                .is_err()
        );
        assert!(
            reached.get(),
            "mutation {fault} must reach checked optimizer transport"
        );
    }
}

#[test]
fn optimized_thread_write_transaction_has_exact_resource_boundaries() {
    let reached = std::cell::Cell::new(false);
    let (result, work, storage) = run_optimized_write_v87(2, None, LIMIT, LIMIT, &reached);
    result.unwrap();
    assert!(reached.get());
    let reached = std::cell::Cell::new(false);
    let (exact, exact_work, exact_storage) =
        run_optimized_write_v87(2, None, work, storage, &reached);
    exact.unwrap();
    assert!(reached.get());
    assert_eq!((exact_work, exact_storage), (work, storage));
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        let reached = std::cell::Cell::new(false);
        assert!(
            run_optimized_write_v87(2, None, work, storage, &reached)
                .0
                .is_err()
        );
    }
}

// This is admitted semantic MIR with captured SSA occurrences, not a rustc receipt.
fn write_owner(
    consume_bool: bool,
    copied_value: bool,
    receiver: usize,
) -> ProductionSemanticSsaOwnerV1 {
    write_owner_count_v87(consume_bool, copied_value, receiver, 1)
}

pub(super) fn write_owner_count_v87(
    consume_bool: bool,
    copied_value: bool,
    receiver: usize,
    count: u32,
) -> ProductionSemanticSsaOwnerV1 {
    assert!(receiver < 2);
    assert!((1..=252).contains(&count));
    let base = owner_with_shape_uncaptured(1, 0);
    let mut types = base.source_semantic().types()[..8].to_vec();
    types[BOOL.index() as usize] = declaration(
        7,
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
    );
    let locals = [UNIT, CARRIER, CARRIER, BORROW, WITNESS, BOOL, U32]
        .into_iter()
        .enumerate()
        .map(|(local, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + local as u8; 32]),
                ty,
                match local {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    2 => SemanticLocalRoleV1::Argument(1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                provenance(),
            )
        })
        .collect();
    let constant = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 4).unwrap()),
        ))
    };
    let mut statements = vec![assign(
        3,
        BORROW,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: place(receiver as u32 + 1, CARRIER),
        },
    )];
    if copied_value {
        statements.push(assign(6, U32, SemanticRvalueKindV1::Use(constant())));
    }
    let mut blocks = vec![block(0, statements, call(1, vec![], 4, WITNESS, 1))];
    for ordinal in 1..=count {
        blocks.push(block(
            u8::try_from(ordinal).unwrap(),
            if ordinal == 1 {
                vec![]
            } else {
                vec![assign(
                    3,
                    BORROW,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(receiver as u32 + 1, CARRIER),
                    },
                )]
            },
            call(
                2,
                vec![
                    SemanticOperandV1::Move(place(3, BORROW)),
                    if ordinal == count {
                        SemanticOperandV1::Move(place(4, WITNESS))
                    } else {
                        SemanticOperandV1::Copy(place(4, WITNESS))
                    },
                    if copied_value {
                        SemanticOperandV1::Copy(place(6, U32))
                    } else {
                        constant()
                    },
                ],
                5,
                BOOL,
                ordinal + 1,
            ),
        ));
    }
    if consume_bool {
        blocks.push(block(
            u8::try_from(count + 1).unwrap(),
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(5, BOOL)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, count + 2),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, count + 3),
                )
                .unwrap(),
            },
        ));
        blocks.push(block(
            u8::try_from(count + 2).unwrap(),
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, count + 3)),
        ));
    }
    blocks.push(block(
        u8::try_from(if consume_bool { count + 3 } else { count + 1 }).unwrap(),
        vec![assign(
            0,
            UNIT,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                UNIT,
                SemanticConstantValueV1::ZeroSized,
            ))),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([20; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([20; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([20; 32]),
        provenance(),
        abi(20, true, &[CARRIER, CARRIER], UNIT),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"issued_pointer_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([23; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                21,
                abi(21, false, &[], WITNESS),
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: WITNESS,
                    raw_index: INDEX,
                },
            ),
            intrinsic(
                22,
                abi_with_bool_result_v86(22, false, &[BORROW, WITNESS, U32], BOOL, true),
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    disjoint_slice: CARRIER,
                    witness: WITNESS,
                    element: U32,
                    raw_index: INDEX,
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                    kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
                },
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

#[test]
fn original_thread_write_replays_receiver_witness_value_and_bool_result() {
    for consume_bool in [false, true] {
        for copied in [false, true] {
            for receiver in 0..2 {
                let reached = std::cell::Cell::new(false);
                let (result, _, _) = run_issued_role_owner_access_v86(
                    write_owner(consume_bool, copied, receiver),
                    fe2o3_kernel_descriptor::AccessMode::WriteOnly,
                    LIMIT,
                    LIMIT,
                    &std::cell::Cell::new(None),
                    |original, budget| {
                        let rows = issued_rows_v18(original);
                        assert_eq!(
                            (
                                rows.sources.len(),
                                rows.writes.len(),
                                rows.issuers.len(),
                                rows.accesses.len(),
                                rows.lengths.len(),
                                rows.selected.len()
                            ),
                            (1, 1, 0, 0, 0, 0)
                        );
                        let row = rows.writes[0];
                        assert_eq!(row.root_parameter, receiver);
                        assert_eq!(row.block.index(), 1);
                        assert_eq!(row.element, ScalarType::U32);
                        assert_eq!(row.index_space, SemanticDisjointIndexSpaceV1::Index1d);
                        assert!(!row.disjoint);
                        assert_eq!(row.tail.predicate, row.tail.extent);
                        if consume_bool {
                            assert!(row.definition.is_some());
                        }
                        assert_eq!(
                            rows.retained_storage(budget)?,
                            rows.sources.capacity() * size_of::<PendingSourceIssuedSiteV29>()
                                + rows.writes.capacity() * size_of::<PendingSourceWriteV86>()
                        );
                        let floor = budget.storage();
                        scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                            original, 0, rows, budget,
                        )?;
                        assert_eq!(budget.storage(), floor);
                        reached.set(true);
                        Ok(())
                    },
                );
                result.unwrap();
                assert!(
                    reached.get(),
                    "consume={consume_bool}, copied={copied}, receiver={receiver}"
                );
            }
        }
    }
}

#[test]
fn original_thread_write_copied_recipe_cannot_change_source_facts() {
    for fault in 0..12 {
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_owner_access_v86(
            write_owner(true, false, 0),
            fe2o3_kernel_descriptor::AccessMode::WriteOnly,
            LIMIT,
            LIMIT,
            &std::cell::Cell::new(None),
            |original, budget| {
                let floor = budget.storage();
                let mut copy = copied_issued_rows_v18(issued_rows_v18(original), budget)?;
                let owned = budget.storage() - floor;
                match fault {
                    0 => copy.writes.clear(),
                    1 => copy.sources.clear(),
                    2 => copy.writes.push(copy.writes[0]),
                    3 => copy.writes[0].root_parameter = 1,
                    4 => copy.writes[0].root_input = copy.writes[0].index,
                    5 => copy.writes[0].index = copy.writes[0].tail.length,
                    6 => copy.writes[0].value = copy.writes[0].index,
                    7 => copy.writes[0].tail.predicate = copy.writes[0].tail.zero,
                    8 => copy.writes[0].definition = None,
                    9 => copy.writes[0].anchor += 1,
                    10 => copy.writes[0].disjoint = true,
                    11 => copy.writes[0].element = ScalarType::F32,
                    _ => unreachable!(),
                }
                let refused = scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                    original, 0, &copy, budget,
                );
                assert!(refused.is_err(), "fault={fault}");
                drop(copy);
                budget.release_storage(owned)?;
                assert_eq!(budget.storage(), floor);
                reached.set(true);
                Ok(())
            },
        );
        // The original owner keeps any observed refusal sticky beyond this callback.
        assert!(reached.get(), "fault={fault}: {result:?}");
        assert!(result.is_err(), "fault={fault}");
    }
}

#[test]
fn original_thread_write_transaction_has_exact_resource_boundaries() {
    let run = |work, storage| {
        run_issued_role_owner_access_v86(
            write_owner(true, true, 1),
            fe2o3_kernel_descriptor::AccessMode::WriteOnly,
            work,
            storage,
            &std::cell::Cell::new(None),
            |original, _| {
                assert_eq!(issued_rows_v18(original).writes.len(), 1);
                Ok(())
            },
        )
    };
    let (result, work, storage) = run(LIMIT, LIMIT);
    result.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(run(work - 1, storage).0.is_err());
    assert!(run(work, storage - 1).0.is_err());
}

thread_local! {
    static WRITE_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static WRITE_VISITED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_write(
    pending: &mut PendingScopedRootEmissionV29,
    _: &ExecutionInstancesV29<'_>,
    _: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = WRITE_FAULT.get();
    let body = pending.function.body.as_mut().unwrap();
    let other = body.parameters[1];
    let mut length = None;
    let mut stores = 0;
    for block in &body.blocks {
        for operation in &block.operations {
            budget.charge_work(2)?;
            if matches!(operation.kind, OperationKind::SliceLength { .. }) {
                assert!(length.replace(operation.results[0].id).is_none());
            }
            stores += usize::from(matches!(operation.kind, OperationKind::GuardedStore { .. }));
        }
    }
    assert_eq!(stores, 1);
    let length = length.unwrap();
    let mut changes = 0;
    for block in &mut body.blocks {
        for operation in &mut block.operations {
            budget.charge_work(2)?;
            match (&mut operation.kind, fault) {
                (OperationKind::SliceLength { slice } | OperationKind::SliceData { slice }, 1) => {
                    *slice = other;
                    changes += 1;
                }
                (OperationKind::Compare { lhs, .. }, 2) => {
                    *lhs = length;
                    changes += 1;
                }
                (OperationKind::Select { true_value, .. }, 2) => {
                    *true_value = length;
                    changes += 1;
                }
                (OperationKind::Constant(Constant::U32(value)), 3) if *value == 17 => {
                    *value = 19;
                    changes += 1;
                }
                (OperationKind::GuardedStore { access, .. }, 4) => {
                    access.volatile = true;
                    changes += 1;
                }
                (
                    OperationKind::GuardedStore {
                        pointer,
                        value,
                        access,
                        ..
                    },
                    5,
                ) => {
                    operation.kind = OperationKind::Store {
                        pointer: *pointer,
                        value: *value,
                        access: *access,
                    };
                    changes += 1;
                }
                _ => {}
            }
        }
    }
    if fault == 6 {
        body.parameters.swap(0, 1);
        changes += 1;
    }
    assert_eq!(
        changes,
        match fault {
            0 => 0,
            1 | 2 => 2,
            _ => 1,
        }
    );
    WRITE_VISITED.set(true);
    Ok(())
}

#[test]
fn original_thread_write_rejects_consistent_physical_tail_with_wrong_source_inputs() {
    let _restore =
        RestoreObserver(ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_write)));
    for fault in 0..=6 {
        WRITE_FAULT.set(fault);
        WRITE_VISITED.set(false);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_owner_access_v86(
            write_owner(true, false, 0),
            fe2o3_kernel_descriptor::AccessMode::WriteOnly,
            LIMIT,
            LIMIT,
            &std::cell::Cell::new(None),
            |_, _| {
                reached.set(true);
                Ok(())
            },
        );
        assert!(WRITE_VISITED.get(), "fault={fault}: {result:?}");
        if fault == 0 {
            result.unwrap();
            assert!(reached.get());
        } else {
            assert!(
                result.is_err() && !reached.get(),
                "fault={fault}: {result:?}"
            );
        }
    }
}
