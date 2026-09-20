#[test]
fn correspondence_bucket_ordering_resolves_each_large_record_once_and_stably() {
    const FUNCTION_COUNT: usize = 257;
    const RECORD_COUNT: usize = 256 * 1024;
    #[derive(Debug, Eq, PartialEq)]
    struct InstrumentedRecordV1 {
        function: SemanticFunctionIdV1,
        sequence: usize,
    }

    let owner = SemanticFunctionIdV1::from_index(0);
    let function_ordinals = (0..FUNCTION_COUNT)
        .map(|ordinal| {
            (
                (
                    owner,
                    SemanticFunctionIdV1::from_index(u32::try_from(ordinal).unwrap()),
                ),
                ordinal,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let records = (0..RECORD_COUNT)
        .map(|record| InstrumentedRecordV1 {
            function: SemanticFunctionIdV1::from_index(
                u32::try_from(record % FUNCTION_COUNT).unwrap(),
            ),
            sequence: record / FUNCTION_COUNT,
        })
        .collect::<Vec<_>>();
    let key_resolutions = std::cell::Cell::new(0_usize);
    let ordered = order_correspondence_records_v1(
        records,
        &function_ordinals,
        |record: &InstrumentedRecordV1| {
            key_resolutions.set(key_resolutions.get() + 1);
            (owner, record.function)
        },
    )
    .unwrap();
    assert_eq!(key_resolutions.get(), RECORD_COUNT);
    assert_eq!(ordered.len(), RECORD_COUNT);
    for window in ordered.windows(2) {
        assert!(
            window[0].function < window[1].function
                || (window[0].function == window[1].function
                    && window[0].sequence < window[1].sequence),
            "function buckets or stable per-function record order changed",
        );
    }
}
