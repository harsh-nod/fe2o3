//! Actual extractor process/selector qualification; no inferred library->binary success.
use super::super::super::super::gfx942_inline_value_qualification_v30_tests::capture_cli_status_v1;
use super::*;
#[path = "gfx942_ordered_composition_cli_pins_v1_tests.rs"]
mod pins;
const OUTPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_CLI_PROMOTION_OUTPUT_V1";
const SELECTORS: [&str; 3] = [
    "empty-request-selector",
    "wrong-mode-selector",
    "conflicting-selector",
];
const DIAGNOSTIC_ENV: &str = "FE2O3_EXTRACT_DIAGNOSTIC_ORDERED_COMPOSITION_DIRECTORY_V1";
const REQUEST_ENV: &str = "FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1";
fn selector_refusal(case: &str) -> &'static str {
    match case {
        "empty-request-selector" => {
            "FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1 must not be empty"
        }
        "wrong-mode-selector" => {
            "FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1 requires the explicit ordered-composition diagnostic mode"
        }
        "conflicting-selector" => {
            "ordered composition diagnostics are mutually exclusive with V16/V17/V19/V20/V21/V22"
        }
        _ => panic!("unknown binary-only selector control"),
    }
}
fn common_environment(command: &mut Command, root: &Path, record: &Invocation) {
    // Cargo adds its own loader paths to this parent. The actual CLI child
    // gets only the fixed, rechecked backend directory and pinned toolchain.
    sanitized(command)
        .env("LD_LIBRARY_PATH", pins::runtime_library_path())
        .current_dir(root)
        .env("CARGO_MANIFEST_DIR", root.join(&record.package))
        .env("CARGO_PKG_NAME", staging::PACKAGE_NAME)
        .env("CARGO_PKG_VERSION", "0.1.0")
        .env("CARGO_CRATE_NAME", staging::LIB_NAME)
        .env("CARGO_PRIMARY_PACKAGE", "1");
}
fn actual_binary(
    root: &Path,
    case: &str,
    record: &Invocation,
    build: &pins::Build,
    started: std::time::Instant,
) -> Value {
    let session = std::time::Instant::now();
    let selector = SELECTORS.contains(&case);
    let out = if selector {
        root.join(format!("selector-{case}"))
    } else {
        super::output(root, case)
    };
    let request = root.join(format!("{case}.request.json"));
    let original = read_bounded(&root.join(checks::ORIGINAL), 64 * 1024).unwrap();
    let initial =
        (case != "diagnostic").then(|| checks::read_report(&root.join("diagnostic-initial")));
    let source_snapshot = ["copy", "preserve", "edit"]
        .into_iter()
        .map(|c| {
            let path = root.join(checks::candidate(c));
            (
                path.clone(),
                path.exists().then(|| {
                    let m = fs::symlink_metadata(&path).unwrap();
                    (read_bounded(&path, 72 * 1024).unwrap(), m.dev(), m.ino())
                }),
            )
        })
        .collect::<Vec<_>>();
    let request_bytes = if case == "diagnostic" || case == "missing-request" || selector {
        assert!(!request.exists());
        None
    } else {
        let value = checks::request(case, initial.as_ref().unwrap(), &digest(&original));
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(bytes.len() <= 8192);
        create(&request, &bytes, 8192);
        Some(bytes)
    };
    let old_output = (case == "reused-output").then(|| checks::diagnostic_files(&out));
    let candidate = (!selector && case != "diagnostic").then(|| root.join(checks::candidate(case)));
    let old_candidate = (case == "reused-candidate").then(|| {
        let path = candidate.as_ref().unwrap();
        let m = fs::symlink_metadata(path).unwrap();
        (read_bounded(path, 72 * 1024).unwrap(), m.dev(), m.ino())
    });
    if let Some(path) = &candidate {
        if old_candidate.is_none() {
            assert!(!path.exists());
        }
    }
    build.recheck();
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 1200).unwrap();
    let mut command = Command::new(pins::extractor());
    common_environment(&mut command, root, record);
    command
        .args(&record.args)
        .env("FE2O3_EXTRACT_CRATE_V1", staging::LIB_NAME);
    // The binary computes/installs its own bindings from these actual arguments
    // and Cargo package identity. We do not inject previously retained identities.
    command
        .env_remove(CRATE_BINDING_ID_ENV_V1)
        .env_remove(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2);
    if case != "wrong-mode-selector" {
        command.env(DIAGNOSTIC_ENV, &out);
    }
    if case == "empty-request-selector" {
        command.env(REQUEST_ENV, "");
    } else if case == "wrong-mode-selector" {
        command
            .env(REQUEST_ENV, &request)
            .env("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1", &out);
    } else if case == "conflicting-selector" {
        command.env(REQUEST_ENV, &request).env(
            "FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V17",
            root.join("conflicting-v17.bin"),
        );
    } else if case != "diagnostic" {
        command.env(REQUEST_ENV, &request);
    }
    let (status, stdout, stderr) =
        capture_cli_status_v1(&mut command, root, &format!("cli-{case}")).unwrap();
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 1200).unwrap();
    let text = std::str::from_utf8(&stderr).unwrap();
    let success = matches!(case, "diagnostic" | "copy" | "preserve" | "edit");
    // Record real terminal/stream evidence before any semantic oracle can fail.
    publish_json(
        root,
        &format!("cli-{case}.terminal.json"),
        &json!({
            "case":case,"exit_code":status,"stdout_bytes":stdout.len(),"stdout_sha256":digest(&stdout),
            "stderr_bytes":stderr.len(),"stderr_sha256":digest(&stderr),
            "diagnostic_directory_exists":out.exists(),"candidate_exists":candidate.as_ref().is_some_and(|p|p.exists()),
            "acceptance":false,"native_effects":"no native worker or GPU path selected",
        }),
    );
    assert_eq!(
        status,
        Some(if success { 0 } else { 1 }),
        "unexpected CLI terminal result"
    );
    let result = if selector {
        assert!(
            text.contains(selector_refusal(case)),
            "wrong selector refusal: {text}"
        );
        assert!(!out.exists() && !root.join("conflicting-v17.bin").exists());
        assert!(!request.exists());
        for (path, before) in &source_snapshot {
            if let Some((bytes, device, inode)) = before {
                let m = fs::symlink_metadata(path).unwrap();
                assert_eq!(read_bounded(path, 72 * 1024).unwrap(), *bytes);
                assert_eq!((m.dev(), m.ino()), (*device, *inode));
            } else {
                assert!(!path.exists());
            }
        }
        json!({"stage":"exact_binary_selector_refused","refusal":selector_refusal(case),"pre_frontend_refusal":true})
    } else if case == "diagnostic" {
        let report = checks::read_report(&out);
        assert!(report["source_promotion"].is_null());
        assert_eq!(checks::diagnostic_files(&out).len(), 3);
        assert!(source_snapshot.iter().all(|(p, _)| !p.exists()));
        json!({"stage":"binary_diagnostic_without_action","public_report":report,"actual_callback_report_present":true})
    } else if success {
        let report = checks::published(
            case,
            root,
            &out,
            initial.as_ref().unwrap(),
            request_bytes.as_ref().unwrap(),
        );
        json!({"stage":"binary_source_promotion","public_report":report,"actual_callback_report_present":true})
    } else {
        assert!(
            text.contains(checks::refusal(case)),
            "wrong public binary refusal: {text}"
        );
        let candidate = candidate.as_ref().unwrap();
        if checks::pre_frontend(case) {
            assert!(!out.exists() && !candidate.exists());
        } else if case == "reused-output" {
            assert_eq!(checks::diagnostic_files(&out), old_output.unwrap());
            assert!(!candidate.exists());
        } else {
            checks::unchanged_original_owner(
                &out,
                &root.join("diagnostic-initial"),
                initial.as_ref().unwrap(),
                false,
            );
            if let Some((bytes, device, inode)) = old_candidate {
                assert!(
                    text.contains("MayHaveCreatedCandidate:")
                        && text.contains("publisher create-new publication refused")
                );
                let m = fs::symlink_metadata(candidate).unwrap();
                assert_eq!(read_bounded(candidate, 72 * 1024).unwrap(), bytes);
                assert_eq!((m.dev(), m.ino()), (device, inode));
            } else {
                assert!(text.contains("NotAttempted:"));
                assert!(!candidate.exists());
            }
        }
        json!({"stage":"exact_binary_public_action_refused","refusal":checks::refusal(case),
            "pre_frontend_refusal":checks::pre_frontend(case),"new_creation_claimed":false,
            "existing_candidate_preserved":case=="reused-candidate"})
    };
    if let Some(bytes) = request_bytes {
        assert_eq!(read_bounded(&request, 8192).unwrap(), bytes);
    }
    assert_eq!(
        read_bounded(&root.join(checks::ORIGINAL), 64 * 1024).unwrap(),
        original
    );
    build.recheck();
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 1200).unwrap();
    json!({"case":case,"invocation":record,"result":result,"extractor_binary_invoked":true,
        "terminal_exit":status,"stdout_sha256":digest(&stdout),"stderr_sha256":digest(&stderr),
        "source_custody_from_files":false,"hardware_observed":false,"launch_authority":false})
}
fn actual_recompile(
    root: &Path,
    case: &str,
    record: &Invocation,
    started: std::time::Instant,
) -> Value {
    let session = std::time::Instant::now();
    // Existing isolated child uses a NEW rustc callback and actual generated
    // package binding; it does not load a canonical file as source authority.
    let mut child = Command::new(std::env::current_exe().unwrap());
    common_environment(&mut child, root, record);
    let stdout = checked(
        child
            .args(["--exact", super::CHILD, "--ignored", "--nocapture"])
            .env(super::INPUT_ENV, root)
            .env(super::CASE_ENV, case)
            .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &record.cargo_observation,
            ),
        root,
        case,
        None,
    );
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 1200).unwrap();
    let text = std::str::from_utf8(&stdout).unwrap();
    let frames = text
        .lines()
        .filter_map(|l| l.strip_prefix(super::PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        text.lines()
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
        serde_json::to_value(record).unwrap()
    );
    assert_eq!(observation["result"]["stage"], "fresh_promoted_source_cpu");
    assert_eq!(observation["result"]["actual_callback_count"], 1);
    assert_eq!(observation["result"]["observation"]["cpu"]["cases"], 32);
    assert_eq!(observation["extractor_binary_invoked"], false);
    assert_eq!(observation["public_library_driver"], false);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 1200).unwrap();
    observation
}
#[test]
#[ignore = "root-owned real extractor + fresh promoted frontends; successful build pins required"]
fn actual_ordered_composition_cli_promotion_ladder() {
    let started = std::time::Instant::now();
    let build = pins::Build::read();
    timely(started.elapsed(), 1200).unwrap();
    let root = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh CLI ladder output"));
    assert!(root.is_absolute());
    fs::create_dir(&root).unwrap();
    assert!(
        !root
            .canonicalize()
            .unwrap()
            .starts_with(repository().parent().unwrap())
    );
    publish_json(&root, "build-input-observed.json", &build);
    let source_before = staging::fixture_source();
    let provider_before = super::super::super::inputs::current_sources();
    let workspace_before = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    super::prepare_public_staging(&root, started);
    let deps = preparation(&root, "package-original");
    let (dependency_before, files) = super::super::super::inputs::dependency_snapshot(&deps);
    publish_json(&root, "dependency-files.json", &files);
    drop(files);
    let mut observations = Vec::new();
    assert_eq!(super::CASES.len(), 17);
    for case in super::CASES.into_iter().chain(SELECTORS) {
        let session = std::time::Instant::now();
        let record_case = if SELECTORS.contains(&case) {
            "diagnostic"
        } else {
            case
        };
        let record = super::derive_public(&root, record_case);
        assert_eq!(record.provider_sources, provider_before);
        assert_eq!(record.dependency_snapshot, dependency_before);
        assert_eq!(record.workspace_lock_sha256, digest(&workspace_before));
        timely(started.elapsed(), 1200).unwrap();
        timely(session.elapsed(), 300).unwrap();
        publish_json(&root, &format!("{case}.invocation.json"), &record);
        let observed = if super::recompile(case) {
            let v = actual_recompile(&root, case, &record, started);
            let prior = case.strip_prefix("recompile-").unwrap();
            let published = observations
                .iter()
                .find(|v: &&Value| v["case"] == prior)
                .unwrap();
            assert_eq!(
                record.leaf_sha256,
                published["result"]["public_report"]["source_promotion"]["candidate_sha256"]
            );
            v
        } else {
            actual_binary(&root, case, &record, &build, started)
        };
        assert_eq!(super::derive_public(&root, record_case), record);
        assert!(
            fs::read_dir(preparation(&root, &record.package).join("analysis-output"))
                .unwrap()
                .next()
                .is_none()
        );
        build.recheck();
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&root, &format!("{case}.accepted.json"), &observed);
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observed);
    }
    assert_eq!(observations.len(), 20);
    let binary = observations
        .iter()
        .filter(|v| v["extractor_binary_invoked"] == true)
        .count();
    assert_eq!(binary, 17);
    let reports = observations
        .iter()
        .filter(|v| v["result"]["actual_callback_report_present"] == true)
        .count();
    assert_eq!(reports, 4);
    let refusals = observations
        .iter()
        .filter(|v| {
            matches!(
                v["result"]["stage"].as_str(),
                Some("exact_binary_selector_refused" | "exact_binary_public_action_refused")
            )
        })
        .count();
    assert_eq!(refusals, 13);
    let early = observations
        .iter()
        .filter(|v| v["result"]["pre_frontend_refusal"] == true)
        .count();
    assert_eq!(early, 8);
    let cpu: u64 = observations
        .iter()
        .filter_map(|v| v["result"]["observation"]["cpu"]["cases"].as_u64())
        .sum();
    assert_eq!(cpu, 96);
    assert_eq!(staging::fixture_source(), source_before);
    assert_eq!(
        super::super::super::inputs::current_sources(),
        provider_before
    );
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        workspace_before
    );
    assert_eq!(
        super::super::super::inputs::dependency_snapshot(&deps).0,
        dependency_before
    );
    build.recheck();
    timely(started.elapsed(), 1200).unwrap();
    let report = json!({"schema":"fe2o3-test-ordered-composition-cli-promotion-ladder-v1",
        "observations":observations,"build_inputs":build,"actual_binary_invocations":binary,
        "successful_actual_callback_reports":reports,"fresh_recompile_callbacks":3,"cpu_cases":cpu,
        "exact_binary_refusals":refusals,"pre_frontend_refusals":early,"source_publications":3,
        "standalone_packages":4,"fresh_shared_dependency_builds":1,"failure_callback_counts_claimed":false,
        "binary_identity_inherited_from_library_qualification":false,"source_custody_from_files":false,
        "normal_checked_handoff_qualified":false,"native_execution":false,"launch_authority":false,
        "acceptance":"requires successful completed parent and root runner, not historical JSON",
        "cleanup_scope":"owned direct children/process groups only; not a whole-family or isolation proof"});
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&root, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn binary_case_counts_include_selectors_without_minting_frontend_callbacks() {
    assert_eq!(super::CASES.len() + SELECTORS.len(), 20);
    assert_eq!(
        super::CASES.iter().filter(|c| !super::recompile(c)).count() + SELECTORS.len(),
        17
    );
    for case in SELECTORS {
        assert!(!"generic compilation failure".contains(selector_refusal(case)));
    }
}
