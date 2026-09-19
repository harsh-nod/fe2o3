use super::*;

fn identity(value: u8) -> SemanticLocalIdentityV1 {
    SemanticLocalIdentityV1::from_sha256([value; 32])
}

#[test]
fn many_occurrences_merge_with_raw_and_receiver_locals_canonically() {
    let raw = [identity(2), identity(8), identity(18)];
    let inserted = [
        identity(15),
        identity(1),
        identity(7),
        identity(5),
        identity(9),
    ];
    let order = LocalOrderV1::new(raw.into_iter(), &inserted, |_| Ok::<_, ()>(())).unwrap();
    for (index, value) in [1, 2, 5, 7, 8, 9, 15, 18].into_iter().enumerate() {
        assert_eq!(
            order
                .index(identity(value), |_| Ok::<_, ()>(()))
                .unwrap()
                .index() as usize,
            index
        );
    }
}

#[test]
fn construction_and_lookup_require_exact_prepaid_work() {
    let raw = [identity(2), identity(8), identity(18)];
    let inserted = [identity(1), identity(7)];
    let mut required = 0;
    LocalOrderV1::new(raw.into_iter(), &inserted, |amount| {
        required += amount;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert!(required > 0);
    for (limit, success) in [(required, true), (required - 1, false)] {
        let mut remaining = limit;
        let result = LocalOrderV1::new(raw.into_iter(), &inserted, |amount| {
            remaining = remaining.checked_sub(amount).ok_or("work")?;
            Ok(())
        });
        assert_eq!(result.is_ok(), success);
        if !success {
            assert!(matches!(result, Err(Error::Resource("work"))));
            assert_eq!(remaining, limit);
        }
    }
    let order = LocalOrderV1::new(raw.into_iter(), &inserted, |_| Ok::<_, ()>(())).unwrap();
    let mut lookup_work = 0;
    order
        .index(identity(7), |amount| {
            lookup_work += amount;
            Ok::<_, ()>(())
        })
        .unwrap();
    for limit in [lookup_work, lookup_work - 1] {
        let mut remaining = limit;
        let result = order.index(identity(7), |amount| {
            remaining = remaining.checked_sub(amount).ok_or("work")?;
            Ok(())
        });
        assert_eq!(result.is_ok(), limit == lookup_work);
        if limit < lookup_work {
            assert!(matches!(result, Err(Error::Resource("work"))));
        }
    }
}

#[test]
fn duplicate_and_missing_local_identities_are_not_accepted() {
    assert!(matches!(
        LocalOrderV1::new([identity(2)].into_iter(), &[identity(2)], |_| Ok::<_, ()>(
            ()
        )),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        LocalOrderV1::new(
            [identity(2), identity(2)].into_iter(),
            &[],
            |_| Ok::<_, ()>(())
        ),
        Err(Error::Invalid)
    ));
    let order = LocalOrderV1::new([identity(2)].into_iter(), &[], |_| Ok::<_, ()>(())).unwrap();
    assert!(matches!(
        order.index(identity(1), |_| Ok::<_, ()>(())),
        Err(Error::Invalid)
    ));
}

#[test]
fn synthetic_local_identity_binds_caller_block_callee_and_role() {
    let function = SemanticFunctionIdentityV1::from_sha256([1; 32]);
    let block = SemanticBlockIdentityV1::from_sha256([2; 32]);
    let callee = SemanticFunctionIdentityV1::from_sha256([3; 32]);
    let ids = identities_v1(function, block, callee);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids, identities_v1(callee, block, callee));
    assert_ne!(
        ids,
        identities_v1(
            function,
            SemanticBlockIdentityV1::from_sha256([4; 32]),
            callee
        )
    );
    assert_ne!(ids, identities_v1(function, block, function));
}
