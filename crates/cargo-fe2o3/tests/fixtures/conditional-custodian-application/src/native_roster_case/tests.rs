use super::*;
fn case(count: usize, mode: Mode) -> Case {
    Case {
        producer_source: "src/lib.rs".into(),
        devices: (1..=count as u64).collect(),
        mode,
    }
}
#[test]
fn native_roster_requires_explicit_mode_bounded_unique_canonical_devices() {
    for mode in [Mode::NativeXgmi, Mode::HostStaged] {
        for count in 2..=8 {
            let mut args: Vec<OsString> = [
                "--native-v5-roster",
                "--producer-source",
                "src/lib.rs",
                "--transport",
                mode.token(),
                "--devices",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            args.extend((1..=count).map(|i| OsString::from(format!("0x{i:016x}"))));
            assert_eq!(parse(args.clone()).unwrap(), case(count, mode));
            args.push("0x0000000000000001".into());
            assert!(parse(args).is_err());
        }
    }
    for suffix in [
        vec![],
        vec!["0x0000000000000001"],
        vec!["0x1", "0x0000000000000002"],
        vec!["0x0000000000000000", "0x0000000000000002"],
        vec!["0x000000000000000A", "0x0000000000000002"],
    ] {
        let args = [
            "--native-v5-roster",
            "--producer-source",
            "src/lib.rs",
            "--transport",
            "native-xgmi",
            "--devices",
        ]
        .into_iter()
        .chain(suffix)
        .map(OsString::from);
        assert!(parse(args).is_err());
    }
}
#[test]
fn native_roster_full_directed_campaign_requires_every_shard_and_exact_mechanism() {
    for mode in [Mode::NativeXgmi, Mode::HostStaged] {
        for count in 2..=8 {
            let case = case(count, mode);
            let observed: Vec<_> = case
                .devices
                .iter()
                .enumerate()
                .map(|(i, uid)| (*uid, 128 + i as u16))
                .collect();
            let mut ledger = Ledger::new();
            assert!(ledger.route(&case, (0, 1), 1).is_err());
            assert!(ledger.finish(&case, &observed).is_err());
            for shard in 0..count {
                ledger.fill(&case, shard).unwrap();
            }
            assert!(ledger.fill(&case, 0).is_err());
            for (i, route) in case.routes().enumerate() {
                let expected = if mode == Mode::NativeXgmi {
                    i as u64 + 1
                } else {
                    0
                };
                assert!(ledger.route(&case, route, expected + 1).is_err());
                ledger.route(&case, route, expected).unwrap();
                assert!(ledger.route(&case, route, expected).is_err());
            }
            assert_eq!(ledger.complete, count * (count - 1));
            assert_eq!(ledger.observations.len(), count * (count - 1));
            for (ordinal, (&(source, destination, counter), pair)) in
                ledger.observations.iter().zip(case.routes()).enumerate()
            {
                assert_eq!((source, destination), pair);
                assert_eq!(
                    counter,
                    if mode == Mode::NativeXgmi {
                        ordinal as u64 + 1
                    } else {
                        0
                    }
                );
            }
            let report = ledger.finish(&case, &observed).unwrap();
            assert_eq!(
                report.matches("\"payload_bytes\"").count(),
                count * (count - 1)
            );
            assert!(report.contains("\"directed_pairs\":["));
            assert!(
                ledger
                    .finish(&case, &observed)
                    .unwrap()
                    .contains(mode.token())
            );
            let mut alias = observed.clone();
            alias[1].1 = alias[0].1;
            assert!(ledger.finish(&case, &alias).is_err());
            let mut missing = observed.clone();
            missing.pop();
            assert!(ledger.finish(&case, &missing).is_err());
        }
    }
}
#[test]
fn native_roster_guards_and_source_extent_reject_route_corruption() {
    let case = case(8, Mode::NativeXgmi);
    for (source, destination) in case.routes() {
        let output: Vec<_> = (0..case.elements(source) as u32).collect();
        let payload = case.payload(source, &output).unwrap();
        let mut frame = case.frame(source, destination);
        frame[GUARD_BYTES..GUARD_BYTES + payload.len()].copy_from_slice(&payload);
        case.check_route(source, destination, &payload, &payload, &frame)
            .unwrap();
        for index in [0, 8, 16, TAG_BYTES, payload.len() - 1] {
            let mut substituted = payload.clone();
            substituted[index] ^= 1;
            let mut matching_frame = case.frame(source, destination);
            matching_frame[GUARD_BYTES..GUARD_BYTES + substituted.len()]
                .copy_from_slice(&substituted);
            assert!(
                case.check_route(
                    source,
                    destination,
                    &substituted,
                    &substituted,
                    &matching_frame
                )
                .is_err()
            );
        }
        for index in [
            0,
            GUARD_BYTES - 1,
            GUARD_BYTES,
            GUARD_BYTES + payload.len(),
            FRAME_BYTES - 1,
        ] {
            frame[index] ^= 1;
            assert!(
                case.check_route(source, destination, &payload, &payload, &frame)
                    .is_err()
            );
            frame[index] ^= 1;
        }
        assert!(
            case.check_route(
                source,
                destination,
                &payload,
                &payload[..payload.len() - 1],
                &frame
            )
            .is_err()
        );
        assert!(case.payload(source, &output[..output.len() - 1]).is_err());
    }
}
