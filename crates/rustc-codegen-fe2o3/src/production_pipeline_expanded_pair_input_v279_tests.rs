use super::*;

#[test]
fn expanded_pair_equal_bytes_do_not_substitute_owner_custody() {
    let first = Box::new([7u8; 32]);
    let second = Box::new([7u8; 32]);
    assert_eq!(first, second);
    same_owner(first.as_ref(), first.as_ref()).unwrap();
    assert!(matches!(
        same_owner(first.as_ref(), second.as_ref()),
        Err(Error::Unsupported(
            "expanded pair input owner or runtime differs"
        ))
    ));
}

#[test]
fn expanded_pair_retained_storage_includes_inline_owner_and_complete_rows() {
    for count in [0, 1, 2, 19] {
        assert_eq!(
            retained_bytes(count).unwrap(),
            size_of::<Input<'_, '_, '_, '_>>()
                + query_headers()
                + count * size_of::<RootInputV279>()
        );
    }
    assert!(matches!(
        retained_bytes(usize::MAX),
        Err(Resource::Arithmetic)
    ));
    assert!(headers().unwrap() > retained_bytes(0).unwrap());
}

#[test]
fn expanded_pair_exact_and_one_short_storage_keep_outside_floor() {
    let retained = retained_bytes(2).unwrap();
    for short in [false, true] {
        let mut work = Work::new(8);
        let mut budget = Budget::new(&mut work, 17 + retained - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let accepted = budget.reserve_storage(retained);
        if short {
            assert!(matches!(accepted, Err(Resource::Storage(_))));
            assert_eq!(budget.storage(), 17);
        } else {
            accepted.unwrap();
            let slot = std::ptr::from_ref(&budget) as usize;
            check_account(
                slot,
                budget.work_ledger_identity_v1(),
                17 + retained,
                &budget,
            )
            .unwrap();
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 17);
        }
    }
}

#[test]
fn expanded_pair_account_rejects_slot_ledger_and_refunded_owner() {
    let mut work = Work::new(8);
    let mut foreign_work = Work::new(8);
    let mut budget = Budget::new(&mut work, 64);
    let foreign = Budget::new(&mut foreign_work, 64);
    budget.reserve_storage(32).unwrap();
    let slot = std::ptr::from_ref(&budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        check_account(slot + 1, ledger, 32, &budget),
        Err(Resource::Accounting)
    ));
    assert!(matches!(
        check_account(slot, foreign.work_ledger_identity_v1(), 32, &budget),
        Err(Resource::Accounting)
    ));
    budget.release_storage(1).unwrap();
    assert!(matches!(
        check_account(slot, ledger, 32, &budget),
        Err(Resource::Accounting)
    ));
}

#[test]
fn expanded_pair_runtime_requires_exact_rank_and_every_extent() {
    let exact = ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [64, 1, 1],
    };
    assert_eq!(
        join_launch(Launch::PhysicalEnvelope(exact), 1, [64, 1, 1]).unwrap(),
        exact
    );
    for (rank, extents) in [
        (2, [64, 1, 1]),
        (1, [63, 1, 1]),
        (1, [64, 2, 1]),
        (1, [64, 1, 2]),
    ] {
        assert!(matches!(
            join_launch(Launch::PhysicalEnvelope(exact), rank, extents),
            Err(Error::Unsupported(
                "expanded pair input owner or runtime differs"
            ))
        ));
    }
}

#[test]
fn expanded_pair_hash_work_has_exact_and_one_short_boundary() {
    for limit in [10, 11] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let mut hash = Sha256::new();
        let result = append(&mut hash, &[1, 2, 3], &mut budget);
        assert_eq!(result.is_ok(), limit == 11);
        assert_eq!(budget.storage(), 17);
        if limit == 10 {
            assert!(budget.check_prior_denials_v1().is_err());
        }
    }
}
