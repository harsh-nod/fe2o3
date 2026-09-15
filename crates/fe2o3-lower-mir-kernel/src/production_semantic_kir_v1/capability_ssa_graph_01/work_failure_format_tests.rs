#[test]
fn default_off_accepts_only_the_existing_explicit_flag() {
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some("01"),
        Some("1 "),
    ] {
        assert!(!enabled_value(value.map(OsStr::new)));
    }
    assert!(enabled_value(Some(OsStr::new("1"))));
    with_enabled(false, || {
        assert!(!enabled());
        with_enabled(true, || assert!(enabled()));
        assert!(!enabled());
    });
}

#[test]
fn exact_debit_and_cache_snapshot_has_no_invented_source_site() {
    let mut bytes = Vec::new();
    write_observation(
        &mut bytes,
        Failure {
            body: &[0x62; 32],
            remaining: 3,
            requested: 4,
            limit: 1_016_002,
            blocks: 1000,
            locals: 2000,
            cache_rows: [11, 12, 13, 14],
        },
        "capability_ssa_graph_01/graph.rs",
        147,
        18,
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(&format!(
        "capability-ssa-analysis-work body={} ",
        "62".repeat(32)
    )));
    assert!(text.contains("remaining=3 requested=4 limit=1016002 blocks=1000 locals=2000"));
    assert!(
        text.contains("cache_uses=11 cache_definitions=12 cache_reachability=13 cache_loans=14")
    );
    assert!(text.ends_with(
        "charge_caller=capability_ssa_graph_01/graph.rs:147:18 caller_truncated=false\n"
    ));
    assert!(!text.contains("statement="));
    assert_eq!(text.lines().count(), 1);
    assert!(text.len() < 1024);
}

#[test]
fn output_is_bounded_for_maximum_fields_and_failed_writer() {
    let failure = Failure {
        body: &[0xff; 32],
        remaining: usize::MAX,
        requested: usize::MAX,
        limit: usize::MAX,
        blocks: usize::MAX,
        locals: usize::MAX,
        cache_rows: [usize::MAX; 4],
    };
    for file in [
        "x".repeat(10_000),
        format!("{}{}", "x".repeat(159), '\u{e9}'),
    ] {
        let mut bytes = Vec::new();
        write_observation(&mut bytes, failure, &file, u32::MAX, u32::MAX).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.len() < 1024);
        assert!(text.ends_with("caller_truncated=true\n"));
        assert!(!text.contains(&"x".repeat(161)));
    }
    let mut short = &mut [0u8; 8][..];
    assert_eq!(
        write_observation(&mut short, failure, "caller", 0, 0)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WriteZero
    );
}

#[test]
fn test_override_restores_on_unwind_without_environment_mutation() {
    with_enabled(false, || {
        let result = std::panic::catch_unwind(|| with_enabled(true, || panic!("probe")));
        assert!(result.is_err());
        assert!(!enabled());
    });
}
