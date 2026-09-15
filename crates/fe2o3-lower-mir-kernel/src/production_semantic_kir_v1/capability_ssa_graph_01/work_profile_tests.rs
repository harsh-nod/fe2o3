#[track_caller]
fn here() -> &'static Location<'static> {
    Location::caller()
}

fn locations() -> [&'static Location<'static>; 65] {
    [
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
        here(),
    ]
}

fn failure(limit: usize, remaining: usize, requested: usize) -> Failure<'static> {
    Failure {
        body: &[0x71; 32],
        limit,
        remaining,
        requested,
        blocks: 2,
        locals: 3,
        cache_rows: [0; 4],
    }
}

fn render(profile: &Profile, failure: Failure<'_>) -> String {
    let mut bytes = Vec::new();
    profile.write_to(&mut bytes, failure, here()).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn work_profile_flag_is_exact_and_snapshot_does_not_follow_later_changes() {
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("01"),
        Some("true"),
        Some("1 "),
    ] {
        assert!(!enabled_value(value.map(OsStr::new)));
    }
    assert!(enabled_value(Some(OsStr::new("1"))));
    with_enabled(false, || {
        assert!(Profile::configured().is_none());
        let mut profile = with_enabled(true, Profile::configured).unwrap();
        profile.record(3, here());
        assert_eq!(profile.accepted, 3);
        assert!(!enabled());
        assert!(std::panic::catch_unwind(|| with_enabled(true, || panic!("probe"))).is_err());
        assert!(!enabled());
    });
}

#[test]
fn work_profile_counts_repeated_sites_and_never_charges_the_rejected_request() {
    let mut profile = Profile::new();
    let first = here();
    let second = here();
    for amount in [3, 0, 4] {
        profile.record(amount, first);
    }
    profile.record(5, second);
    assert_eq!(
        (profile.len, profile.accepted, profile.unclassified),
        (2, 12, 0)
    );
    assert_eq!(profile.rows[0].unwrap().accepted, 7);
    assert_eq!(profile.rows[1].unwrap().accepted, 5);
    for requested in [9, usize::MAX] {
        let text = render(&profile, failure(20, 8, requested));
        assert!(text.contains("accepted=12 unclassified=0 sites=2"));
        assert!(text.contains("site_accounting_complete=true accounting_valid=true"));
        assert!(text.ends_with("capability-ssa-work-profile-end-v1 record_complete=1\n"));
        assert_eq!(text.lines().count(), 6);
    }
    profile.record(8, first);
    assert_eq!(profile.accepted, 20);
    assert!(render(&profile, failure(20, 0, 1)).contains("accounting_valid=true"));
    // External test-only budget mutation must be visible, never repaired.
    assert!(render(&profile, failure(20, 1, 2)).contains("accounting_valid=false"));
}

#[test]
fn work_profile_overflow_sites_keep_every_debit_and_known_rows_keep_accumulating() {
    let sites = locations();
    for (index, site) in sites.iter().enumerate() {
        assert!(sites[..index].iter().all(|old| *old != *site));
    }
    let mut profile = Profile::new();
    for (index, site) in sites.iter().enumerate() {
        profile.record(index + 1, site);
    }
    profile.record(7, sites[0]);
    profile.record(11, sites[64]);
    profile.record(0, here());
    assert_eq!(profile.len, 64);
    assert_eq!(profile.rows[0].unwrap().accepted, 8);
    assert_eq!(profile.unclassified, 76);
    assert_eq!(profile.accepted, 65 * 66 / 2 + 18);
    let text = render(&profile, failure(profile.accepted, 0, 1));
    assert!(text.contains("site_accounting_complete=false accounting_valid=true"));
    assert_eq!(text.lines().count(), 68);
    assert!(text.len() < 32_768);
    // Bound the formatter even for an inconsistent maximal synthetic ledger.
    for row in profile.rows.iter_mut().flatten() {
        row.accepted = usize::MAX;
    }
    profile.accepted = usize::MAX;
    profile.unclassified = usize::MAX;
    let text = render(&profile, failure(usize::MAX, usize::MAX, usize::MAX));
    assert!(text.contains("accounting_valid=false"));
    assert!(text.len() < 32_768);
}

#[test]
fn work_profile_maximum_arithmetic_and_output_errors_never_repair_the_budget() {
    let mut profile = Profile::new();
    profile.record(usize::MAX, here());
    profile.record(0, here());
    assert!(render(&profile, failure(usize::MAX, 0, usize::MAX)).contains("accounting_valid=true"));
    profile.record(1, here());
    assert!(!profile.valid);
    assert_eq!(profile.accepted, usize::MAX);
    assert!(render(&profile, failure(usize::MAX, 0, 1)).contains("accounting_valid=false"));
    for file in [
        "x".repeat(10_000),
        format!("{}{}", "x".repeat(159), '\u{e9}'),
    ] {
        let mut bytes = Vec::new();
        write_caller(&mut bytes, &file, u32::MAX, u32::MAX).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.ends_with("caller_truncated=true\n"));
        assert!(text.len() < 256);
        assert!(!text.contains(&"x".repeat(161)));
    }
    let mut short = &mut [0u8; 8][..];
    assert_eq!(
        profile
            .write_to(&mut short, failure(1, 0, 2), here())
            .unwrap_err()
            .kind(),
        io::ErrorKind::WriteZero
    );
}
