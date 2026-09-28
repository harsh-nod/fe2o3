fn source_descriptor_replay_boundary_v1762(
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        work,
        storage,
        |original, _, budget| {
            slice_view_v1::check_original_descriptor_replay_facts_v1762(original, budget)
        },
    )
}

#[test]
fn original_descriptor_replay_preserves_actual_operation_and_assertion_coordinates() {
    source_descriptor_replay_boundary_v1762(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT)
        .0
        .unwrap();
}

#[test]
fn original_descriptor_replay_complete_transaction_exact_and_one_short() {
    let (result, work, storage) =
        source_descriptor_replay_boundary_v1762(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let exact = source_descriptor_replay_boundary_v1762(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = source_descriptor_replay_boundary_v1762(work_limit, storage_limit);
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(short.0.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert_eq!(error.actual(), work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(error.actual(), storage);
            }
            other => panic!("genuine original replay boundary: {other:?}"),
        }
    }
}

#[test]
fn original_whole_value_origin_queries_restore_the_exact_post_helper_caller_floor() {
    run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, _, budget| slice_view_v1::check_original_origin_floor_v1762(original, budget),
    )
    .0
    .unwrap();
}

#[test]
fn original_descriptor_replay_foreign_source_site_is_latched_before_visitation() {
    let visited = std::cell::Cell::new(false);
    let result = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, _, budget| {
            slice_view_v1::check_foreign_descriptor_site_v1762(original, budget, &visited)
        },
    )
    .0;
    assert!(!visited.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "slice source/root/forwarding association"
            ))
        ),
        "{result:?}"
    );
}
