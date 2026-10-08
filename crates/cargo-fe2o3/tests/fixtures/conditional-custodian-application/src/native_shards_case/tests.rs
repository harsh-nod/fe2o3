use super::*;

fn args(count: usize) -> Vec<OsString> {
    [
        "--native-v5-shards",
        "--producer-source",
        "/source/src/lib.rs",
        "--devices",
    ]
    .into_iter()
    .map(OsString::from)
    .chain((0..count).map(|index| OsString::from(format!("0x{:016x}", index + 7))))
    .collect()
}

fn fixture() -> (Case, Vec<(u64, u16)>, Vec<Shard>) {
    let case = parse(args(3)).unwrap();
    let observed = case
        .devices
        .iter()
        .enumerate()
        .map(|(index, &uid)| (uid, 128 + index as u16))
        .collect();
    let shards = case
        .devices
        .iter()
        .enumerate()
        .map(|(index, &uid)| Shard {
            uid,
            render_minor: 128 + index as u16,
            node: index as u32 + 1,
            elements: case.elements(index),
            state: State::Succeeded,
            checked_values: case.elements(index),
        })
        .collect();
    (case, observed, shards)
}

#[test]
fn explicit_closed_selector_requires_two_through_eight_unique_uids() {
    for count in 2..=8 {
        assert_eq!(parse(args(count)).unwrap().devices.len(), count);
    }
    for count in [0, 1, 9] {
        assert!(parse(args(count)).is_err());
    }
    for bad in [
        "0x0000000000000000",
        "0x0000000000000007",
        "0x000000000000000A",
        "8",
        "--mock",
    ] {
        let mut input = args(2);
        input[5] = bad.into();
        assert!(parse(input).is_err());
    }
    for index in 0..4 {
        let mut input = args(2);
        input[index] = "".into();
        assert!(parse(input).is_err());
    }
    let mut input = args(2);
    input[2] = "a\0b".into();
    assert!(parse(input).is_err());
    let mut input = args(2);
    input[2] = "x".repeat(4097).into();
    assert!(parse(input).is_err());
}

#[test]
fn all_admitted_roster_refuses_subset_reorder_duplicate_and_foreign_render() {
    let (case, observed, _) = fixture();
    case.check_roster(&observed).unwrap();
    assert!(case.check_roster(&observed[..2]).is_err());
    let mut changed = observed.clone();
    changed.swap(0, 2);
    assert!(case.check_roster(&changed).is_err());
    changed = observed.clone();
    changed[2].1 = changed[0].1;
    assert!(case.check_roster(&changed).is_err());
    changed = observed;
    changed[2].0 = 999;
    assert!(case.check_roster(&changed).is_err());
}

#[test]
fn each_success_requires_exact_independent_values_and_extent() {
    let (case, _, _) = fixture();
    assert!(case.check_fill(case.devices.len(), &[]).is_err());
    for index in 0..case.devices.len() {
        let mut values: Vec<_> = (0..case.elements(index) as u32).collect();
        case.check_fill(index, &values).unwrap();
        assert!(case.check_fill(index, &values[..values.len() - 1]).is_err());
        values[5] ^= 1;
        assert!(case.check_fill(index, &values).is_err());
    }
}

#[test]
fn report_independently_refuses_unbounded_zero_and_duplicate_rosters() {
    for devices in [vec![], vec![7], (1..=9).collect(), vec![0, 7], vec![7, 7]] {
        let case = Case {
            producer_source: "source.rs".into(),
            devices,
        };
        let observed: Vec<_> = case
            .devices
            .iter()
            .enumerate()
            .map(|(index, &uid)| (uid, 128 + index as u16))
            .collect();
        assert!(case.check_roster(&observed).is_err());
        assert!(case.report(&observed, &[], false).is_err());
    }
}

#[test]
fn exact_partial_manifest_preserves_both_healthy_original_children() {
    let (case, observed, mut shards) = fixture();
    for failure in [
        State::RejectedBeforePublication,
        State::DeviceUnavailableBeforeActivation,
        State::ReadbackFailed,
    ] {
        shards[0].state = failure;
        shards[0].checked_values = 0;
        let report = case.report(&observed, &shards, true).unwrap();
        assert!(report.contains("\"successful_shards\":2"));
        assert!(report.contains("\"failed_shards\":1"));
        assert!(report.contains("\"uid\":\"0x0000000000000008\""));
        assert!(report.contains("\"uid\":\"0x0000000000000009\""));
        assert!(report.contains("\"copied_bytes\":0"));
        assert!(report.contains("\"direct_native_data_transfer\":false"));
        assert!(!report.contains("physical_overlap_measured\":true"));
        assert!(case.report(&observed, &shards, false).is_err());
    }
}

#[test]
fn report_cannot_hide_missing_duplicate_foreign_or_unchecked_shards() {
    let (case, observed, shards) = fixture();
    case.report(&observed, &shards, false).unwrap();
    assert!(case.report(&observed, &shards[..2], false).is_err());
    assert!(case.report(&observed, &shards, true).is_err());
    for axis in 0..6 {
        let (_, _, mut changed) = fixture();
        match axis {
            0 => changed[2].node = 1,
            1 => changed[2].uid = 7,
            2 => changed[2].render_minor = 128,
            3 => changed[2].elements += 1,
            4 => changed[2].checked_values = 0,
            5 => changed[2].state = State::ReadbackFailed,
            _ => unreachable!(),
        }
        assert!(case.report(&observed, &changed, axis == 5).is_err());
    }
}

#[test]
fn entirely_failed_roster_is_not_renamed_healthy_or_complete_success() {
    let (case, observed, mut shards) = fixture();
    for shard in &mut shards {
        shard.state = State::RejectedBeforePublication;
        shard.checked_values = 0;
    }
    let report = case.report(&observed, &shards, true).unwrap();
    assert!(report.contains("\"successful_shards\":0"));
    assert!(report.contains("\"failed_shards\":3"));
    assert!(report.contains("\"settled_local_failure\":true"));
    assert!(case.report(&observed, &shards, false).is_err());
}
