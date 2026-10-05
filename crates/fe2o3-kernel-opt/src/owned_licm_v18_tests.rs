use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18, CanonicalKirMemorySsaV18, check_canonical_kir_licm_v18 as check_pair,
};
use fe2o3_kernel_ir::{
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, StorageLayoutIdV1 as LayoutId,
    StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1 as Storage,
    VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

const LAYOUTS: fe2o3_kernel_ir::StorageLayoutLimitsV1 = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
    rows: 2,
    edges: 0,
    containment_depth: 1,
    object_bytes: 4,
};

fn storage_fixture() -> Module {
    let mut module = fixture();
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 1,
            alignment: 1,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U8),
        },
    ];
    let blocks = blocks(&mut module);
    blocks[0].operations.extend([
        op(
            200,
            Type::Execution(Role::Context),
            OperationKind::Execution(Execution::ContextIssue),
        ),
        op(
            201,
            Type::Execution(Role::Workgroup),
            OperationKind::Execution(Execution::WorkgroupDerive {
                context: ValueId(200),
            }),
        ),
        op(
            202,
            Type::pointer(
                Type::StorageObject(LayoutId(0)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            OperationKind::Alloca {
                element: Type::StorageObject(LayoutId(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    ]);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    blocks[3].operations.extend([
        Operation::new(
            vec![],
            OperationKind::Storage(Storage::WriteValue {
                address: ValueId(202),
                value: ValueId(34),
                access,
            }),
        ),
        op(
            203,
            u32_ty(),
            OperationKind::Storage(Storage::ReadValue {
                address: ValueId(202),
                access,
            }),
        ),
        op(
            204,
            u32_ty(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(203),
            },
        ),
        op(
            205,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
    ]);
    blocks[5].operations.push(Operation::new(
        vec![],
        OperationKind::Execution(Execution::ScopeEnd {
            workgroup: ValueId(201),
            discarded: vec![],
        }),
    ));
    module
}

fn admit_v18(module: &Module) -> (Owner18, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner18::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget)
            .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

fn with_input_v18(module: &Module, run: impl FnOnce(&Owner18, &mut Budget<'_>)) {
    let (owner, retained) = admit_v18(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(retained + 37).unwrap();
    let floor = budget.storage();
    run(&owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 37);
}

fn settle(tail: OwnedLicmContinuationV18, budget: &mut Budget<'_>) {
    let retained = tail.retained_storage();
    drop(tail);
    budget.release_storage(retained).unwrap();
}

#[test]
fn owned_v18_licm_forwards_exact_caller_layout_limits_without_fallback() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrReplayAdmissionErrorV18 as Admission, StorageLayoutErrorV1 as Layout,
        StorageLayoutProblemV1 as Problem,
    };
    for (layouts, problem) in [
        (
            fe2o3_kernel_ir::StorageLayoutLimitsV1 { rows: 1, ..LAYOUTS },
            Problem::Rows,
        ),
        (
            fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                object_bytes: 3,
                ..LAYOUTS
            },
            Problem::Size,
        ),
    ] {
        with_input_v18(&storage_fixture(), |input, budget| {
            let floor = budget.storage();
            let result = prepare_owned_licm_v18(input, layouts, budget);
            assert!(matches!(result,
                Err(Error::AdmissionV18(Admission::Layout(Layout::Invalid {
                    row: 0,
                    problem: actual,
                }))) if actual == problem
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
    with_input_v18(&storage_fixture(), |input, budget| {
        let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        assert_eq!(
            tail.output().module().storage_layouts,
            input.module().storage_layouts
        );
        assert!(tail.origins().iter().any(|row| row.hoist.is_some()));
        settle(tail, budget);
    });
}

#[test]
fn owned_v18_licm_moves_total_invariants_and_preserves_storage_execution_and_memory_order() {
    with_input_v18(&storage_fixture(), |input, budget| {
        let floor = budget.storage();
        let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(tail.retained_storage()).unwrap();
        assert_eq!(
            tail.origins()
                .iter()
                .filter(|row| row.hoist.is_some())
                .count(),
            5
        );
        assert_eq!(
            tail.output().module().storage_layouts,
            input.module().storage_layouts
        );
        assert!(!tail.grants_authority());
        let (pair, ps) = tail.replay_against(input, budget).unwrap();
        budget.reserve_storage(ps.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), tail.output()));
        assert!(!pair.grants_authority());
        for row in pair.origins() {
            let original = &input.module().functions[0].body.as_ref().unwrap().blocks
                [row.input.block.block as usize]
                .operations[row.input.operation as usize];
            let final_op = &tail.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[row.output.block.block as usize]
                .operations[row.output.operation as usize];
            assert_eq!(original, final_op);
            if row.hoist.is_some() {
                assert_eq!(row.input.block.block, 3);
                assert_eq!(row.output.block.block, 0);
            } else {
                assert_eq!(row.input.block, row.output.block);
            }
            if matches!(
                original.kind,
                OperationKind::Execution(_)
                    | OperationKind::Storage(_)
                    | OperationKind::Store { .. }
                    | OperationKind::Alloca { .. }
            ) || original
                .results
                .iter()
                .any(|r| [ValueId(204), ValueId(205), ValueId(35)].contains(&r.id))
            {
                assert!(row.hoist.is_none());
            }
        }
        drop(pair);
        budget.release_storage(ps.retained_storage()).unwrap();
        settle(tail, budget);
    });
}

#[test]
fn owned_v18_licm_rebuilds_memory_ssa_for_the_actual_final_owner() {
    with_input_v18(&storage_fixture(), |input, budget| {
        let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let (before, bs) = CanonicalKirInventoryV18::derive_v18(input, budget).unwrap();
        budget.reserve_storage(bs.retained_storage()).unwrap();
        let (after, as_) = CanonicalKirInventoryV18::derive_v18(tail.output(), budget).unwrap();
        budget.reserve_storage(as_.retained_storage()).unwrap();
        let (old, os) =
            CanonicalKirMemorySsaV18::derive_v18(&before, Default::default(), budget).unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (new, ns) =
            CanonicalKirMemorySsaV18::derive_v18(&after, Default::default(), budget).unwrap();
        budget.reserve_storage(ns.retained_storage()).unwrap();
        assert!(std::ptr::eq(old.inventory().owner(), input));
        assert!(std::ptr::eq(new.inventory().owner(), tail.output()));
        assert!(!std::ptr::eq(
            old.inventory().owner(),
            new.inventory().owner()
        ));
        assert!(old.belongs_to(&before));
        assert!(new.belongs_to(&after));
        assert!(!old.belongs_to(&after));
        assert!(!new.belongs_to(&before));
        for row in tail.origins() {
            let old_node = old.operation(row.input, budget).unwrap();
            let new_node = new.operation(row.output, budget).unwrap();
            assert_eq!(old_node.is_some(), new_node.is_some());
            if row.hoist.is_some() {
                assert!(old_node.is_none());
            }
        }
        drop(new);
        drop(old);
        drop(after);
        drop(before);
        budget
            .release_storage(
                ns.retained_storage()
                    + os.retained_storage()
                    + as_.retained_storage()
                    + bs.retained_storage(),
            )
            .unwrap();
        settle(tail, budget);
    });
}

#[test]
fn owned_v18_licm_independent_pair_rejects_same_count_lineage_storage_and_cfg_substitutions() {
    with_input_v18(&storage_fixture(), |input, budget| {
        let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        for fault in 0..4 {
            let mut rows = tail.origins().to_vec();
            match fault {
                0 => rows[0].input = rows[1].input,
                1 => rows[0].output = rows[1].output,
                2 => {
                    rows.iter_mut()
                        .find(|r| r.hoist.is_some())
                        .unwrap()
                        .hoist
                        .as_mut()
                        .unwrap()
                        .sequence = 99
                }
                3 => {
                    rows.iter_mut()
                        .find(|r| r.hoist.is_some())
                        .unwrap()
                        .hoist
                        .as_mut()
                        .unwrap()
                        .header
                        .block = 0
                }
                _ => unreachable!(),
            }
            let floor = budget.storage();
            assert!(
                matches!(
                    check_pair(input, tail.output(), &rows, Default::default(), budget),
                    Err(PairError::Mismatch(_))
                ),
                "fault {fault}"
            );
            assert_eq!(budget.storage(), floor);
        }
        for fault in 0..3 {
            let mut candidate = tail.output().module().clone();
            match fault {
                0 => {
                    candidate.storage_layouts[1].kind = StorageLayoutKindV1::Scalar(ScalarType::I8)
                }
                1 => blocks(&mut candidate)[2].terminator = Some(conditional(3, 50, 40)),
                2 => {
                    blocks(&mut candidate)[0]
                        .operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|r| r.id == ValueId(30)))
                        .unwrap()
                        .kind = OperationKind::Constant(Constant::U32(9))
                }
                _ => unreachable!(),
            }
            let (foreign, retained) = admit_v18(&candidate);
            budget.reserve_storage(retained).unwrap();
            let floor = budget.storage();
            assert!(
                matches!(
                    check_pair(input, &foreign, tail.origins(), Default::default(), budget),
                    Err(PairError::Mismatch(_))
                ),
                "endpoint fault {fault}"
            );
            assert_eq!(budget.storage(), floor);
            drop(foreign);
            budget.release_storage(retained).unwrap();
        }
        settle(tail, budget);
    });
}

#[test]
fn owned_v18_licm_independent_pair_refuses_a_well_typed_speculated_memory_operation() {
    with_input_v18(&storage_fixture(), |input, budget| {
        let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let mut candidate = tail.output().module().clone();
        let mut rows = tail.origins().to_vec();
        let body = blocks(&mut candidate);
        let at = body[3]
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Store { .. }))
            .unwrap();
        let destination = body[0].operations.len();
        let moved = body[3].operations.remove(at);
        body[0].operations.push(moved);
        for row in &mut rows {
            if row.output.block.block == 3 {
                if row.output.operation as usize == at {
                    row.output.block.block = 0;
                    row.output.operation = destination as u32;
                    row.hoist = Some(Hoist {
                        header: Block {
                            function: row.input.block.function,
                            block: 1,
                        },
                        sequence: 5,
                    });
                } else if row.output.operation as usize > at {
                    row.output.operation -= 1;
                }
            }
        }
        let (foreign, retained) = admit_v18(&candidate);
        budget.reserve_storage(retained).unwrap();
        let floor = budget.storage();
        assert!(matches!(
            check_pair(input, &foreign, &rows, Default::default(), budget),
            Err(PairError::Mismatch(
                "eligible total operation and exact loop/preheader"
            ))
        ));
        assert_eq!(budget.storage(), floor);
        drop(foreign);
        budget.release_storage(retained).unwrap();
        settle(tail, budget);
    });
}

#[test]
fn owned_v18_licm_handles_nested_duplicate_edges_and_irreducible_storage_graphs() {
    for mode in 0..3 {
        let mut module = if mode == 0 {
            nested()
        } else if mode == 1 {
            fixture()
        } else {
            irreducible()
        };
        module.storage_layouts.push(StorageLayoutV1 {
            size: 1,
            alignment: 1,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U8),
        });
        if mode == 1 {
            blocks(&mut module)[4].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(20),
                then_arguments: vec![ValueId(35)],
                else_target: BlockId(20),
                else_arguments: vec![ValueId(35)],
            });
        }
        with_input_v18(&module, |input, budget| {
            let tail = prepare_owned_licm_v18(input, LAYOUTS, budget).unwrap();
            budget.reserve_storage(tail.retained_storage()).unwrap();
            let (pair, ps) = tail.replay_against(input, budget).unwrap();
            budget.reserve_storage(ps.retained_storage()).unwrap();
            let moved = pair.origins().iter().filter(|r| r.hoist.is_some()).count();
            if mode == 2 {
                assert_eq!(moved, 0);
            } else {
                assert!(moved > 0);
            }
            for (before, after) in input
                .module()
                .functions
                .iter()
                .zip(&tail.output().module().functions)
            {
                for (a, b) in before
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .zip(&after.body.as_ref().unwrap().blocks)
                {
                    assert_eq!(a.terminator, b.terminator);
                }
            }
            drop(pair);
            budget.release_storage(ps.retained_storage()).unwrap();
            settle(tail, budget);
        });
    }
}

#[test]
fn owned_v18_licm_preparation_and_replay_have_exact_and_one_short_resource_limits() {
    let (input, retained) = admit_v18(&storage_fixture());
    let floor = retained + 37;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = (|| {
            let tail = prepare_owned_licm_v18(&input, LAYOUTS, &mut budget)?;
            let credit = tail.retained_storage();
            if let Err(error) = budget.reserve_storage(credit) {
                drop(tail);
                return Err(Error::Resource(error));
            }
            let replay = tail.replay_against(&input, &mut budget);
            let outcome = match replay {
                Ok((pair, ps)) => {
                    let reservation = budget.reserve_storage(ps.retained_storage());
                    drop(pair);
                    if reservation.is_ok() {
                        budget.release_storage(ps.retained_storage()).unwrap();
                    }
                    reservation.map_err(Error::Resource)
                }
                Err(error) => Err(error),
            };
            drop(tail);
            budget.release_storage(credit).unwrap();
            outcome
        })();
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = run(WORK, STORAGE);
    result.unwrap();
    let (exact, exact_work, exact_peak) = run(work, storage);
    exact.unwrap();
    assert_eq!((exact_work, exact_peak), (work, storage));
    let Resource::Work(denied) = resource(run(work - 1, storage).0.unwrap_err()) else {
        panic!("one-short work must retain a typed work refusal");
    };
    assert_eq!(denied.limit(), work - 1);
    assert!(denied.actual() > denied.limit());
    let Resource::Storage(denied) = resource(run(work, storage - 1).0.unwrap_err()) else {
        panic!("one-short storage must retain a typed storage refusal");
    };
    assert_eq!(denied.limit(), storage - 1);
    assert!(denied.actual() > denied.limit());
}

fn resource(error: Error) -> Resource {
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as Verification, KernelIrDecodeError as Decode,
        KernelIrEncodeError as Encode,
    };
    match error {
        Error::Resource(r)
        | Error::Inventory(InventoryError::Resource(r))
        | Error::Loops(LoopError::Resource(r))
        | Error::Pair(PairError::Resource(r))
        | Error::Pair(PairError::Inventory(InventoryError::Resource(r)))
        | Error::Pair(PairError::Loops(LoopError::Resource(r)))
        | Error::AdmissionV18(AdmissionError18::Resource(r))
        | Error::AdmissionV18(AdmissionError18::Layout(
            fe2o3_kernel_ir::StorageLayoutErrorV1::Resource(r),
        ))
        | Error::AdmissionV18(AdmissionError18::Verification(Verification::Resource(r)))
        | Error::AdmissionV18(AdmissionError18::Decode(Decode::Resource(r)))
        | Error::ControlFlow(FlowError::Resource(r))
        | Error::Pair(PairError::ControlFlow(FlowError::Resource(r))) => r,
        Error::AdmissionV18(AdmissionError18::Encode(Encode::WorkLimit(error)))
        | Error::AdmissionV18(AdmissionError18::Decode(Decode::WorkLimit(error)))
        | Error::AdmissionV18(AdmissionError18::Decode(Decode::Encode(Encode::WorkLimit(error)))) => {
            Resource::Work(error)
        }
        other => panic!("expected typed cumulative resource refusal, got {other:?}"),
    }
}
