#[test]
fn issued_helper_transport_roles_cover_exact_casts_in_each_actual_root() {
    for nested in [false, true] {
        let owner = issued_helper_two_root_owner_v26(nested);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let mut roots = 0;
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                roots = slice_view_v1::test_issued_pointer_transport_roles_v26(
                    original, optimized, 0, budget,
                )?;
                Ok(())
            },
        )
        .0;
        assert!(result.is_ok(), "nested={nested}: {result:?}");
        assert_eq!(roots, 2);
    }
}

#[test]
fn issued_helper_transport_roles_reject_incomplete_duplicate_and_noncast_censuses() {
    for fault in 1..=8 {
        let owner = issued_helper_two_root_owner_v26(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let mut completed = false;
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let error = slice_view_v1::test_issued_pointer_transport_roles_v26(
                    original, optimized, fault, budget,
                )
                .unwrap_err();
                let expected = match fault {
                    1 => "issued final transport cast lacks an exact original occurrence",
                    2 | 5 => "issued retained cast absent from final exact transport paths",
                    3 => "issued output transport census is not unique and ordered",
                    4 => "issued transport changed cast opcode",
                    6 => "issued transport selected definition census is incomplete",
                    7 => "issued transport selection changed checked disposition",
                    8 => "issued transport changed function or original occurrence",
                    _ => unreachable!(),
                };
                assert!(
                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding(message) if message == expected),
                    "fault={fault}: {error:?}"
                );
                completed = true;
                Err(error)
            },
        )
        .0;
        assert!(result.is_err(), "fault={fault}");
        assert!(completed, "fault={fault}: {result:?}");
    }
}

#[test]
fn issued_helper_transport_roles_have_exact_and_one_short_transaction_limits() {
    let run = |work, storage| {
        let owner = issued_helper_two_root_owner_v26(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            work,
            storage,
            |original, optimized, budget| {
                assert_eq!(
                    slice_view_v1::test_issued_pointer_transport_roles_v26(
                        original, optimized, 0, budget
                    )?,
                    2
                );
                Ok(())
            },
        )
    };
    let full = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    full.0.unwrap();
    let exact = run(full.1, full.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (full.1, full.2));
    for (work, storage, is_work) in [(full.1 - 1, full.2, true), (full.1, full.2 - 1, false)] {
        let resource = source_slot_tests::original_repeated_source_resource_v29(
            run(work, storage).0.unwrap_err(),
        );
        match (is_work, resource) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work);
                assert!(error.actual() > work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage);
                assert!(error.actual() > storage);
            }
            other => panic!("issued transport transaction: {other:?}"),
        }
    }
}
