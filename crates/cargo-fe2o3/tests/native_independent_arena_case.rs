#[path = "fixtures/conditional-custodian-application/src/native_independent_arena_case.rs"]
mod native_independent_arena_case;

#[test]
fn independent_arena_report_is_structured_and_never_promotes_size_to_duration_or_depth() {
    use native_independent_arena_case::{MEMBERS, Report, SOURCE_BYTES};
    let report = Report {
        events: 2048,
        publications: MEMBERS,
        completions: MEMBERS,
        pending: 0,
        maximum_unobserved: MEMBERS,
        pre_metadata_bytes: SOURCE_BYTES + 1_000_000,
        pre_metadata_records: 20,
        residual_metadata_bytes: 1_000_000,
        residual_metadata_records: 10,
        pre_result_bytes: 3 * SOURCE_BYTES,
        pre_result_records: 2 * MEMBERS,
        residual_source_bytes: SOURCE_BYTES,
        residual_source_records: MEMBERS,
        witness: None,
    };
    let value: serde_json::Value =
        serde_json::from_str(&format!("{{{}}}", report.fields(0xabcd).unwrap())).unwrap();
    assert_eq!(value["device"], "0x000000000000abcd");
    assert_eq!(value["target"], "gfx942:xnack-");
    assert_eq!(value["counts"]["even"], 65536);
    assert_eq!(value["counts"]["odd"], 64);
    assert_eq!(value["source_bytes"], SOURCE_BYTES);
    assert_eq!(value["prelaunch_result_bytes"], 3 * SOURCE_BYTES);
    assert_eq!(value["residual_source_bytes"], SOURCE_BYTES);
    assert_eq!(
        value["maximum_published_without_observed_completion"],
        MEMBERS
    );
    assert!(value["out_of_order_witness"].is_null());
    for key in [
        "native_out_of_order_measured",
        "native_durations_measured",
        "thousands_inflight_qualified",
        "rolling_rearm",
    ] {
        assert_eq!(value[key], false, "{key}");
    }
    assert_eq!(value["results_dropped_before_common_destroy"], MEMBERS);
    assert_eq!(value["common_arena_destroyed"], true);
    assert_eq!(value["metadata_credits"], "refunded");
    assert_eq!(
        value["qualification_status"],
        "unqualified-no-receipt-order-witness"
    );
    assert!(
        !value
            .as_object()
            .unwrap()
            .contains_key("physical_overlap_measured")
    );
}
#[path = "fixtures/conditional-custodian-application/src/native_registry4_case.rs"]
mod native_registry4_case;
