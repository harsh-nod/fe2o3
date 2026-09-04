#[test]
fn worker_v3_verification_typestate_is_closed() {
    let cases = trybuild::TestCases::new();
    let legacy_hsa = [
        "loaded_cannot_clone.rs",
        "loaded_launch_is_unavailable.rs",
        "prepared_cannot_clone.rs",
        "prepared_cannot_dispatch_twice.rs",
        "unload_while_prepared.rs",
    ];
    let default_build_only = [
        "compiler_execution_synthetic_constructor_is_unavailable.rs",
        "synthetic_verifier_is_unavailable.rs",
        "verifier_is_sealed.rs",
    ];
    for entry in std::fs::read_dir("tests/ui/worker_v3_verification_admission").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        if !cfg!(feature = "qualification-legacy-hip-hsa")
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| legacy_hsa.contains(&name))
        {
            continue;
        }
        if cfg!(feature = "worker-v3-verifier-test-support")
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| default_build_only.contains(&name))
        {
            continue;
        }
        cases.compile_fail(path);
    }
}
