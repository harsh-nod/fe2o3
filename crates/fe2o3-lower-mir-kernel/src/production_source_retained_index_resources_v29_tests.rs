#[test]
fn retained_source_index_actual_final_scope_exact_and_one_short_resources() {
    for case in [
        RetainedIndexCaseV29::Constant,
        RetainedIndexCaseV29::GuardLoop,
    ] {
        let (result, work, storage, completed) =
            retained_index_run_v29(case, true, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        let (result, _, _, completed) = retained_index_run_v29(case, true, work, storage);
        result.unwrap();
        assert!(completed);
        for (work_limit, storage_limit, work_short) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let (result, _, _, _) = retained_index_run_v29(case, true, work_limit, storage_limit);
            // This assertion runs outside every framework panic catcher. An inner
            // assertion panic cannot masquerade as a resource-limit success.
            let error = match result {
                Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                )) => error,
                other => panic!("expected exact original work/storage refusal: {other:?}"),
            };
            if work_short {
                assert!(matches!(error, ArgumentResourceV1::Work(_)));
            } else {
                assert!(matches!(error, ArgumentResourceV1::Storage(_)));
            }
        }
    }
}
#[test]
fn retained_source_index_headers_use_independent_owned_envelopes() {
    use std::mem::size_of;
    assert_eq!(
        source_index_result_header_v29::<PendingSourceIndexV29>().unwrap(),
        size_of::<Vec<PendingSourceIndexV29>>()
            + size_of::<Result<Vec<PendingSourceIndexV29>, ProductionSemanticKirErrorV1>>()
    );
    assert_eq!(
        source_index_result_header_v29::<PendingSourceIndexGuardV29>().unwrap(),
        size_of::<Vec<PendingSourceIndexGuardV29>>()
            + size_of::<Result<Vec<PendingSourceIndexGuardV29>, ProductionSemanticKirErrorV1>>()
    );
    assert_eq!(
        source_index_result_header_v29::<SourceIndexFailureV29>().unwrap(),
        size_of::<Vec<SourceIndexFailureV29>>()
            + size_of::<Result<Vec<SourceIndexFailureV29>, ProductionSemanticKirErrorV1>>()
    );
    assert_eq!(
        scoped_raw_admission_v29::immutable_index_header_v29().unwrap(),
        size_of::<Vec<(usize, usize, usize)>>()
            + size_of::<(
                Vec<SourceIndexLocationV29>,
                Vec<SourceIndexGuardLocationV29>
            )>()
            + size_of::<
                Result<
                    (
                        Vec<SourceIndexLocationV29>,
                        Vec<SourceIndexGuardLocationV29>
                    ),
                    ProductionSourceOwnedViewErrorV18,
                >,
            >()
    );
}
