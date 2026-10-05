fn with_aggregate_occurrence_pair_v30(
    before: &Module,
    after: &Module,
    run: impl FnOnce(
        &CheckedCanonicalKirAggregateSsaV18<'_>,
        &Inventory<'_>,
        &Inventory<'_>,
        &mut Budget<'_>,
    ),
) {
    let (input, ib) = admit(before);
    let (output, ob) = admit(after);
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    let floor = ib + ob + 47;
    budget.reserve_storage(floor).unwrap();
    let witness = derive_canonical_kir_aggregate_ssa_v18(&input, &mut budget).unwrap();
    let wb = witness.retained_storage();
    budget.reserve_storage(wb).unwrap();
    let (checked, cb) =
        check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget).unwrap();
    budget.reserve_storage(cb.retained_storage()).unwrap();
    let (a, ab) = Inventory::derive_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(ab.retained_storage()).unwrap();
    let (b, bb) = Inventory::derive_v18(&output, &mut budget).unwrap();
    budget.reserve_storage(bb.retained_storage()).unwrap();
    let retained = budget.storage();
    run(&checked, &a, &b, &mut budget);
    assert_eq!(budget.storage(), retained);
    drop(b);
    drop(a);
    drop(checked);
    drop(witness);
    budget
        .release_storage(wb + cb.retained_storage() + ab.retained_storage() + bb.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn aggregate_occurrences_bind_actual_copy_removed_pointer_and_parameter() {
    with_aggregate_occurrence_pair_v30(
        &fixture(),
        &expected(),
        |checked, input, output, budget| {
            let floor = budget.storage();
            let (index, receipt) =
                CheckedCanonicalKirAggregateOccurrencesV30::derive(checked, input, output, budget)
                    .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(std::ptr::eq(index.checked(), checked));
            assert!(std::ptr::eq(index.input(), input));
            assert!(std::ptr::eq(index.output(), output));
            assert!(!index.grants_authority());
            assert_eq!(index.operation(0, budget).unwrap(), None);
            assert_eq!(index.operation(1, budget).unwrap(), None);
            let copy = index.operation(2, budget).unwrap().unwrap();
            assert_eq!(copy.output(), output.operations()[1].coordinate);
            assert!(!copy.is_retained());
            assert_eq!(
                index.definition(0, budget).unwrap(),
                Some(output.definitions()[0].coordinate)
            );
            assert_eq!(index.definition(1, budget).unwrap(), None);
            assert_eq!(
                index.definition(2, budget).unwrap(),
                Some(output.definitions()[2].coordinate)
            );
            drop(index);
            budget.release_storage(receipt.retained_storage()).unwrap();
        },
    );
}

fn aggregate_occurrence_parallel_fixture_v30() -> (Module, Module) {
    let mut before = fixture();
    let mut after = expected();
    for module in [&mut before, &mut after] {
        let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
        blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(30), Type::BOOL),
            Kind::Constant(Constant::Bool(false)),
        ));
        blocks[0].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(30),
            then_target: BlockId(200),
            then_arguments: vec![ValueId(20)],
            else_target: BlockId(200),
            else_arguments: vec![ValueId(20)],
        });
        let mut merge = BasicBlock::new(BlockId(200));
        merge
            .parameters
            .push(ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U32)));
        merge.terminator = Some(Terminator::Return {
            values: vec![ValueId(40)],
        });
        blocks.push(merge);
    }
    let entry = &mut after.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations[0].results[0].id = ValueId(41);
    let Kind::Select { condition, .. } = &mut entry.operations[1].kind else {
        unreachable!()
    };
    *condition = ValueId(41);
    (before, after)
}

