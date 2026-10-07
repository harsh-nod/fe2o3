use super::*;

fn parse(args: &[&str]) -> Result<Option<ExecutionCase>, String> {
    parse_execution_case(args.iter().map(OsString::from))
}

fn roster(count: usize) -> RosterCase {
    let args = std::iter::once("--roster".to_owned())
        .chain((1..=count).map(|uid| format!("{uid:#x}")))
        .map(OsString::from);
    let Some(ExecutionCase::Roster(case)) = parse_execution_case(args).unwrap() else {
        panic!("roster expected")
    };
    case
}

fn payload(case: RosterCase, shard: usize) -> Vec<u8> {
    case.payload(
        shard,
        &(0..case.elements(shard) as u32)
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn completed(case: RosterCase) -> CampaignLedger {
    let mut ledger = CampaignLedger::new(case);
    for shard in 0..case.count() {
        ledger.record_fill(shard).unwrap();
    }
    ledger
}

#[test]
fn roster_is_explicit_and_pair_controls_remain_unchanged() {
    assert_eq!(parse(&[]).unwrap(), None);
    for args in [
        vec!["0x1", "0x2"],
        vec!["0x1", "0x2", "second-coverage-reject"],
        vec!["0x1", "0x2", "peer-deadline-before-submit"],
    ] {
        let pair = native_case::parse_case(args.iter().map(OsString::from)).unwrap();
        assert_eq!(parse(&args).unwrap(), pair.map(ExecutionCase::Pair));
    }
    for count in 2..=MAX_DEVICES {
        let case = roster(count);
        assert_eq!(case.devices(), (1..=count as u64).collect::<Vec<_>>());
        assert_eq!(case.count(), count);
        assert_eq!(
            case.result_budget_bytes(),
            (0..count).map(|i| (ELEMENTS + i) * 8).sum()
        );
    }
}

#[test]
fn invalid_missing_duplicate_excess_and_non_utf8_rosters_reject() {
    for args in [
        vec!["--roster"],
        vec!["--roster", "0x1"],
        vec!["--roster", "0x0", "0x2"],
        vec!["--roster", "0x1", "0x01"],
        vec!["--roster", "1", "0x2"],
        vec!["--roster", "0X1", "0x2"],
        vec!["--roster", "0x1 ", "0x2"],
        vec!["--roster", "0x", "0x2"],
        vec!["--roster", "0x10000000000000000", "0x2"],
        vec!["--roster", "0x1", "0x2", "second-coverage-reject"],
        vec!["--roster", "0x1", "0x2", "positive"],
        vec!["--all", "0x1", "0x2"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    let oversized = std::iter::once(OsString::from("--roster"))
        .chain((1..=MAX_DEVICES + 1).map(|id| OsString::from(format!("{id:#x}"))));
    assert!(parse_execution_case(oversized).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        assert!(
            parse_execution_case([
                "--roster".into(),
                "0x1".into(),
                OsString::from_vec(vec![255])
            ])
            .is_err()
        );
    }
}

#[test]
fn roster_is_exact_not_a_subset_or_reordered_identity_match() {
    let case = roster(MAX_DEVICES);
    case.check_observed_roster(case.devices()).unwrap();
    for count in 0..MAX_DEVICES {
        assert!(
            case.check_observed_roster(&case.devices()[..count])
                .is_err()
        );
    }
    let mut observed = case.devices().to_vec();
    observed.swap(0, 1);
    assert!(case.check_observed_roster(&observed).is_err());
    observed.push(9);
    assert!(case.check_observed_roster(&observed).is_err());
    for index in 0..MAX_DEVICES {
        let mut changed = case.devices().to_vec();
        changed[index] = 99;
        assert!(case.check_observed_roster(&changed).is_err());
    }
}

#[test]
fn every_shard_has_its_own_extent_and_every_fill_value_is_checked() {
    let case = roster(MAX_DEVICES);
    for shard in 0..case.count() {
        let values: Vec<_> = (0..case.elements(shard) as u32).collect();
        case.check_fill(shard, &values).unwrap();
        assert!(case.check_fill(shard, &values[..values.len() - 1]).is_err());
        for index in 0..values.len() {
            let mut changed = values.clone();
            changed[index] ^= 1;
            assert!(case.check_fill(shard, &changed).is_err());
            assert!(
                case.payload(
                    shard,
                    &changed
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>()
                )
                .is_err()
            );
        }
        assert_eq!(payload(case, shard).len(), case.payload_bytes(shard));
    }
}

#[test]
fn complete_directed_campaign_rejects_missing_duplicate_or_reversed_receipts() {
    for count in 2..=MAX_DEVICES {
        let case = roster(count);
        let routes: Vec<_> = case.routes().collect();
        assert_eq!(routes.len(), count * (count - 1));
        let unique: std::collections::BTreeSet<_> = routes.iter().copied().collect();
        assert_eq!(unique.len(), routes.len());
        assert!(
            unique
                .iter()
                .all(|(source, destination)| source != destination
                    && unique.contains(&(*destination, *source)))
        );
        assert!(CampaignLedger::new(case).record_peer(routes[0], 1).is_err());
        for missing in 0..count {
            let mut ledger = CampaignLedger::new(case);
            for shard in 0..missing {
                ledger.record_fill(shard).unwrap();
            }
            assert!(ledger.finish().is_err());
        }
        for prefix in 0..routes.len() {
            let mut ledger = completed(case);
            for (index, route) in routes[..prefix].iter().enumerate() {
                ledger.record_peer(*route, index as u64 + 1).unwrap();
            }
            let route = routes[prefix];
            assert!(
                ledger
                    .record_peer((route.1, route.0), prefix as u64 + 1)
                    .is_err()
            );
            assert!(ledger.record_peer(route, prefix as u64).is_err());
            if prefix > 0 {
                assert!(
                    ledger
                        .record_peer(routes[prefix - 1], prefix as u64 + 1)
                        .is_err()
                );
            }
            assert!(ledger.finish().is_err());
        }
        let mut ledger = completed(case);
        assert!(ledger.record_fill(count - 1).is_err());
        for (index, route) in routes.iter().enumerate() {
            ledger.record_peer(*route, index as u64 + 1).unwrap();
        }
        assert!(
            ledger
                .record_peer(routes[0], routes.len() as u64 + 1)
                .is_err()
        );
        let report = ledger.finish().unwrap().json();
        assert!(report.contains(&format!("\"native_peer_completions\":{}", routes.len())));
        assert!(report.contains("\"ordering\":\"compute-complete-before-peer-submit\""));
        assert!(report.contains("\"copy_compute_overlap\":\"not-measured\""));
        assert!(report.contains("\"peer_tag_origin\":\"host\""));
    }
}

#[test]
fn all_routes_detect_noop_wrong_shard_and_every_source_payload_and_guard_corruption() {
    let case = roster(MAX_DEVICES);
    let payloads: Vec<_> = (0..case.count())
        .map(|shard| payload(case, shard))
        .collect();
    for (source, destination) in case.routes() {
        let payload = &payloads[source];
        let mut frame = case.destination_frame(source, destination);
        assert!(
            case.check_route(source, destination, payload, payload, &frame)
                .is_err()
        );
        frame[GUARD_BYTES..GUARD_BYTES + payload.len()].copy_from_slice(payload);
        case.check_route(source, destination, payload, payload, &frame)
            .unwrap();
        for byte in 0..frame.len() {
            let mut changed = frame.clone();
            changed[byte] ^= 1;
            assert!(
                case.check_route(source, destination, payload, payload, &changed)
                    .is_err()
            );
        }
        for byte in 0..payload.len() {
            let mut changed = payload.clone();
            changed[byte] ^= 1;
            assert!(
                case.check_route(source, destination, payload, &changed, &frame)
                    .is_err()
            );
        }
        for (other, other_payload) in payloads.iter().enumerate() {
            if other == source {
                continue;
            }
            let mut swapped = case.destination_frame(source, destination);
            swapped[GUARD_BYTES..GUARD_BYTES + other_payload.len()].copy_from_slice(other_payload);
            assert!(
                case.check_route(source, destination, payload, payload, &swapped)
                    .is_err()
            );
            assert!(
                case.check_route(source, destination, other_payload, other_payload, &swapped)
                    .is_err()
            );
        }
        assert!(
            case.check_route(source, destination, &[], &[], &frame)
                .is_err()
        );
        assert!(
            case.check_route(
                source,
                destination,
                payload,
                payload,
                &frame[..frame.len() - 1]
            )
            .is_err()
        );
        frame.push(0);
        assert!(
            case.check_route(source, destination, payload, payload, &frame)
                .is_err()
        );
    }
}
