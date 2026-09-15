use super::*;

#[test]
fn lane_scope_requires_exact_flag_and_role_without_scanning_other_scopes() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let candidates = candidates(view);
    for (setting, role) in [
        (None, Some("lane")), (Some("0"), Some("lane")),
        (Some("true"), Some("lane")), (Some("1"), None),
        (Some("1"), Some("matrix")), (Some("1"), Some("subgroup")),
        (Some("1"), Some("Lane")), (Some("1"), Some("lane ")),
    ] {
        let mut observed = Observation::with_lane_role(
            Some(view), std::iter::from_fn(|| panic!("disabled lane iterator")),
            &candidates, setting.map(OsStr::new), role.map(OsStr::new),
        );
        observed.after(candidates[0].site, "unused", &candidates);
        assert!(observed.view.is_none());
        assert_eq!(observed.remaining, MAX_STEPS);
        assert_eq!(observed.len, 0);
    }
    let disabled_outer = Observation::with_role(
        Some(view), std::iter::from_fn(|| panic!("matrix iterator")),
        std::iter::from_fn(|| panic!("subgroup iterator")), &candidates,
        Some(OsStr::new("1")), Some(OsStr::new("lane")),
    );
    assert!(disabled_outer.view.is_none());
    assert_eq!(disabled_outer.remaining, MAX_STEPS);
}

#[test]
fn lane_scope_keeps_first_inner_failure_and_exact_completed_map_membership() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut candidates = candidates(view);
    let mut observed = Observation::with_lane_role(
        Some(view), candidates.iter().map(|c| c.source_type.index()), &candidates,
        Some(OsStr::new("1")), Some(OsStr::new("lane")),
    );
    assert!(observed.view.is_some());
    candidates[0].valid = false;
    let point = candidates[1].site;
    observed.after(point, "lane-ordinary-statement", &candidates);
    observed.after(candidates[0].site, "lane-later-use", &candidates);
    observed.component(0, Some(0), false, &candidates);
    let accepted = BTreeMap::from([(candidates[1].site, ())]);
    let before = (source.canonical_encoding().to_vec(), *view.identity(),
        candidates.iter().map(|c| (c.valid, c.consumers, c.intrinsic_consumer)).collect::<Vec<_>>(),
        observed.remaining);
    let mut output = Vec::new();
    observed.write(&mut output, view, &candidates, &accepted, |_| None).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("scope=lane"));
    assert!(output.contains("mode=lane-ordinary-statement"));
    assert!(!output.contains("mode=lane-later-use"));
    assert!(output.contains("source_block=Some("));
    assert!(output.contains("source_statement=Some("));
    assert!(output.contains("accepted_borrow=true"));
    assert!(output.contains("accepted_borrow=false"));
    assert_eq!(before, (source.canonical_encoding().to_vec(), *view.identity(),
        candidates.iter().map(|c| (c.valid, c.consumers, c.intrinsic_consumer)).collect::<Vec<_>>(),
        observed.remaining));
}

#[test]
fn lane_scope_shares_all_existing_selection_work_and_output_limits() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let first = candidates(view)[0];
    let candidates = vec![first; MAX_TRACKED + 1];
    let mut observed = Observation::with_lane_role(
        Some(view), [first.source_type.index()].into_iter(), &candidates,
        Some(OsStr::new("1")), Some(OsStr::new("lane")),
    );
    assert_eq!(observed.len, MAX_TRACKED);
    assert!(observed.truncated);
    observed.remaining = 1;
    observed.after(first.site, "exhausted", &candidates);
    assert_eq!(observed.remaining, 0);
    assert!(observed.tracked[..observed.len].iter().flatten().all(|c| c.failure.is_none()));
    assert!(candidates.iter().all(|c| c.valid && c.consumers == 0));
    let oversized = Observation::with_lane_role(
        Some(view), 0..MAX_OWNED as u32 + 1, &[],
        Some(OsStr::new("1")), Some(OsStr::new("lane")),
    );
    assert_eq!(oversized.owned_len, MAX_OWNED);
    assert!(oversized.truncated);
    let mut output = Output { bytes: [0; MAX_OUTPUT], len: 0, truncated: false };
    output.write_all(&[b'x'; MAX_OUTPUT]).unwrap();
    assert!(output.write_all(b"x").is_err());
    assert_eq!(output.len, MAX_OUTPUT);
}
