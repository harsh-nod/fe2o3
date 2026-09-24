//! Public library-driver action qualification, not extractor-binary selection.
//! Separate actual packages/callbacks; all request identities are inert selectors.
use super::*;
use std::os::unix::fs::MetadataExt;

#[path = "gfx942_ordered_composition_public_promotion_checks_v1_tests.rs"]
mod checks;

const OUTPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLIC_PROMOTION_OUTPUT_V1";
const INPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLIC_PROMOTION_INPUT_V1";
const CASE_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLIC_PROMOTION_CASE_V1";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_composition_qualification_v1_tests::publisher::public_action::actual_ordered_composition_public_promotion_child";
const PREFIX: &str = "FE2O3_ORDERED_COMPOSITION_PUBLIC_PROMOTION_OBSERVATION_V1 ";
const CASES: [&str; 17] = [
    "diagnostic",
    "copy",
    "recompile-copy",
    "preserve",
    "recompile-preserve",
    "edit",
    "recompile-edit",
    "stale-semantic",
    "stale-canonical",
    "stale-source",
    "unknown-json",
    "unknown-instruction",
    "input-destination",
    "register-overlap",
    "reused-output",
    "reused-candidate",
    "missing-request",
];
fn derive_public(root: &Path, case: &str) -> Invocation {
    assert!(CASES.contains(&case));
    super::derive_package(
        root,
        case,
        checks::package(case),
        "ordered-composition-publish-direct",
    )
}
fn output(root: &Path, case: &str) -> PathBuf {
    if matches!(case, "diagnostic" | "reused-output") {
        root.join("diagnostic-initial")
    } else {
        root.join(format!("public-{case}"))
    }
}
fn recompile(case: &str) -> bool {
    matches!(
        case,
        "recompile-copy" | "recompile-preserve" | "recompile-edit"
    )
}
struct FreshCallbacks<'a> {
    case: &'a str,
    root: &'a Path,
    started: std::time::Instant,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for FreshCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = super::super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let target = transaction.lower_ordered_composition_diagnostic_v1()?;
            let observation_case = if self.case == "recompile-edit" {
                "recompile-edit"
            } else {
                "recompile-copy"
            };
            super::publication::observe(target, observation_case, self.root, self.started)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated public promotion child; invoke only the preparation parent"]
