use super::*;

fn roster() -> [ObservedGpu; 2] {
    [
        ObservedGpu {
            uid: 1,
            gpu_id: 101,
            minor: 128,
        },
        ObservedGpu {
            uid: 2,
            gpu_id: 202,
            minor: 129,
        },
    ]
}

#[test]
fn uid_parser_is_exact_and_preserves_order_and_full_width() {
    assert_eq!(
        parse_ids(["0xffffffffffffffff", "0x01"]).unwrap(),
        [u64::MAX, 1]
    );
    for values in [
        ["0x0", "0x1"],
        ["0x1", "0x01"],
        ["0x", "0x2"],
        ["0x10000000000000000", "0x2"],
        ["0x+1", "0x2"],
        ["1", "0x2"],
        ["0x1 ", "0x2"],
        ["0xg", "0x2"],
        ["0x1", "0x2;id"],
    ] {
        assert!(parse_ids(values).is_err(), "{values:?}");
    }
}

#[test]
fn both_routes_use_kernel_ids_in_requested_uid_order() {
    let mut calls = Vec::new();
    let selected = select([2, 1], &roster(), &[(1, 128), (2, 129)], |a, b| {
        calls.push((a, b));
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, [(202, 101), (101, 202)]);
    assert_eq!(
        selected.devices,
        ["0x0000000000000002", "0x0000000000000001"]
    );
    assert_eq!(selected.render_minors, [129, 128]);
    let mut calls = Vec::new();
    assert!(
        select([1, 2], &roster(), &[(1, 128), (2, 129)], |a, b| {
            calls.push((a, b));
            if a == 202 {
                Err("reverse unavailable".into())
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert_eq!(calls, [(101, 202), (202, 101)]);
}

#[test]
fn absent_ambiguous_or_uncorrelated_devices_reject_before_routes() {
    let good = roster().to_vec();
    let mut cases = vec![
        (vec![good[0]], vec![(1, 128), (2, 129)]),
        (vec![good[0], good[0], good[1]], vec![(1, 128), (2, 129)]),
        (good.clone(), vec![(1, 128), (2, 130)]),
        (good.clone(), vec![(1, 128), (1, 128), (2, 129)]),
    ];
    for minor in [127, 128, 256] {
        let mut changed = good.clone();
        changed[1].minor = minor;
        cases.push((changed, vec![(1, 128), (2, minor)]));
    }
    let mut changed = good.clone();
    changed[1].gpu_id = u64::from(u32::MAX) + 1;
    cases.push((changed, vec![(1, 128), (2, 129)]));
    for (nodes, renders) in cases {
        assert!(
            select([1, 2], &nodes, &renders, |_, _| panic!(
                "route before valid roster"
            ))
            .is_err()
        );
    }
    for ids in [[0, 1], [1, 1], [3, 2]] {
        assert!(
            select(ids, &good, &[(1, 128), (2, 129)], |_, _| panic!(
                "invalid UID"
            ))
            .is_err()
        );
    }
}

#[test]
fn selection_json_is_exact_and_retains_u64_precision() {
    let current = Selection {
        schema: SELECTION_SCHEMA.into(),
        devices: [format!("{:#018x}", u64::MAX), "0x0000000000000001".into()],
        render_minors: [255, 128],
    };
    let encoded = serde_json::to_string(&current).unwrap();
    match_selection(&encoded, &current).unwrap();
    for (key, value) in [
        ("schema", serde_json::json!("wrong")),
        (
            "devices",
            serde_json::json!(["0x0000000000000001", "0xffffffffffffffff"]),
        ),
        ("render_minors", serde_json::json!([128, 255])),
        ("extra", serde_json::json!(1)),
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        changed[key] = value;
        assert!(match_selection(&changed.to_string(), &current).is_err());
    }
    assert!(
        match_selection(
            &encoded.replace("\"schema\":", "\"schema\":\"duplicate\",\"schema\":"),
            &current
        )
        .is_err()
    );
}

#[test]
fn device_metadata_rejects_regular_files_symlinks_and_wrong_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let regular = dir.path().join("regular");
    std::fs::write(&regular, b"not a GPU").unwrap();
    assert!(checked_node(&regular, None).is_err());
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink("/dev/null", &alias).unwrap();
    assert!(checked_node(&alias, None).is_err());
    checked_node(Path::new("/dev/null"), Some((1, 3))).unwrap();
    assert!(checked_node(Path::new("/dev/null"), Some((226, 128))).is_err());
}

#[test]
fn dac_uses_owner_group_precedence_and_only_needed_groups() {
    assert_eq!(required_group(0, 993, 0o660).unwrap(), Some(993));
    assert_eq!(required_group(0, 0, 0o666).unwrap(), None);
    assert_eq!(required_group(1000, 993, 0o600).unwrap(), None);
    assert_eq!(required_group(0, 1000, 0o660).unwrap(), None);
    for (uid, gid, mode) in [
        (0, 0, 0o660),
        (0, u32::MAX, 0o660),
        (1000, 993, 0o066),
        (0, 1000, 0o606),
        (0, 993, 0o640),
    ] {
        assert!(required_group(uid, gid, mode).is_err());
    }
    assert!(read_write_allowed(0, 993, 0o660, &[993]));
    assert!(!read_write_allowed(0, 993, 0o660, &[]));
    assert!(read_write_allowed(0, 993, 0o606, &[]));
    assert!(!read_write_allowed(0, 993, 0o606, &[993]));
}

fn report() -> String {
    serde_json::json!({"schema":REPORT_SCHEMA,"devices":["0x0000000000000001","0x0000000000000002"],
        "elements":65,"native_peer_completions":2,"shutdown":"released"})
    .to_string()
}

#[test]
fn report_requires_one_complete_matching_execution_record() {
    let good = report();
    let escaped = good.replace(REPORT_SCHEMA, "fe2o3.genuine-two-gpu.\\u00761");
    verify_report(escaped.as_bytes(), [1, 2]).unwrap();
    assert!(verify_report(format!("{good}\n{escaped}\n").as_bytes(), [1, 2]).is_err());
    assert!(
        verify_report(
            format!("{good}\n{{\"unrelated\":true}}\n").as_bytes(),
            [1, 2]
        )
        .is_err()
    );
    verify_report(format!("admission log\n{good}\n").as_bytes(), [1, 2]).unwrap();
    for bad in [
        String::new(),
        "admission only\n".into(),
        format!("{good}\n{good}\n"),
        format!("{good}\nmalformed {REPORT_SCHEMA}"),
        format!("prefix {good}"),
        good.replace("\"elements\":65", "\"elements\":65,\"elements\":65"),
    ] {
        assert!(verify_report(bad.as_bytes(), [1, 2]).is_err(), "{bad}");
    }
    assert!(verify_report(good.as_bytes(), [2, 1]).is_err());
    for (key, value) in [
        ("elements", serde_json::json!(64)),
        ("native_peer_completions", serde_json::json!(1)),
        ("shutdown", serde_json::json!("retained")),
        ("extra", serde_json::json!(true)),
        ("schema", serde_json::json!("fe2o3.genuine-two-gpu.v2")),
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(&good).unwrap();
        changed[key] = value;
        assert!(verify_report(changed.to_string().as_bytes(), [1, 2]).is_err());
    }
}

#[test]
fn negative_report_requires_exact_mode_clean_exit_and_no_positive_claim() {
    for control in [
        Control::SecondCoverageReject,
        Control::PeerDeadlineBeforeSubmit,
    ] {
        let good = serde_json::json!({"schema":CONTROL_SCHEMA,
            "devices":["0x0000000000000001","0x0000000000000002"],
            "mode":control.token(),"shutdown":"released"})
        .to_string();
        let check =
            |text: &str, code| verify_control_report(text.as_bytes(), [1, 2], control, code);
        check(&format!("admission log\n{good}\n"), Some(0)).unwrap();
        for code in [None, Some(1), Some(41), Some(42), Some(43), Some(137)] {
            assert!(check(&good, code).is_err(), "{code:?}");
        }
        assert!(verify_report(good.as_bytes(), [1, 2]).is_err());
        assert!(verify_control_report(good.as_bytes(), [2, 1], control, Some(0)).is_err());
        let positive = report();
        let escaped = positive.replace(REPORT_SCHEMA, "fe2o3.genuine-two-gpu.\\u00761");
        for bad in [
            String::new(),
            "admission only".into(),
            positive.clone(),
            format!("{good}\n{good}"),
            format!("{good}\n{positive}"),
            format!("{good}\n{escaped}"),
            format!("{good}\nmalformed {REPORT_SCHEMA}"),
            format!("{good}\n{{\"unrelated\":true}}"),
            format!("prefix {good}"),
            good.replace("\"mode\":", "\"mode\":\"duplicate\",\"mode\":"),
        ] {
            assert!(check(&bad, Some(0)).is_err(), "{bad}");
        }
        for (key, value) in [
            ("mode", serde_json::json!("unknown")),
            ("shutdown", serde_json::json!("retained")),
            ("schema", serde_json::json!(REPORT_SCHEMA)),
            ("extra", serde_json::json!(true)),
            ("devices", serde_json::json!(["0x1", "0x2"])),
        ] {
            let mut changed: serde_json::Value = serde_json::from_str(&good).unwrap();
            changed[key] = value;
            assert!(check(&changed.to_string(), Some(0)).is_err());
        }
        assert!(verify_control_report(&[255], [1, 2], control, Some(0)).is_err());
    }
}
