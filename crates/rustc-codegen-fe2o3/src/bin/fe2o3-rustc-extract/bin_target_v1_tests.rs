struct BinCompileFiles(PathBuf);
impl BinCompileFiles {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("fe2o3-bin-compile-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        for file in ["main.rs", "lib.rs"] { std::fs::write(root.join(file), b"").unwrap(); }
        Self(root)
    }
    fn args(&self, source: &str, kind: &str) -> Vec<OsString> {
        let mut args = compile_argv("selected_bin", &["actual"]);
        args[4] = self.0.join(source).into_os_string();
        args.extend(["--crate-type", kind].map(OsString::from));
        args
    }
    fn request(&self) -> OsString {
        cargo_target_selection_v1::Selection::bin("selected-bin".into(),
            self.0.join("main.rs"), "selected_bin").unwrap().encode().unwrap()
    }
}
impl Drop for BinCompileFiles {
    fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); }
}

#[test]
fn exact_bin_prepare_captures_only_actual_selected_compile_and_preserves_bindings() {
    let files = BinCompileFiles::new();
    let prepare_target = |args, selected| prepare_for_target(
        args, Some("selected_bin".into()), None, None, None, None,
        Some("fresh.fe2sim".into()), None, Some(package_identity("1.0.0", 1)), selected,
    );
    for (source, kind) in [("lib.rs", "lib"), ("main.rs", "lib"), ("lib.rs", "bin")] {
        let args = files.args(source, kind);
        let original = args[2..].to_vec();
        let PreparedExtractionV1::Passthrough { executable, forwarded_args } =
            prepare_target(args, Some(files.request())).unwrap()
        else { panic!("nonselected Cargo target was captured"); };
        assert_eq!(executable, OsString::from("rustc"));
        assert_eq!(forwarded_args, original);
    }
    let PreparedExtractionV1::Selected(actual) =
        prepare_target(files.args("main.rs", "bin"), Some(files.request())).unwrap()
    else { panic!("exact selected bin was not captured"); };
    let PreparedExtractionV1::Selected(legacy) =
        prepare_target(files.args("main.rs", "bin"), None).unwrap()
    else { panic!("legacy compile was not captured"); };
    assert_eq!(actual.args, legacy.args);
    assert_eq!(actual.crate_binding, legacy.crate_binding);
    assert_eq!(actual.metadata_observation, legacy.metadata_observation);
}

#[test]
fn malformed_bin_request_is_never_downgraded_to_crate_only_capture() {
    let files = BinCompileFiles::new();
    for request in ["", "not-json", "{}"] {
        assert!(prepare_for_target(
            files.args("main.rs", "bin"), Some("selected_bin".into()), None, None, None,
            None, Some("fresh.fe2sim".into()), None, Some(package_identity("1.0.0", 1)),
            Some(request.into()),
        ).is_err());
    }
    assert!(prepare_for_target(
        files.args("main.rs", "bin"), None, None, None, None, None,
        Some("fresh.fe2sim".into()), None, Some(package_identity("1.0.0", 1)),
        Some(files.request()),
    ).is_err());
    let command = passthrough_command("rustc".into(), vec![]);
    assert!(command.get_envs().any(|(name, value)|
        name == cargo_target_selection_v1::ENV && value.is_none()));
}