fn actual_ordered_composition_public_promotion_child() {
    let started = std::time::Instant::now();
    let root =
        PathBuf::from(std::env::var_os(INPUT_ENV).expect("fresh public promotion preparation"));
    assert!(root.is_absolute());
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    let case = std::env::var(CASE_ENV).expect("closed public case");
    let actual = derive_public(&root, &case);
    let retained: Invocation = serde_json::from_slice(
        &read_bounded(&root.join(format!("{case}.invocation.json")), 256 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained);
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", staging::PACKAGE_NAME),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", staging::LIB_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        root.join(&actual.package)
    );
    super::super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let original = read_bounded(&root.join(checks::ORIGINAL), 64 * 1024).unwrap();
    timely(started.elapsed(), 300).unwrap();
    let result = if recompile(&case) {
        let mut callbacks = FreshCallbacks {
            case: &case,
            root: &root,
            started,
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&actual.args, &mut callbacks);
        assert_eq!(callbacks.calls, 1);
        let observed = callbacks.result.expect("fresh promoted callback").unwrap();
        assert_eq!(observed["cpu"]["cases"], 32);
        json!({"stage":"fresh_promoted_source_cpu","actual_callback_count":1,"observation":observed})
    } else if case == "diagnostic" {
        let out = output(&root, &case);
        super::super::super::run_diagnostic_ordered_composition_extraction_driver_v1(
            &actual.args,
            &out,
        )
        .unwrap();
        let observed = checks::read_report(&out);
        assert!(observed["source_promotion"].is_null());
        assert_eq!(checks::diagnostic_files(&out).len(), 3);
        for name in [
            "package-promoted-copy",
            "package-promoted-preserve",
            "package-promoted-edit",
        ] {
            assert!(!root.join(name).join(staging::LEAF).exists());
        }
        json!({"stage":"public_diagnostic_without_action","public_report":observed,
            "actual_callback_report_present":true,"source_publication_requested":false})
    } else {
        let initial = checks::read_report(&root.join("diagnostic-initial"));
        let out = output(&root, &case);
        let request = root.join(format!("{case}.request.json"));
        let request_bytes = if case == "missing-request" {
            assert!(!request.exists());
            None
        } else {
            let bytes = read_bounded(&request, 8192).unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                checks::request(&case, &initial, &digest(&original))
            );
            Some(bytes)
        };
        let old_output = (case == "reused-output").then(|| checks::diagnostic_files(&out));
        let candidate = root.join(checks::candidate(&case));
        let old_candidate = (case == "reused-candidate").then(|| {
            let bytes = read_bounded(&candidate, 72 * 1024).unwrap();
            let meta = fs::symlink_metadata(&candidate).unwrap();
            (bytes, meta.dev(), meta.ino())
        });
        if old_candidate.is_none() {
            assert!(!candidate.exists());
        }
        timely(started.elapsed(), 300).unwrap();
        let action = super::super::super::run_ordered_composition_source_promotion_driver_v1(
            &actual.args,
            &out,
            &request,
        );
        // Preserve failure facts before matching the exact boundary. A refusal
        // does not erase existing candidates or partially written diagnostics.
        if let Err(error) = &action {
            assert!(error.len() <= 64 * 1024);
            publish_json(
                &root,
                &format!("{case}.public-refusal.json"),
                &json!({
                    "schema":"fe2o3-test-public-promotion-refusal-v1","case":case,"diagnostic":error,
                    "candidate_currently_exists":candidate.exists(),"diagnostic_directory_exists":out.exists(),
                    "acceptance":false,"no_new_creation_inferred_from_existing_path":true,
                }),
            );
        }
        let observed = if matches!(case.as_str(), "copy" | "preserve" | "edit") {
            action.unwrap();
            let report = checks::published(
                &case,
                &root,
                &out,
                &initial,
                request_bytes.as_ref().unwrap(),
            );
            json!({"stage":"public_source_promotion","public_report":report,"actual_callback_report_present":true})
        } else {
            let error = action.expect_err("invalid public request unexpectedly succeeded");
            assert!(
                error.contains(checks::refusal(&case)),
                "wrong public boundary: {error}"
            );
            if checks::pre_frontend(&case) {
                assert!(!out.exists() && !candidate.exists());
            } else if case == "reused-output" {
                assert_eq!(checks::diagnostic_files(&out), old_output.unwrap());
                assert!(!candidate.exists());
            } else {
                checks::unchanged_original_owner(
                    &out,
                    &root.join("diagnostic-initial"),
                    &initial,
                    false,
                );
                if let Some((bytes, device, inode)) = old_candidate {
                    assert!(error.starts_with("MayHaveCreatedCandidate:"));
                    assert!(error.contains("publisher create-new publication refused"));
                    let current = fs::symlink_metadata(&candidate).unwrap();
                    assert_eq!(read_bounded(&candidate, 72 * 1024).unwrap(), bytes);
                    assert_eq!((current.dev(), current.ino()), (device, inode));
                } else {
                    assert!(error.starts_with("NotAttempted:"));
                    assert!(!candidate.exists());
                }
            }
            json!({"stage":"exact_public_promotion_refused","diagnostic":error,
                "pre_frontend_refusal":checks::pre_frontend(&case),
                "new_callback_report_present":false,
                "partial_original_owner_diagnostics_retained":!checks::pre_frontend(&case)&&case!="reused-output",
                "existing_candidate_preserved":case=="reused-candidate",
                "new_candidate_creation_claimed":false})
        };
        if let Some(bytes) = request_bytes {
            assert_eq!(read_bounded(&request, 8192).unwrap(), bytes);
        }
        observed
    };
    assert_eq!(
        read_bounded(&root.join(checks::ORIGINAL), 64 * 1024).unwrap(),
        original
    );
    assert_eq!(derive_public(&root, &case), actual);
    assert!(
        fs::read_dir(preparation(&root, &actual.package).join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    timely(started.elapsed(), 300).unwrap();
    let frame = serde_json::to_string(&json!({
        "schema":"fe2o3-test-ordered-composition-public-promotion-observation-v1",
        "case":case,"invocation":actual,"result":result,"original_source_unchanged":true,
        "public_library_driver":!recompile(&case),"extractor_binary_invoked":false,
        "normal_checked_handoff_qualified":false,"native_execution":false,"launch_authority":false,
    }))
    .unwrap();
    assert!(frame.len() <= 256 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "root-owned standalone public action ladder; baseline publisher must qualify first"]
fn actual_ordered_composition_public_promotion_ladder() {
    let started = std::time::Instant::now();
    let root = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh public action output"));
    assert!(root.is_absolute());
    fs::create_dir(&root).unwrap();
    assert!(
        !root
            .canonicalize()
            .unwrap()
            .starts_with(repository().parent().unwrap())
    );
    let source_before = staging::fixture_source();
    let provider_before = super::super::inputs::current_sources();
    let workspace_before = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    prepare_public_staging(&root, started);
    let deps = preparation(&root, "package-original");
    let (dependency_before, files) = super::super::inputs::dependency_snapshot(&deps);
    publish_json(&root, "dependency-files.json", &files);
    drop(files);
    let mut observations = Vec::new();
    for case in CASES {
        let session = std::time::Instant::now();
        let record = derive_public(&root, case);
        assert_eq!(record.provider_sources, provider_before);
        assert_eq!(record.dependency_snapshot, dependency_before);
        assert_eq!(record.workspace_lock_sha256, digest(&workspace_before));
        timely(started.elapsed(), 1200).unwrap();
        timely(session.elapsed(), 300).unwrap();
        publish_json(&root, &format!("{case}.invocation.json"), &record);
        if case != "diagnostic" && !recompile(case) && case != "missing-request" {
            let initial = checks::read_report(&root.join("diagnostic-initial"));
            let request = checks::request(case, &initial, &digest(&source_before));
            let bytes = serde_json::to_vec(&request).unwrap();
            assert!(bytes.len() <= 8192);
            create(&root.join(format!("{case}.request.json")), &bytes, 8192);
        }
        timely(started.elapsed(), 1200).unwrap();
        timely(session.elapsed(), 300).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(&root)
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(INPUT_ENV, &root)
                .env(CASE_ENV, case)
                .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &record.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", root.join(&record.package))
                .env("CARGO_PKG_NAME", staging::PACKAGE_NAME)
                .env("CARGO_PKG_VERSION", "0.1.0")
                .env("CARGO_CRATE_NAME", staging::LIB_NAME),
            &root,
            case,
            None,
        );
        timely(session.elapsed(), 300).unwrap();
        let stdout = std::str::from_utf8(&stdout).unwrap();
        let frames = stdout
            .lines()
            .filter_map(|l| l.strip_prefix(PREFIX))
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 1);
        assert_eq!(
            stdout
                .lines()
                .filter(|l| l.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observation: Value = serde_json::from_str(frames[0]).unwrap();
        assert_eq!(
            observation["schema"],
            "fe2o3-test-ordered-composition-public-promotion-observation-v1"
        );
        assert_eq!(observation["case"], case);
        assert_eq!(
            observation["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observation["original_source_unchanged"], true);
        assert_eq!(observation["extractor_binary_invoked"], false);
        assert_eq!(derive_public(&root, case), record);
        if recompile(case) {
            let prior = case.strip_prefix("recompile-").unwrap();
            let action = observations
                .iter()
                .find(|v: &&Value| v["case"] == prior)
                .unwrap();
            assert_eq!(
                observation["invocation"]["leaf_sha256"],
                action["result"]["public_report"]["source_promotion"]["candidate_sha256"]
            );
            assert_eq!(observation["result"]["actual_callback_count"], 1);
            assert_eq!(observation["result"]["observation"]["cpu"]["cases"], 32);
        }
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&root, &format!("{case}.accepted.json"), &observation);
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observation);
    }
    assert_eq!(observations.len(), 17);
    assert_eq!(staging::fixture_source(), source_before);
    assert_eq!(super::super::inputs::current_sources(), provider_before);
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        workspace_before
    );
    assert_eq!(
        super::super::inputs::dependency_snapshot(&deps).0,
        dependency_before
    );
    let cpu_cases: u64 = observations
        .iter()
        .filter_map(|v| v["result"]["observation"]["cpu"]["cases"].as_u64())
        .sum();
    assert_eq!(cpu_cases, 96);
    let refusals = observations
        .iter()
        .filter(|v| v["result"]["stage"] == "exact_public_promotion_refused")
        .count();
    assert_eq!(refusals, 10);
    let early = observations
        .iter()
        .filter(|v| v["result"]["pre_frontend_refusal"] == true)
        .count();
    assert_eq!(early, 5);
    let reports = observations
        .iter()
        .filter(|v| v["result"]["actual_callback_report_present"] == true)
        .count();
    assert_eq!(reports, 4);
    let report = json!({
        "schema":"fe2o3-test-ordered-composition-public-promotion-ladder-v1","observations":observations,
        "isolated_children":17,"public_driver_invocations":14,"successful_public_callback_reports":reports,
        "custom_fresh_recompile_callbacks":3,"cpu_cases":cpu_cases,"exact_public_refusals":refusals,
        "pre_frontend_refusals":early,"failed_driver_callback_count_claimed":false,
        "actual_source_publications":3,"fresh_standalone_metadata_packages":4,"fresh_dependency_builds":1,
        "copy_omitted_edit":true,"typed_preserving_edit":true,"typed_changing_edit":true,
        "extractor_binary_invoked":false,"source_custody_from_files":false,
        "normal_checked_handoff_qualified":false,"native_execution":false,"launch_authority":false,
        "acceptance":"requires completed successful parent/runner; JSON alone is historical",
        "cleanup_scope":"bounded direct child/process group; no whole-family or isolation claim",
    });
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&root, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn public_case_roster_separates_parse_driver_and_recompile_domains() {
    assert_eq!(CASES.len(), 17);
    assert_eq!(CASES.iter().filter(|v| recompile(v)).count(), 3);
    assert_eq!(CASES.iter().filter(|v| checks::pre_frontend(v)).count(), 5);
    assert_ne!(
        checks::package("recompile-copy"),
        checks::package("recompile-preserve")
    );
}

/// Same fixed standalone preparation used by the separate binary qualifier.
fn prepare_public_staging(root: &Path, started: std::time::Instant) {
    let compiler = PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned compiler"));
    assert!(compiler.is_absolute());
    let mut command = Command::new(compiler);
    let bytes = checked(
        sanitized(&mut command).args(["--print", "sysroot"]),
        &root,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&bytes).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    for package in [
        "package-original",
        "package-promoted-copy",
        "package-promoted-preserve",
        "package-promoted-edit",
    ] {
        staging::package(&root, package, package == "package-original");
        staging::prepare(&root, package, &sysroot, started);
        fs::create_dir(preparation(&root, package).join("analysis-output")).unwrap();
    }
    let deps = preparation(&root, "package-original");
    let target = deps.join("dependencies");
    let mut command = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut command).current_dir(root.join("package-original")).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(root.join("package-original/Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &deps,"dependencies",Some(&target));
}

#[path = "gfx942_ordered_composition_cli_promotion_v1_tests.rs"]
mod cli;
