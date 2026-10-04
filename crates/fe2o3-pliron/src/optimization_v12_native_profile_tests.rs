#[test]
fn native_profile_changes_only_the_structurally_bounded_observer_component() {
    // With B=31 and N=4: volume=32800, observer W=64*(16+4)=1280,
    // observer S=1536*4+4096=10240. All other components stay frozen.
    let (native, capture) = native_execution_resources_v1(31, 4).unwrap();
    assert_eq!(capture.node_limit(), 4);
    assert_eq!(native.work(), 27_368_704);
    assert_eq!(native.persistent_storage(), 276_736);
    assert_eq!(native.temporary_storage(), 528_896);
    let report =
        size_of::<PlironOptimizationReportV1>() + 7 * size_of::<PlironOptimizationPassReportV1>();
    assert_eq!(native.retained_storage(), report);
    let target = execution_resources_v12(31).unwrap();
    assert_eq!(target.work(), 28_391_552);
    assert_eq!(target.persistent_storage(), 464_128);
    assert_eq!(target.temporary_storage(), native.temporary_storage());
    assert_eq!(target.retained_storage(), report);

    // Identity bytes still pay the unchanged linear construction allowance;
    // they no longer act as the native observer's node estimate.
    let (padded, padded_capture) = native_execution_resources_v1(4_096, 4).unwrap();
    assert_eq!(padded_capture.node_limit(), 4);
    assert_eq!(padded.work(), 27_368_704 + 64 * (4_096 - 31));
    assert_eq!(padded.persistent_storage(), 276_736 + 8 * (4_096 - 31));
    assert_eq!(padded.temporary_storage(), 528_896 + 16 * (4_096 - 31));
    assert_eq!(
        execution_resources_v12(4_096).unwrap().work(),
        4_390_494_272
    );
}

#[test]
fn native_observer_growth_stays_within_the_frozen_replay_ceiling() {
    // B=37 gives frozen K=2*37+64=138. The native structural bound narrows
    // capacity; it cannot widen the legacy checker/observer compatibility cap.
    for (bound, expected) in [(1, 1), (4, 4), (138, 138), (262_144, 138)] {
        let (_, capture) = native_execution_resources_v1(37, bound).unwrap();
        assert_eq!(capture.node_limit(), expected);
        assert_eq!(
            capture.work().unwrap(),
            64 * (expected * expected + expected)
        );
        assert_eq!(capture.storage().unwrap(), 1536 * expected + 4096);
    }
    assert!(matches!(
        native_execution_resources_v1(37, 0),
        Err(PlironOptimizationErrorV12::Mapping(
            crate::KirOptimizationMapErrorV12::Limit
        ))
    ));
    assert!(native_execution_resources_v1(usize::MAX, 1).is_err());
}
