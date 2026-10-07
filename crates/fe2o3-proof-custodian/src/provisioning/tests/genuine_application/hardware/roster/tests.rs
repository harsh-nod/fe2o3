use super::*;

fn fixture(count: usize) -> (Vec<ObservedGpu>, Vec<(u64, u16)>) {
    let nodes: Vec<_> = (0..count)
        .map(|index| ObservedGpu {
            uid: index as u64 + 1,
            gpu_id: index as u64 + 20,
            minor: index as u16 + 128,
        })
        .collect();
    let renders = nodes.iter().map(|node| (node.uid, node.minor)).collect();
    (nodes, renders)
}

#[test]
fn request_parser_is_bounded_exact_and_never_uses_pair_fallback() {
    assert_eq!(
        parse_ids(r#"["0xffffffffffffffff","0x1"]"#).unwrap(),
        [u64::MAX, 1]
    );
    for input in [
        "",
        "[]",
        r#"["0x1"]"#,
        r#"["0x1","0x01"]"#,
        r#"["0x0","0x2"]"#,
        r#"["0x1",2]"#,
        r#"["0x1","0X2"]"#,
        r#"["0x1","0x2 "]"#,
        r#"["0x1","0x10000000000000000"]"#,
        r#"{"devices":["0x1","0x2"]}"#,
    ] {
        assert!(parse_ids(input).is_err(), "{input}");
    }
    assert!(parse_ids(&" ".repeat(4097)).is_err());
    for count in [2, 8, 9] {
        let request = serde_json::to_string(
            &(1..=count)
                .map(|uid| format!("{uid:#x}"))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(parse_ids(&request).is_ok(), count <= MAX_DEVICES);
    }
}

#[test]
fn selection_preserves_order_checks_all_directed_routes_and_names_exclusions() {
    let (nodes, renders) = fixture(8);
    for count in 2..=8 {
        let ids: Vec<_> = (1..=count as u64).rev().collect();
        let mut routes = Vec::new();
        let selection = select(&ids, &nodes, &renders, |left, right| {
            routes.push((left, right));
            Ok(())
        })
        .unwrap();
        assert_eq!(routes.len(), count * (count - 1));
        let unique: std::collections::BTreeSet<_> = routes.iter().copied().collect();
        assert_eq!(unique.len(), routes.len());
        assert!(
            routes
                .iter()
                .all(|(left, right)| left != right && unique.contains(&(*right, *left)))
        );
        assert_eq!(
            selection.devices,
            ids.iter()
                .map(|uid| format!("{uid:#018x}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            selection.render_minors,
            (128..128 + count as u16).rev().collect::<Vec<_>>()
        );
        assert_eq!(selection.observed_host_devices.len(), 8);
        assert_eq!(selection.excluded_unselected_devices.len(), 8 - count);
        assert_eq!(selection.occupancy, "not-measured");
        assert_eq!(selection.scope, "selected-admitted-context-only");
        match_selection(&serde_json::to_string(&selection).unwrap(), &selection).unwrap();
        for edge in 0..routes.len() {
            let mut seen = 0;
            assert!(
                select(&ids, &nodes, &renders, |_, _| {
                    let current = seen;
                    seen += 1;
                    if current == edge {
                        Err("missing route".into())
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(seen, edge + 1);
        }
    }
}

#[test]
fn ambiguous_uid_gpu_id_render_alias_and_missing_correlations_reject_before_routes() {
    let (nodes, renders) = fixture(3);
    let mut cases = Vec::new();
    let mut duplicate = nodes.clone();
    duplicate.push(nodes[0]);
    cases.push((duplicate, renders.clone()));
    let mut same_minor = nodes.clone();
    same_minor[1].minor = same_minor[0].minor;
    cases.push((same_minor, vec![(1, 128), (2, 128), (3, 130)]));
    let mut same_gpu = nodes.clone();
    same_gpu[1].gpu_id = same_gpu[0].gpu_id;
    cases.push((same_gpu, renders.clone()));
    for invalid in [0, u64::from(u32::MAX) + 1] {
        let mut wrong = nodes.clone();
        wrong[0].gpu_id = invalid;
        cases.push((wrong, renders.clone()));
    }
    for invalid in [127, 256] {
        let mut wrong = nodes.clone();
        wrong[0].minor = invalid;
        cases.push((wrong, renders.clone()));
    }
    cases.push((nodes.clone(), vec![(1, 128), (1, 128), (2, 129)]));
    cases.push((nodes.clone(), vec![(1, 128), (99, 128), (2, 129)]));
    cases.push((nodes.clone(), vec![(1, 128), (1, 130), (2, 129)]));
    cases.push((nodes.clone(), vec![(2, 129)]));
    cases.push((nodes[..1].to_vec(), renders));
    for (nodes, renders) in cases {
        assert!(
            select(&[1, 2], &nodes, &renders, |_, _| panic!(
                "invalid roster reached route checks"
            ))
            .is_err()
        );
    }
}

#[test]
fn selection_revalidation_rejects_every_changed_field_and_extra_keys() {
    let (nodes, renders) = fixture(3);
    let selection = select(&[2, 1], &nodes, &renders, |_, _| Ok(())).unwrap();
    let original = serde_json::to_value(&selection).unwrap();
    for (field, replacement) in [
        ("schema", serde_json::json!("wrong")),
        ("devices", serde_json::json!(["0x1", "0x2"])),
        ("render_minors", serde_json::json!([128, 129])),
        ("observed_host_devices", serde_json::json!([])),
        ("excluded_unselected_devices", serde_json::json!([])),
        ("occupancy", serde_json::json!("idle")),
        ("scope", serde_json::json!("all-host-qualified")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut changed = original.clone();
        changed[field] = replacement;
        assert!(
            match_selection(&changed.to_string(), &selection).is_err(),
            "{field}"
        );
    }
    let encoded = serde_json::to_string(&selection).unwrap();
    assert!(
        match_selection(
            &encoded.replacen('{', r#"{"schema":"duplicate","#, 1),
            &selection
        )
        .is_err()
    );
}

#[test]
fn reports_require_complete_canonical_roster_shards_edges_and_non_overlap_claim() {
    for count in 2..=8 {
        let ids: Vec<_> = (1..=count as u64).collect();
        let expected = expected_report(&ids).unwrap();
        let encoded = serde_json::to_string(&expected).unwrap();
        verify_report(encoded.as_bytes(), &ids).unwrap();
        verify_report(format!("ordinary diagnostic\n{encoded}\n").as_bytes(), &ids).unwrap();
        for line in [
            "".to_owned(),
            format!("{encoded}\n{encoded}"),
            format!("{encoded}\n{{}}"),
            format!("{encoded}\n{{\"schema\":\"fe2o3.genuine-two-gpu.v1\"}}"),
            encoded.replacen('{', r#"{"schema":"duplicate","#, 1),
        ] {
            assert!(verify_report(line.as_bytes(), &ids).is_err());
        }
        let original = serde_json::to_value(&expected).unwrap();
        for (field, replacement) in [
            ("schema", serde_json::json!("fe2o3.genuine-two-gpu.v1")),
            ("devices", serde_json::json!([])),
            ("shards", serde_json::json!([])),
            ("directed_peer_pairs", serde_json::json!([])),
            (
                "native_peer_completions",
                serde_json::json!(count * (count - 1) - 1),
            ),
            ("roster_scope", serde_json::json!("all-host-devices")),
            ("ordering", serde_json::json!("concurrent")),
            ("copy_compute_overlap", serde_json::json!("measured")),
            ("peer_tag_origin", serde_json::json!("device")),
            ("shutdown", serde_json::json!("quarantined")),
            ("extra", serde_json::json!(true)),
        ] {
            let mut changed = original.clone();
            changed[field] = replacement;
            assert!(
                verify_report(changed.to_string().as_bytes(), &ids).is_err(),
                "{field}"
            );
        }
        for edge in 0..expected.directed_peer_pairs.len() {
            let mut changed = original.clone();
            changed["directed_peer_pairs"]
                .as_array_mut()
                .unwrap()
                .remove(edge);
            assert!(verify_report(changed.to_string().as_bytes(), &ids).is_err());
            let mut changed = original.clone();
            changed["directed_peer_pairs"][edge][0] =
                changed["directed_peer_pairs"][edge][1].clone();
            assert!(verify_report(changed.to_string().as_bytes(), &ids).is_err());
        }
        for shard in 0..count {
            let mut changed = original.clone();
            changed["shards"][shard]["elements"] = serde_json::json!(64);
            assert!(verify_report(changed.to_string().as_bytes(), &ids).is_err());
        }
    }
}
