#[path = "fixtures/conditional-custodian-application/src/native_shards_case.rs"]
mod native_shards_case;
#[path = "fixtures/conditional-custodian-application/src/native_staged_ring_case.rs"]
mod native_staged_ring_case;

#[test]
fn staged_ring_report_is_structured_and_never_claims_direct_data_or_overlap() {
    use native_staged_ring_case::{Compute, Row, Transfer, nodes};
    let case = native_shards_case::Case {
        producer_source: "/original/source".into(),
        devices: vec![7, 9],
    };
    let observed = [(7, 128), (9, 129)];
    let rows: Vec<_> = (0..2)
        .map(|index| Row {
            source: observed[index],
            destination: observed[(index + 1) % 2],
            nodes: nodes(index),
            compute: Compute::Succeeded,
            staging: Transfer::Succeeded,
            copy: Transfer::Succeeded,
            checked_values: case.elements(index),
            checked_copy_bytes: case.elements(index) * 4,
            staged_version_committed: true,
            copy_input_available: true,
            copy_version_committed: true,
        })
        .collect();
    let fields = native_staged_ring_case::report(&case, &observed, &rows, false).unwrap();
    let value: serde_json::Value = serde_json::from_str(&format!("{{{fields}}}")).unwrap();
    assert_eq!(value["admitted_devices"], 2);
    assert_eq!(value["ring"].as_array().unwrap().len(), 2);
    assert_eq!(value["checked_copy_bytes"], 260);
    assert_eq!(value["direct_native_data_transfer"], false);
    assert_eq!(value["xgmi_transfer"], false);
    assert_eq!(value["measured_compute_transfer_overlap"], false);
}