#[test]
fn aggregate_occurrences_preserve_retained_operations_and_parallel_edge_ordinals() {
    let (before, after) = aggregate_occurrence_parallel_fixture_v30();
    with_aggregate_occurrence_pair_v30(&before, &after, |checked, input, output, budget| {
        let (index, receipt) =
            CheckedCanonicalKirAggregateOccurrencesV30::derive(checked, input, output, budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let retained = index.operation(3, budget).unwrap().unwrap();
        assert!(retained.is_retained());
        assert_eq!(retained.output(), output.operations()[2].coordinate);
        assert_eq!(input.edges().len(), 2);
        assert_eq!(input.edges()[0].target, input.edges()[1].target);
        assert_ne!(
            index.edge(0, budget).unwrap(),
            index.edge(1, budget).unwrap()
        );
        for ordinal in 0..2 {
            assert_eq!(
                index.edge(ordinal, budget).unwrap(),
                input.edges()[ordinal].coordinate
            );
        }
        drop(index);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn aggregate_occurrences_refuse_equal_byte_foreign_endpoint_inventories() {
    with_aggregate_occurrence_pair_v30(
        &fixture(),
        &expected(),
        |checked, input, output, budget| {
            for original in [true, false] {
                let (foreign, owner_bytes) = admit(if original {
                    input.owner().module()
                } else {
                    output.owner().module()
                });
                budget.reserve_storage(owner_bytes).unwrap();
                let (inventory, credit) = Inventory::derive_v18(&foreign, budget).unwrap();
                budget.reserve_storage(credit.retained_storage()).unwrap();
                let floor = budget.storage();
                let result = if original {
                    CheckedCanonicalKirAggregateOccurrencesV30::derive(
                        checked, &inventory, output, budget,
                    )
                } else {
                    CheckedCanonicalKirAggregateOccurrencesV30::derive(
                        checked, input, &inventory, budget,
                    )
                };
                assert!(matches!(
                    result,
                    Err(Error::Inconsistent(
                        "aggregate occurrence exact pair inventories"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                drop(inventory);
                drop(foreign);
                budget
                    .release_storage(owner_bytes + credit.retained_storage())
                    .unwrap();
            }
            let (index, receipt) =
                CheckedCanonicalKirAggregateOccurrencesV30::derive(checked, input, output, budget)
                    .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(std::ptr::eq(index.checked(), checked));
            drop(index);
            budget.release_storage(receipt.retained_storage()).unwrap();
        },
    );
}

#[test]
fn aggregate_occurrences_refuse_missing_dense_rows_without_partial_transport() {
    with_aggregate_occurrence_pair_v30(
        &fixture(),
        &expected(),
        |checked, input, output, budget| {
            let (index, receipt) =
                CheckedCanonicalKirAggregateOccurrencesV30::derive(checked, input, output, budget)
                    .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                index.operation(input.operations().len(), budget),
                Err(Error::Inconsistent(
                    "aggregate occurrence operation ordinal"
                ))
            ));
            assert!(matches!(
                index.definition(input.definitions().len(), budget),
                Err(Error::Inconsistent(
                    "aggregate occurrence definition ordinal"
                ))
            ));
            assert!(matches!(
                index.edge(input.edges().len(), budget),
                Err(Error::Inconsistent("aggregate occurrence edge ordinal"))
            ));
            assert!(index.operation(2, budget).unwrap().is_some());
            drop(index);
            budget.release_storage(receipt.retained_storage()).unwrap();
        },
    );
}

#[test]
fn aggregate_occurrences_exact_and_one_short_index_resources_restore_floor() {
    with_aggregate_occurrence_pair_v30(&fixture(), &expected(), |checked, input, output, outer| {
        let floor = outer.storage();
        let mut work = Work::new(20_000_000);
        let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
        budget.reserve_storage(floor).unwrap();
        let (index, _) =
            CheckedCanonicalKirAggregateOccurrencesV30::derive(checked, input, output, &mut budget)
                .unwrap();
        let used = budget.work();
        let peak = budget.peak_storage();
        drop(index);
        assert_eq!(budget.storage(), floor);
        for (work_limit, storage_limit, expected) in
            [(used, peak, 0), (used - 1, peak, 1), (used, peak - 1, 2)]
        {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = CheckedCanonicalKirAggregateOccurrencesV30::derive(
                checked,
                input,
                output,
                &mut budget,
            );
            match (expected, result) {
                (0, Ok((index, _))) => drop(index),
                (
                    1,
                    Err(
                        Error::Resource(Resource::Work(_))
                        | Error::Inventory(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_))),
                    ),
                ) => {}
                (2, Err(Error::Resource(Resource::Storage(_)))) => {}
                _ => panic!("exact checked aggregate occurrence-index resource boundary"),
            }
            assert_eq!(budget.storage(), floor);
        }
    });
}
