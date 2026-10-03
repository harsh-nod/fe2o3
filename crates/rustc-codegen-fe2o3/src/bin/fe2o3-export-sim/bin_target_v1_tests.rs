struct BinOptionFiles(PathBuf);
impl BinOptionFiles {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("fe2o3-bin-options-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("main.rs"), b"").unwrap();
        Self(root)
    }
    fn args(&self) -> Vec<OsString> {
        ["--crate", "selected_bin", "--output", "result.fe2sim",
            "--bin-name", "selected-bin", "--bin-source", "main.rs"]
            .map(OsString::from).into()
    }
}
impl Drop for BinOptionFiles {
    fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); }
}

#[test]
fn explicit_bin_options_bind_one_source_without_changing_legacy_selection() {
    let files = BinOptionFiles::new();
    let options = parse(files.args(), &files.0).unwrap();
    assert_eq!(options.selected_target.as_ref().unwrap().name(), "selected-bin");
    assert!(options.cargo_args.is_empty());
    let mut legacy = files.args();
    legacy.truncate(4);
    legacy.extend(["--", "--lib"].map(OsString::from));
    let options = parse(legacy, &files.0).unwrap();
    assert!(options.selected_target.is_none());
    assert_eq!(options.cargo_args, ["--lib"].map(OsString::from));
    assert!(conflicting_extraction_environment().contains(&cargo_target_selection_v1::ENV));
}

#[test]
fn explicit_bin_options_reject_partial_duplicate_or_mixed_selection() {
    let files = BinOptionFiles::new();
    for tail in [
        vec!["--bin-name", "selected-bin"],
        vec!["--bin-source", "main.rs"],
        vec!["--bin-name", "selected-bin", "--bin-name", "selected-bin", "--bin-source", "main.rs"],
        vec!["--bin-name", "wrong", "--bin-source", "main.rs"],
        vec!["--bin-name", "selected-bin", "--bin-source", "missing.rs"],
    ] {
        let mut args = files.args();
        args.truncate(4);
        args.extend(tail.into_iter().map(OsString::from));
        assert!(parse(args, &files.0).is_err());
    }
    for selector in ["--lib", "--bin=other", "--bins", "--test=x", "--tests",
        "--example=x", "--examples", "--bench=x", "--benches", "--all-targets"]
    {
        let mut args = files.args();
        args.extend(["--", selector].map(OsString::from));
        assert!(parse(args, &files.0).is_err(), "{selector}");
    }
}
