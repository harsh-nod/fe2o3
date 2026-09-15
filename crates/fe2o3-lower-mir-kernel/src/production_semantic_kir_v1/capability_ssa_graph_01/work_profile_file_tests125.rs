fn work_profile_file_names125() -> [&'static str; 34] {
    std::array::from_fn(|index| {
        Box::leak(format!("compiler/file-{index}.rs").into_boxed_str()) as &'static str
    })
}

#[test]
fn work_profile_files_keep_first32_and_aggregate_known_files_after_overflow() {
    assert_eq!(FILE_LIMIT, 32);
    let names = work_profile_file_names125();
    let mut files = Files::new();
    assert_eq!(
        (files.len, files.unclassified, files.accounted()),
        (0, 0, Some(0))
    );
    for (index, &file) in names.iter().enumerate() {
        files.record(index + 1, file);
    }
    assert_eq!((files.len, files.unclassified), (32, 33 + 34));
    assert_eq!(files.accounted(), Some(34 * 35 / 2));
    for (index, row) in files.rows.iter().enumerate() {
        let row = row.as_ref().unwrap();
        assert_eq!((row.file, row.accepted), (names[index], index + 1));
    }
    files.record(7, names[0]);
    files.record(11, names[31]);
    files.record(13, names[32]);
    assert_eq!((files.len, files.unclassified), (32, 33 + 34 + 13));
    assert_eq!(files.rows[0].as_ref().unwrap().accepted, 1 + 7);
    assert_eq!(files.rows[31].as_ref().unwrap().accepted, 32 + 11);
    assert_eq!(files.accounted(), Some(34 * 35 / 2 + 7 + 11 + 13));
}

#[test]
fn work_profile_files_compare_filename_contents_and_ignore_zero_debits() {
    let first = Box::leak(String::from("compiler/shared.rs").into_boxed_str()) as &'static str;
    let second = Box::leak(String::from("compiler/shared.rs").into_boxed_str()) as &'static str;
    assert_ne!(first.as_ptr(), second.as_ptr());
    let mut files = Files::new();
    files.record(0, "ignored.rs");
    assert_eq!(files.len, 0);
    files.record(3, first);
    files.record(4, second);
    files.record(0, "also-ignored.rs");
    assert_eq!(
        (files.len, files.unclassified, files.accounted()),
        (1, 0, Some(7))
    );
    let row = files.rows[0].as_ref().unwrap();
    assert_eq!((row.file, row.accepted), ("compiler/shared.rs", 7));
    assert!(files.rows[1..].iter().all(Option::is_none));
}

#[test]
fn work_profile_file_accounting_stays_complete_past_site65() {
    let sites = locations();
    assert_ne!(sites[0].line(), sites[64].line());
    assert!(sites.iter().all(|site| site.file() == sites[0].file()));
    let mut profile = Profile::new();
    for (index, &site) in sites.iter().enumerate() {
        profile.record(index + 1, site);
    }
    profile.record(7, sites[0]);
    profile.record(11, sites[64]);
    profile.record(0, here());
    let expected = 65 * 66 / 2 + 7 + 11;
    assert_eq!(
        (profile.len, profile.accepted, profile.unclassified),
        (64, expected, 76)
    );
    assert_eq!((profile.files.len, profile.files.unclassified), (1, 0));
    assert_eq!(profile.files.accounted(), Some(expected));
    let row = profile.files.rows[0].as_ref().unwrap();
    assert_eq!((row.file, row.accepted), (sites[0].file(), expected));
    let text = render(&profile, failure(expected, 0, usize::MAX));
    assert!(
        text.lines()
            .next()
            .unwrap()
            .contains("site_accounting_complete=false accounting_valid=true")
    );
    assert!(text.lines().any(|line| line == "capability-ssa-work-profile-files-v1 files=1 file_limit=32 unclassified=0 file_accounting_complete=true accounting_valid=true"));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("capability-ssa-work-profile-file-v1 "))
            .count(),
        1
    );
}

