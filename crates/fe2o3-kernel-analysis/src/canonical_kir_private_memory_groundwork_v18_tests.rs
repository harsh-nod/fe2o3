//! Extraction groundwork only: exercise the existing physical checker on real
//! verified V12 inventories. No V18 memory constructor or admission is added.
use super::*;

fn assert_exact_latest(input: &Module, cells: usize, expected: &[Option<usize>]) {
    with_inventory(input, |inventory, floor| {
        let run = |work_limit, storage_limit| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: cells },
                &mut budget,
            );
            assert_eq!(budget.storage(), floor);
            let result = result.map(|(proof, receipt)| {
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(proof.is_for(inventory));
                assert!(!proof.grants_authority());
                assert_eq!(proof.latest_stores(), expected);
                drop(proof);
                assert!(receipt.retained_storage() > 0);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, peak) = run(WORK, STORAGE);
        result.unwrap();
        let exact = run(work, peak);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, peak));
        for (work_limit, storage_limit, is_work) in
            [(work - 1, peak, true), (work, peak - 1, false)]
        {
            let denied = run(work_limit, storage_limit).0.unwrap_err();
            match (is_work, denied) {
                (true, Error::Resource(Resource::Work(error))) => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > work_limit);
                }
                (false, Error::Resource(Resource::Storage(error))) => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > storage_limit);
                }
                other => panic!("exact physical checker boundary: {other:?}"),
            }
        }
    });
}

#[test]
fn native_groundwork_same_store_intersection_survives_diamond_and_loop() {
    let diamond = module(vec![
        block(
            100,
            vec![allocation(), constant(), store()],
            conditional(200, 300),
        ),
        block(200, vec![], branch(400)),
        block(300, vec![], branch(400)),
        block(400, vec![load()], ret()),
    ]);
    assert_exact_latest(&diamond, 1, &[None, None, None, Some(2)]);
    let stable_loop = module(vec![
        block(100, vec![allocation(), constant(), store()], branch(200)),
        block(200, vec![], conditional(300, 400)),
        block(300, vec![], branch(200)),
        block(400, vec![load()], ret()),
    ]);
    assert_exact_latest(&stable_loop, 1, &[None, None, None, Some(2)]);
    let mut changed_loop = stable_loop;
    changed_loop.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .push(store());
    // Both stores write ValueId(20), but their operation identities differ.
    both_refuse(&changed_loop, "Load requires one exact reaching Store");
}

#[test]
fn native_groundwork_loop_allocation_resets_before_every_reaching_read() {
    let reinitialized = module(vec![
        block(100, vec![constant()], branch(200)),
        block(200, vec![allocation(), store()], branch(300)),
        block(300, vec![load()], conditional(200, 400)),
        block(400, vec![], ret()),
    ]);
    assert_exact_latest(&reinitialized, 1, &[None, None, None, Some(2)]);
    let stale_iteration = module(vec![
        block(100, vec![constant()], branch(200)),
        block(200, vec![allocation()], conditional(300, 400)),
        block(300, vec![store()], branch(200)),
        block(400, vec![load()], ret()),
    ]);
    both_refuse(&stale_iteration, "Load requires one exact reaching Store");
}

fn constant_index(id: u32, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Constant(Constant::Index(value)),
    )
}

fn element_address(id: u32, offset: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::GetElementPointer {
            base: ValueId(10),
            offset: ValueId(offset),
        },
    )
}

#[test]
fn native_groundwork_distinct_pointer_ids_join_only_the_exact_physical_cell() {
    let mut alloca = allocation();
    let OperationKind::Alloca { count, .. } = &mut alloca.kind else {
        unreachable!()
    };
    *count = Some(ValueId(21));
    let mut write = store();
    let OperationKind::Store { pointer, .. } = &mut write.kind else {
        unreachable!()
    };
    *pointer = ValueId(11);
    let mut read = load();
    let OperationKind::Load { pointer, .. } = &mut read.kind else {
        unreachable!()
    };
    *pointer = ValueId(12);
    let mut input = module(vec![
        block(
            100,
            vec![
                constant_index(21, 2),
                alloca,
                constant(),
                constant_index(30, 0),
                constant_index(31, 0),
                element_address(11, 30),
                element_address(12, 31),
                write,
            ],
            branch(200),
        ),
        block(200, vec![read], ret()),
    ]);
    assert_exact_latest(
        &input,
        2,
        &[None, None, None, None, None, None, None, None, Some(7)],
    );
    input.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind =
        OperationKind::Constant(Constant::Index(1));
    both_refuse(&input, "Load requires one exact reaching Store");
}

#[test]
fn native_groundwork_nonprivate_allocation_is_not_a_private_cell() {
    let mut input = local();
    let operations = &mut input.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations[0].results[0].ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let OperationKind::Alloca { address_space, .. } = &mut operations[0].kind else {
        unreachable!()
    };
    *address_space = AddressSpace::Workgroup;
    let OperationKind::Store { access, .. } = &mut operations[2].kind else {
        unreachable!()
    };
    access.address_space = AddressSpace::Workgroup;
    let OperationKind::Load { access, .. } = &mut operations[3].kind else {
        unreachable!()
    };
    access.address_space = AddressSpace::Workgroup;
    // This is a well-typed canonical graph, not a mislabeled private pointer.
    both_refuse(&input, "one exact scalar private allocation");
}

#[test]
fn native_groundwork_transfer_cut_is_before_reset_and_never_publishes_a_read() {
    let events = [
        Event::Reset {
            start: 9,
            length: 2,
        },
        Event::Write(9),
        Event::Read(9),
    ];
    // Three fixed operation debits plus the exact two-cell reset payload.
    const EXACT: usize = 3 * 3 + 2;
    for limit in [EXACT, EXACT - 1, 4] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        let mut state = [Some(70), Some(71)];
        let mut reads = Vec::new();
        let result = transfer(
            &events,
            0..events.len(),
            9,
            &mut state,
            &mut budget,
            |ordinal, store, _| {
                reads.push((ordinal, store));
                Ok(())
            },
        );
        if limit == EXACT {
            result.unwrap();
            assert_eq!(reads, vec![(2, Some(1))]);
            assert_eq!(state, [Some(1), None]);
            assert_eq!(budget.work(), EXACT);
        } else {
            let Error::Resource(Resource::Work(error)) = result.unwrap_err() else {
                panic!("exact transfer work refusal")
            };
            assert_eq!(error.limit(), limit);
            assert!(error.actual() > limit);
            assert!(reads.is_empty());
            if limit == 4 {
                assert_eq!(state, [Some(70), Some(71)], "reset debit precedes mutation");
                assert_eq!(budget.work(), 3);
            } else {
                assert_eq!(state, [Some(1), None]);
                assert_eq!(budget.work(), 8);
            }
        }
        assert_eq!(budget.storage(), 0, "caller owns event/state scratch");
    }
}
