//! Pure report and selector controls; no runtime, native or device-health authority.
#[path = "fixtures/conditional-custodian-application/src/native_shards_case.rs"]
mod native_shards_case;

#[test]
fn native_shard_partial_manifest_is_structured_and_does_not_assert_all_success() {
    use native_shards_case::{Case, Shard, State};
    let case = Case {
        producer_source: "source.rs".into(),
        devices: vec![7, 8],
    };
    let observed = [(7, 128), (8, 129)];
    let shards = [
        Shard {
            uid: 7,
            render_minor: 128,
            node: 1,
            elements: 32,
            state: State::DeviceUnavailableBeforeActivation,
            checked_values: 0,
        },
        Shard {
            uid: 8,
            render_minor: 129,
            node: 2,
            elements: 33,
            state: State::Succeeded,
            checked_values: 33,
        },
    ];
    let fields = case.report(&observed, &shards, true).unwrap();
    let value: serde_json::Value = serde_json::from_str(&format!("{{{fields}}}")).unwrap();
    assert_eq!(value["coverage"], "all-admitted-context");
    assert_eq!(value["admitted_devices"], 2);
    assert_eq!(value["successful_shards"], 1);
    assert_eq!(value["failed_shards"], 1);
    assert_eq!(value["settled_local_failure"], true);
    assert_eq!(value["shards"][0]["checked_values"], 0);
    assert_eq!(value["shards"][1]["checked_values"], 33);
    assert_eq!(value["direct_native_data_transfer"], false);
    assert_eq!(value["copied_bytes"], 0);
}