#[test]
fn work_profile_file_writer_bounds_rows_and_utf8_paths_before_end_record() {
    let paths: [&'static str; 34] = std::array::from_fn(|index| {
        let path = match index {
            0 => String::from("compiler/short.rs"),
            1 => "x".repeat(CALLER_BYTES),
            2 => format!("{}{}", "x".repeat(CALLER_BYTES - 2), '\u{e9}'),
            3 => format!("{}{}", "x".repeat(CALLER_BYTES - 1), '\u{e9}'),
            _ => format!("compiler/{index}/{}", "x".repeat(10_000)),
        };
        Box::leak(path.into_boxed_str()) as &'static str
    });
    let mut profile = Profile::new();
    profile.record(34, here());
    // Synthetic filenames exercise formatting while preserving both ledger totals.
    profile.files = Files::new();
    for &path in &paths {
        profile.files.record(1, path);
    }
    let text = render(&profile, failure(34, 0, 1));
    let lines: Vec<_> = text.lines().collect();
    assert!(lines[0].contains("accounting_valid=true"));
    let summary = lines
        .iter()
        .position(|line| line.starts_with("capability-ssa-work-profile-files-v1 "))
        .unwrap();
    assert_eq!(
        lines[summary],
        "capability-ssa-work-profile-files-v1 files=32 file_limit=32 unclassified=2 file_accounting_complete=false accounting_valid=true"
    );
    assert_eq!(lines.len(), summary + 1 + 32 + 1);
    for (index, path) in paths[..32].iter().enumerate() {
        let prefix =
            format!("capability-ssa-work-profile-file-v1 index={index} accepted=1 charge_file=");
        let suffix = format!(" file_truncated={}", path.len() > CALLER_BYTES);
        let shown = lines[summary + 1 + index]
            .strip_prefix(prefix.as_str())
            .unwrap()
            .strip_suffix(suffix.as_str())
            .unwrap();
        assert!(shown.len() <= CALLER_BYTES);
        let end = path.len().min(CALLER_BYTES);
        if let Some(expected) = path.get(..end) {
            assert_eq!(shown, expected);
        } else {
            assert_eq!(shown, "<utf8-boundary>");
        }
    }
    assert_eq!(
        lines.last().copied(),
        Some("capability-ssa-work-profile-end-v1 record_complete=1")
    );
    assert!(text.len() < 32_768);
}

#[test]
fn work_profile_corrupt_file_ledger_invalidates_header_and_file_summary() {
    let mut profile = Profile::new();
    profile.record(7, here());
    for overflow in [false, true] {
        // Corruption is injected directly, never through unchecked record calls.
        profile.files.rows[0].as_mut().unwrap().accepted = if overflow { usize::MAX } else { 8 };
        profile.files.unclassified = usize::from(overflow);
        assert_eq!(
            profile.files.accounted(),
            if overflow { None } else { Some(8) }
        );
        let text = render(&profile, failure(7, 0, 1));
        assert!(
            text.lines()
                .next()
                .unwrap()
                .contains("accounting_valid=false")
        );
        let summary = text
            .lines()
            .find(|line| line.starts_with("capability-ssa-work-profile-files-v1 "))
            .unwrap();
        assert!(summary.ends_with("accounting_valid=false"));
        assert!(!text.contains("accounting_valid=true"));
        assert_eq!(profile.accepted, 7);
        assert_eq!(profile.rows[0].as_ref().unwrap().accepted, 7);
        assert!(text.ends_with("capability-ssa-work-profile-end-v1 record_complete=1\n"));
        assert!(text.len() < 32_768);
    }
}

#[test]
fn work_profile_files_retain_maximum_safe_row_and_overflow_counters() {
    let names = work_profile_file_names125();
    for target in [0, 32] {
        let mut files = Files::new();
        for &file in &names[..32] {
            files.record(1, file);
        }
        files.record(usize::MAX - 32, names[target]);
        files.record(0, names[33]);
        assert_eq!(files.len, 32);
        assert_eq!(files.accounted(), Some(usize::MAX));
        assert_eq!(
            files.rows[0].as_ref().unwrap().accepted,
            if target == 0 { usize::MAX - 31 } else { 1 }
        );
        assert_eq!(
            files.unclassified,
            if target == 32 { usize::MAX - 32 } else { 0 }
        );
    }
    let mut profile = Profile::new();
    let site = here();
    profile.record(usize::MAX - 1, site);
    profile.record(1, site);
    profile.record(0, here());
    assert_eq!(profile.files.rows[0].as_ref().unwrap().accepted, usize::MAX);
    assert_eq!(profile.files.accounted(), Some(usize::MAX));
    let text = render(&profile, failure(usize::MAX, 0, 1));
    assert!(
        text.lines()
            .next()
            .unwrap()
            .contains("accounting_valid=true")
    );
    assert!(text.lines().any(|line| line == "capability-ssa-work-profile-files-v1 files=1 file_limit=32 unclassified=0 file_accounting_complete=true accounting_valid=true"));
    profile.record(1, site);
    assert!(!profile.valid);
    assert_eq!(profile.accepted, usize::MAX);
    assert_eq!(profile.files.rows[0].as_ref().unwrap().accepted, usize::MAX);
    assert_eq!(profile.files.accounted(), Some(usize::MAX));
}
