use super::*;

fn symbols() -> BTreeMap<u64, String> {
    [(7, "example.kd".into())].into()
}

#[test]
fn native_packet_observation_preserves_order_identity_and_raw_ticks() {
    for count in [1, 64, 65, 649, 652, 688, 1024] {
        let symbols = symbols();
        let record = Record::new(
            [99, 3, 100],
            &vec![7; count],
            &symbols,
            (0..count).map(|i| (i as u64 + 1, i as u64 + 9)).collect(),
        )
        .unwrap();
        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["next_write"], 100 + count as u64);
        assert_eq!(
            json["packets"][count - 1],
            serde_json::json!([7, count, count + 8])
        );
        assert_eq!(json["clock"], "raw_gpu_clock_ticks");
        assert!(json.get("duration_ns").is_none());
    }
}

#[test]
fn native_packet_observation_rejects_invalid_ticks_and_rosters() {
    let symbols = symbols();
    for ticks in [
        vec![],
        vec![(0, 1)],
        vec![(1, 1)],
        vec![(2, 1)],
        vec![(1, 2); 2],
    ] {
        assert!(Record::new([99, 0, 0], &[7], &symbols, ticks).is_err());
    }
    assert!(Record::new([99, 0, 0], &[8], &symbols, vec![(1, 2)]).is_err());
    assert!(Record::new([99, 0, 0], &[7], &BTreeMap::new(), vec![(1, 2)]).is_err());
    for name in [String::new(), "x".repeat(1025)] {
        assert!(Record::new([99, 0, 0], &[7], &[(7, name)].into(), vec![(1, 2)]).is_err());
    }
}

#[test]
fn native_packet_observation_rejects_bounds_and_frontier_overflow() {
    for count in [0, 1025, usize::MAX] {
        assert!(validate_identity([1, 0, 0], count).is_err());
    }
    assert!(validate_identity([1, 0, u64::MAX], 1).is_err());
    assert_eq!(
        validate_identity([1, 0, u64::MAX - 1], 1).unwrap(),
        u64::MAX
    );
}

#[test]
fn native_packet_observation_does_not_infer_nonoverlap_or_clock_rate() {
    let symbols = symbols();
    let record = Record::new([99, 0, 0], &[7, 7], &symbols, vec![(100, 200), (90, 210)]).unwrap();
    assert_eq!(record.packets, [[7, 100, 200], [7, 90, 210]]);
}
