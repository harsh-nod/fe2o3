//! Fresh public CLI publication -> actual checked source -> ordinary inert outputs.
use super::super as action;
use super::super::super as publisher;
use super::super::super::super as qualification;
use super::super::super::super::super as driver;
use super::*;
use std::io::Write;
#[path = "gfx942_ordered_composition_promoted_normal_checks_v1_tests.rs"]
mod checks;
#[path = "gfx942_ordered_composition_promoted_normal_inputs_v1_tests.rs"]
mod inputs;
#[path = "gfx942_ordered_composition_promoted_normal_observation_v1_tests.rs"]
mod observation;
#[path = "gfx942_ordered_composition_transport_ladder_v1_tests.rs"]
mod transport_ladder;
#[path = "gfx942_ordered_composition_transport_roles_v1_tests.rs"]
mod transport_roles;
const OUTPUT_ENV: &str = "FE2O3_TEST_COMPOSITION_PROMOTED_NORMAL_OUTPUT_V1";
const INPUT_ENV: &str = "FE2O3_TEST_COMPOSITION_PROMOTED_NORMAL_INPUT_V1";
const VARIANT_ENV: &str = "FE2O3_TEST_COMPOSITION_PROMOTED_NORMAL_VARIANT_V1";
const MODE_ENV: &str = "FE2O3_TEST_COMPOSITION_PROMOTED_NORMAL_MODE_V1";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_composition_qualification_v1_tests::publisher::public_action::cli::promoted_normal::actual_promoted_normal_child";
const PREFIX: &str = "FE2O3_COMPOSITION_PROMOTED_NORMAL_SESSION_V1 ";
struct Body<'a> {
    transport_roles: bool,
    role_ledger: Option<fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1>,
    variant: &'a str,
    output: &'a Path,
    started: std::time::Instant,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for Body<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = driver::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let target = transaction.lower_ordered_composition_target_v1()?;
            if self.transport_roles {
                let (value, ledger) =
                    transport_roles::observe(target, self.variant, self.output, self.started)?;
                self.role_ledger = Some(ledger);
                Ok(value)
            } else {
                observation::observe(target, self.variant, self.output, self.started)
            }
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated fresh-source child; invoke only the promoted normal parent"]
fn actual_promoted_normal_child() {
    let started = std::time::Instant::now();
    let root = PathBuf::from(std::env::var_os(INPUT_ENV).expect("fresh prepared root"));
    assert!(root.is_absolute());
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    let variant = std::env::var(VARIANT_ENV).unwrap();
    let mode = std::env::var(MODE_ENV).unwrap();
    let name = if mode == transport_roles::MODE {
        transport_roles::case(&variant).unwrap()
    } else {
        checks::normal_case(&variant, &mode).unwrap()
    };
    // Mode is not part of source invocation identity; all three modes rederive the same source args.
    let record = inputs::invocation(&root, &variant, &variant);
    let retained: Invocation = serde_json::from_slice(
        &read_bounded(&root.join(format!("{name}.invocation.json")), 256 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(record, retained);
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, record.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            record.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", staging::PACKAGE_NAME),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", staging::LIB_NAME),
        ("FE2O3_EXTRACT_ORDERED_COMPOSITION_V1", "1"),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        root.join(inputs::package(&variant))
    );
    driver::require_canonical_overflow_checks_v1(&record.args).unwrap();
    let source = inputs::snapshot(&inputs::source(&root, &variant), 72 * 1024);
    assert_eq!(record.leaf_sha256, source.sha256);
    let output = root.join(format!("{name}.output"));
    assert!(!output.exists());
    timely(started.elapsed(), 300).unwrap();
    // Only the new mode retains a ledger through the terminal output boundary.
    let mut role_ledger = None;
    let result = match mode.as_str() {
        "observe" | transport_roles::MODE => {
            let mut body = Body {
                transport_roles: mode == transport_roles::MODE,
                role_ledger: None,
                variant: &variant,
                output: &output,
                started,
                calls: 0,
                result: None,
            };
            rustc_driver::run_compiler(&record.args, &mut body);
            assert_eq!(body.calls, 1);
            role_ledger = body.role_ledger.take();
            body.result.expect("actual source callback")
        }
        "llvm" => driver::run_production_gfx942_llvm_extraction_driver_v1(&record.args, &output)
            .map(|()| {
                json!({"stage":"public_normal_llvm","llvm_sha256":
                digest(&read_bounded(&output,4*1024*1024).unwrap())})
            }),
        "handoff" => driver::run_production_gfx942_compiler_handoff_extraction_driver_v1(
            &record.args,
            &output,
        )
        .map(|()| {
            let bytes = read_bounded(&output, 4 * 1024 * 1024).unwrap();
            let h = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(&bytes).unwrap();
            assert!(!h.authenticates_compiler_origin() && !h.grants_compiler_authority());
            json!({"stage":"public_normal_handoff","handoff_sha256":digest(&bytes)})
        }),
        _ => unreachable!(),
    };
    if let Err(error) = &result {
        assert!(error.len() <= 64 * 1024);
        publish_json(
            &root,
            &format!("{name}.rejection.json"),
            &json!({
                "accepted":false,"diagnostic":error,"source":source,"output_exists":output.exists()
            }),
        );
    }
    let observed = result.unwrap();
    inputs::recheck(&root, &variant, &variant, &record, &source);
    timely(started.elapsed(), 300).unwrap();
    let transport = mode == transport_roles::MODE;
    assert_eq!(role_ledger.is_some(), transport);
    let schema = if transport {
        transport_roles::SCHEMA
    } else {
        "fe2o3-test-composition-promoted-normal-session-v1"
    };
    let prefix = if transport {
        transport_roles::FRAME_PREFIX
    } else {
        PREFIX
    };
    let frame = serde_json::to_string(&json!({
        "schema":schema,
        "variant":variant,"mode":mode,"invocation":record,"source":source,"observation":observed,
        "actual_fresh_frontend":true,"runtime_conditions_discharged":false,
        "source_custody_exported":false,"hardware_observed":false,"protected_authority":false
    }))
    .unwrap();
    assert!(frame.len() <= 256 * 1024);
    println!("\n{prefix}{frame}");
    std::io::stdout().flush().unwrap();
    timely(started.elapsed(), 300).unwrap();
    drop(frame);
    drop(role_ledger);
}
fn normal_session(root: &Path, variant: &str, mode: &str, started: std::time::Instant) -> Value {
    let session = std::time::Instant::now();
    let name = checks::normal_case(variant, mode).unwrap();
    let record = inputs::invocation(root, variant, variant);
    let source = inputs::snapshot(&inputs::source(root, variant), 72 * 1024);
    inputs::write_invocation(root, &name, &record);
    let mut child = Command::new(std::env::current_exe().unwrap());
    inputs::environment(&mut child, root, &record);
    child
        .args(["--exact", CHILD, "--ignored", "--nocapture"])
        .env(INPUT_ENV, root)
        .env(VARIANT_ENV, variant)
        .env(MODE_ENV, mode)
        .env("FE2O3_EXTRACT_ORDERED_COMPOSITION_V1", "1")
        .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
        .env(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            &record.cargo_observation,
        );
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let bytes = checked(&mut child, root, &name, None);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let frames = text
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let observed: Value = serde_json::from_str(frames[0]).unwrap();
    assert_eq!(
        observed["schema"],
        "fe2o3-test-composition-promoted-normal-session-v1"
    );
    assert_eq!(observed["variant"], variant);
    assert_eq!(observed["mode"], mode);
    assert_eq!(
        observed["invocation"],
        serde_json::to_value(&record).unwrap()
    );
    assert_eq!(observed["source"], serde_json::to_value(&source).unwrap());
    assert_eq!(observed["actual_fresh_frontend"], true);
    for key in [
        "runtime_conditions_discharged",
        "source_custody_exported",
        "hardware_observed",
        "protected_authority",
    ] {
        assert_eq!(observed[key], false);
    }
    inputs::recheck(root, variant, variant, &record, &source);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    publish_json(root, &format!("{name}.accepted.json"), &observed);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    observed
}
fn final_publication(root: &Path, report: &Value, started: std::time::Instant) {
    assert!(!root.join("observation.json").exists());
    assert!(!root.join("observation.unaccepted.json").exists());
    // Any unwind after report creation quarantines the positive file. Root completion
    // remains required; this is not a cleanup/process-family acceptance receipt.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = serde_json::to_vec_pretty(report).unwrap();
        assert!(bytes.len() <= 4 * 1024 * 1024);
        timely(started.elapsed(), 2400).unwrap();
        publisher::create(&root.join("observation.json"), &bytes, 4 * 1024 * 1024);
        timely(started.elapsed(), 2400).unwrap();
        let mut out = std::io::stdout().lock();
        out.write_all(b"Fresh promoted sources completed ordinary checked/CPU/LLVM/handoff qualification; native pending.\n").unwrap();
        out.flush().unwrap();
        timely(started.elapsed(), 2400).unwrap();
    }));
    if let Err(error) = result {
        let positive = root.join("observation.json");
        if positive.exists() {
            let rejected = root.join("observation.unaccepted.json");
            assert!(!rejected.exists());
            fs::rename(positive, rejected).unwrap();
        }
        std::panic::resume_unwind(error);
    }
}
#[test]
#[ignore = "root-owned actual CLI publication and fresh normal source matrix; current completed build pins required"]
fn actual_promoted_normal_ladder() {
    let started = std::time::Instant::now();
    let build = pins::Build::read();
    let root = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh promoted normal output"));
    let root = inputs::fresh_output_root(&root, repository().parent().unwrap()).unwrap();
    publish_json(&root, "build-input-observed.json", &build);
    let original = staging::fixture_source();
    let seed = inputs::finite_seed(&original).unwrap();
    let sources = qualification::inputs::current_sources();
    let lock = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    action::prepare_public_staging_with_seed(&root, started, &seed);
    assert_eq!(
        read_bounded(&inputs::source(&root, "original"), 64 * 1024).unwrap(),
        seed
    );
    publish_json(
        &root,
        "finite-seed.json",
        &json!({
            "schema":"fe2o3-test-composition-finite-promotion-seed-v1",
            "original_dynamic_bytes":original.len(),"original_dynamic_sha256":digest(&original),
            "finite_bytes":seed.len(),"finite_sha256":digest(&seed),
            "edit":"exact launch annotation adds max_grid=[2,1,1] before any publication",
            "original_fixture_modified":false,"published_candidate_modified":false
        }),
    );
    let dependencies = publisher::preparation(&root, "package-original");
    let (dependency_before, files) = qualification::inputs::dependency_snapshot(&dependencies);
    publish_json(&root, "dependency-files.json", &files);
    drop(files);
    let original_snapshot = inputs::snapshot(&inputs::source(&root, "original"), 64 * 1024);
    let mut actions = Vec::new();
    for case in checks::CLI_CASES {
        actions.push(checks::actual_cli(&root, case, &build, started));
    }
    assert_eq!(actions.len(), 13);
    let published_snapshots = ["copy", "preserve", "edit"].map(|variant| {
        (
            variant,
            inputs::snapshot(&inputs::source(&root, variant), 72 * 1024),
        )
    });
    let mut rows = vec![normal_session(&root, "original", "observe", started)];
    for variant in ["copy", "preserve", "edit"] {
        for mode in ["observe", "llvm", "handoff"] {
            rows.push(normal_session(&root, variant, mode, started));
        }
    }
    let joined = checks::join(&root, &actions, &rows);
    assert_eq!(
        inputs::snapshot(&inputs::source(&root, "original"), 64 * 1024),
        original_snapshot
    );
    for (variant, snapshot) in published_snapshots {
        assert_eq!(
            inputs::snapshot(&inputs::source(&root, variant), 72 * 1024),
            snapshot
        );
    }
    assert_eq!(staging::fixture_source(), original);
    assert_eq!(qualification::inputs::current_sources(), sources);
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        lock
    );
    assert_eq!(
        qualification::inputs::dependency_snapshot(&dependencies).0,
        dependency_before
    );
    build.recheck();
    timely(started.elapsed(), 2400).unwrap();
    let report = json!({
        "schema":"fe2o3-test-composition-promoted-normal-ladder-v1",
        "workload_children":23,"actual_extractor_invocations":13,"source_publications":3,
        "feature_specific_capture_diagnostics":3,"exact_source_refusals":6,
        "actual_normal_sessions":10,"fresh_candidate_normal_triples":3,"cpu_cases":128,
        "descriptor_extension_refusals":15,"candidate_descriptor_extension_refusals":12,
        "actions":actions,"normal_sessions":rows,"candidate_joins":joined,
        "original_source":original_snapshot,"provider_sources":sources,"dependencies":dependency_before,
        "build_inputs":build,"finite_seed_sha256":digest(&seed),
        "fresh_source_custody_constructed_in_callback":true,"source_custody_from_files":false,
        "published_candidates_modified":false,"historical_evidence_rewritten":false,
        "runtime_conditions_discharged":false,"native_worker_qualified":false,
        "hardware_observed":false,"protected_authority":false,
        "cleanup_scope":"bounded direct child/process group only; outer root supervisor required",
        "acceptance":"completed successful parent and root runner; JSON alone is historical"
    });
    final_publication(&root, &report, started);
}
#[test]
fn whole_runner_deadline_is_strict_at_and_after_terminal_boundary() {
    for seconds in [300_u64, 2400] {
        let bound = std::time::Duration::from_secs(seconds);
        assert!(timely(bound - std::time::Duration::from_nanos(1), seconds).is_ok());
        assert!(timely(bound, seconds).is_err());
        assert!(timely(bound + std::time::Duration::from_nanos(1), seconds).is_err());
    }
}
