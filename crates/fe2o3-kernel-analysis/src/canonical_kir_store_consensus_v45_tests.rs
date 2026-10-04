use super::*;

fn diamond() -> Module {
    let mut module = fixture();
    blocks(&mut module)[0].operations.pop();
    blocks(&mut module)[1].operations.push(store(100, 0));
    blocks(&mut module)[2].operations.push(store(100, 0));
    module
}

#[test]
fn store_consensus_pair_derives_both_branches_and_irreducible_cycles_independently() {
    let mut module = diamond();
    accept(&module, true);
    blocks(&mut module)[1].terminator = Some(branch(30, 40));
    blocks(&mut module)[2].terminator = Some(branch(20, 40));
    accept(&module, true);
    let (output, mut rows) = rewritten(&module, true);
    rows.last_mut().unwrap().store = Some(site(2, 0));
    assert!(
        run(&admit(&module), &admit(&output), &rows, WORK, STORAGE)
            .0
            .is_ok(),
        "either exact class representative is inert; all paths are independently checked"
    );
    blocks(&mut module)[1].operations.clear();
    blocks(&mut module)[2].operations.clear();
    accept(&module, false);
    let (output, mut rows) = rewritten(&module, true);
    rows.last_mut().unwrap().store = Some(site(0, 0));
    assert!(
        run(&admit(&module), &admit(&output), &rows, WORK, STORAGE)
            .0
            .is_err()
    );
}

#[test]
fn store_consensus_pair_refuses_one_missing_unequal_clobbered_or_retyped_arm() {
    for fault in 0..6 {
        let mut input = diamond();
        match fault {
            0 => blocks(&mut input)[2].operations.clear(),
            1 => {
                blocks(&mut input)[0]
                    .operations
                    .push(Operation::effect_free(
                        ValueDef::new(ValueId(2), ty()),
                        Kind::Unary {
                            op: UnaryOp::Not,
                            operand: ValueId(0),
                        },
                    ));
                blocks(&mut input)[2].operations[0] = store(100, 2);
            }
            2 => blocks(&mut input)[2].operations.push(allocation(200)),
            3 => {
                let Kind::Store { access, .. } = &mut blocks(&mut input)[2].operations[0].kind
                else {
                    unreachable!()
                };
                access.alignment = 2;
            }
            4 => {
                let Kind::Store { access, .. } = &mut blocks(&mut input)[2].operations[0].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            5 => {
                blocks(&mut input)[0].operations.push(allocation(200));
                blocks(&mut input)[2].operations[0] = store(200, 0);
            }
            _ => unreachable!(),
        }
        let (output, rows) = rewritten(&input, true);
        assert!(
            run(&admit(&input), &admit(&output), &rows, WORK, STORAGE)
                .0
                .is_err(),
            "fault={fault}"
        );
        accept(&input, false);
    }
}

#[test]
fn store_consensus_v18_pair_preserves_layouts_and_rejects_changed_actual_payloads() {
    use fe2o3_kernel_ir::{StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1};
    let layouts = StorageLayoutLimitsV1 {
        rows: 1,
        edges: 0,
        containment_depth: 1,
        object_bytes: 8,
    };
    let mut input = diamond();
    input.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    let (output, rows) = rewritten(&input, true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (a, sa) =
        Owner18::from_module_ref_with_verification_budget_v18(&input, layouts, &mut budget)
            .unwrap();
    budget.reserve_storage(sa.retained_storage()).unwrap();
    let (b, sb) =
        Owner18::from_module_ref_with_verification_budget_v18(&output, layouts, &mut budget)
            .unwrap();
    budget.reserve_storage(sb.retained_storage()).unwrap();
    let floor = budget.storage();
    let (pair, size) = check_canonical_kir_cross_block_forwarding_v18(
        &a,
        &b,
        &rows,
        Limits::default(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(size.retained_storage()).unwrap();
    assert!(std::ptr::eq(pair.input(), &a));
    assert!(std::ptr::eq(pair.output(), &b));
    assert!(!pair.grants_authority());
    drop(pair);
    budget.release_storage(size.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    let mut changed = output;
    changed.storage_layouts[0] = StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    };
    let (c, sc) =
        Owner18::from_module_ref_with_verification_budget_v18(&changed, layouts, &mut budget)
            .unwrap();
    budget.reserve_storage(sc.retained_storage()).unwrap();
    assert!(matches!(
        check_canonical_kir_cross_block_forwarding_v18(
            &a,
            &c,
            &rows,
            Limits::default(),
            &mut budget
        ),
        Err(Error::Mismatch("module payload"))
    ));
}
