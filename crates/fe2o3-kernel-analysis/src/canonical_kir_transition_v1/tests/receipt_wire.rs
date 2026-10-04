use super::*;
use fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1 as Receipt;

pub(super) fn roundtrip(
    a: &Inventory<'_>,
    b: &Inventory<'_>,
    rows: &Rows,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let (encoded, encoded_storage) =
        Receipt::from_candidate_with_budget(&a.identity(), &b.identity(), rows.candidate(), budget)
            .unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(encoded_storage.retained_storage())
        .unwrap();
    let encoded_floor = budget.storage();
    let (decoded, decoded_storage) =
        Receipt::decode_with_budget(encoded.canonical_bytes(), budget).unwrap();
    assert_eq!(budget.storage(), encoded_floor);
    budget
        .reserve_storage(decoded_storage.retained_storage())
        .unwrap();
    let checked_floor = budget.storage();
    let retained = {
        let (checked, checked_storage) =
            check_canonical_kir_transition_receipt_v1(a, b, &decoded, budget).unwrap();
        assert_eq!(budget.storage(), checked_floor);
        budget
            .reserve_storage(checked_storage.retained_storage())
            .unwrap();
        assert!(std::ptr::eq(checked.input(), a));
        assert!(std::ptr::eq(checked.output(), b));
        assert!(std::ptr::eq(
            checked.rows().operations,
            decoded.candidate().operations
        ));
        assert_eq!(checked.checker_policy(), 1);
        assert!(!checked.grants_authority());
        checked_storage.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    drop(decoded);
    budget
        .release_storage(decoded_storage.retained_storage())
        .unwrap();
    drop(encoded);
    budget
        .release_storage(encoded_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

fn repeated() -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(1)],
        else_target: BlockId(20),
        else_arguments: vec![ValueId(2)],
    });
    let mut exit = returning(20, vec![], &[3]);
    exit.parameters = vec![ValueDef::new(ValueId(3), U32)];
    module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![0, 1, 2],
        vec![entry, exit],
    )
}

#[test]
fn receipt_admission_rejects_repeated_edge_origin_swaps_after_valid_wire_decode() {
    inspect(
        repeated(),
        repeated(),
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert_eq!(a.edges().len(), 2);
            assert_eq!(a.edges()[0].target, a.edges()[1].target);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.reserve_storage(floor).unwrap();
            roundtrip(a, b, rows, &mut budget);
            let first = rows.edges[0].input;
            rows.edges[0].input = rows.edges[1].input;
            rows.edges[1].input = first;
            let (encoded, es) = Receipt::from_candidate_with_budget(
                &a.identity(),
                &b.identity(),
                rows.candidate(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(es.retained_storage()).unwrap();
            let (decoded, ds) =
                Receipt::decode_with_budget(encoded.canonical_bytes(), &mut budget).unwrap();
            budget.reserve_storage(ds.retained_storage()).unwrap();
            let before = budget.storage();
            assert!(matches!(
                check_canonical_kir_transition_receipt_v1(a, b, &decoded, &mut budget),
                Err(Error::Rule(_))
            ));
            assert_eq!(budget.storage(), before);
        },
    );
}

#[test]
fn receipt_endpoint_comparison_exact_one_under_preserves_borrowed_owner_floor() {
    inspect(
        Module::new("input"),
        Module::new("output"),
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (receipt, storage) = Receipt::from_candidate_with_budget(
                &b.identity(),
                &a.identity(),
                rows.candidate(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            for exact in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + 80 - usize::from(!exact));
                let mut budget = Budget::new(&mut work, floor);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(floor).unwrap();
                let error = check_canonical_kir_transition_receipt_v1(a, b, &receipt, &mut budget)
                    .unwrap_err();
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor);
                assert_eq!(budget.failed_storage(), None);
                if exact {
                    assert_eq!(error, Error::Rule("transition receipt endpoint or policy"));
                    assert_eq!(budget.work(), 87);
                    assert_eq!(work.failed_work(), None);
                } else {
                    assert!(matches!(error, Error::Resource(Resource::Work(_))));
                    assert_eq!(budget.work(), 7);
                    assert_eq!(work.failed_work(), Some(87));
                }
            }
        },
    );
}

#[test]
fn empty_receipt_checker_exact_work_and_storage_extend_the_fixed_direct_checker() {
    inspect(
        Module::new("x"),
        Module::new("x"),
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (receipt, storage) = Receipt::from_candidate_with_budget(
                &a.identity(),
                &b.identity(),
                rows.candidate(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            let view = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
            let peak = floor + view + size_of::<State<'_, '_, '_, '_>>();
            // Direct empty "x" checker is7: entry/state2, metadata1+2+1,
            // fixed-point visit1. The receipt comparison adds exactly80.
            for exact in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(17 + 87 - usize::from(!exact));
                let mut budget = Budget::new(&mut work, peak);
                budget.charge_work(17).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = check_canonical_kir_transition_receipt_v1(a, b, &receipt, &mut budget);
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
                if exact {
                    let (checked, _) = result.unwrap();
                    assert!(std::ptr::eq(checked.input(), a));
                    assert!(std::ptr::eq(checked.output(), b));
                    assert_eq!(budget.work(), 104);
                    assert_eq!(work.failed_work(), None);
                } else {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(budget.work(), 103);
                    assert_eq!(work.failed_work(), Some(104));
                }
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, peak - 1);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                check_canonical_kir_transition_receipt_v1(a, b, &receipt, &mut budget),
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + view);
            assert_eq!(budget.failed_storage(), Some(peak));
            assert_eq!(budget.work(), 17 + 80 + 2);
            assert_eq!(work.failed_work(), None);
        },
    );
}
