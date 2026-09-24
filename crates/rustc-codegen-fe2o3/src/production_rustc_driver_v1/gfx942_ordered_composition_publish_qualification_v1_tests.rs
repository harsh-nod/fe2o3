//! Real promotion/recompile in fresh task-owned packages, never candidate trees.
//! Parent/child completion is required; publication facts alone are historical.
use super::super::gfx942_inline_value_qualification_v30_tests::invocation_for_fixture_source_with_dependencies;
use super::*;

#[path = "gfx942_ordered_composition_publish_observation_v1_tests.rs"]
mod publication;
#[path = "gfx942_ordered_composition_publish_staging_v1_tests.rs"]
mod staging;

const OUTPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLISH_OUTPUT_V1";
const INPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLISH_INPUT_V1";
const CASE_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_PUBLISH_CASE_V1";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_composition_qualification_v1_tests::publisher::actual_ordered_composition_publish_child";
const PREFIX: &str = "FE2O3_ORDERED_COMPOSITION_PUBLISH_OBSERVATION_V1 ";
const CASES: [&str; 9] = [
    "copy",
    "recompile-copy",
    "edit",
    "recompile-edit",
    "collision",
    "const",
    "local",
    "wrapper",
    "inspection-after-publish",
];
fn create(path: &Path, bytes: &[u8], cap: usize) {
    super::super::publish_new_inert_output(
        path,
        bytes,
        cap,
        "composition publication qualification",
    )
    .unwrap();
}
fn case_parts(case: &str) -> (&'static str, &'static str) {
    match case {
        "copy" | "edit" | "inspection-after-publish" => {
            ("package-original", "ordered-composition-publish-direct")
        }
        "recompile-copy" => (
            "package-promoted-copy",
            "ordered-composition-publish-direct",
        ),
        "recompile-edit" => (
            "package-promoted-edit",
            "ordered-composition-publish-direct",
        ),
        "collision" => ("package-original", "ordered-composition-publish-collision"),
        "const" => ("package-original", "ordered-composition-publish-const"),
        "local" => ("package-original", "ordered-composition-publish-local"),
        "wrapper" => ("package-original", "ordered-composition-publish-wrapper"),
        _ => panic!("unknown closed publisher case"),
    }
}
fn preparation(root: &Path, name: &str) -> PathBuf {
    root.join(format!("{name}.preparation"))
}
fn hash_file(path: &Path, cap: usize) -> String {
    digest(&read_bounded(path, cap).unwrap())
}
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Invocation {
    schema: String,
    case: String,
    package: String,
    feature: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    manifest_sha256: String,
    lock_sha256: String,
    root_sha256: String,
    leaf_sha256: String,
    workspace_lock_sha256: String,
    metadata_sha256: String,
    sysroot_sha256: String,
    original_fixture_sha256: String,
    provider_sources: Vec<super::inputs::FilePin>,
    dependency_snapshot: super::inputs::TreePin,
    dependency_artifacts_sha256: String,
}
fn derive(root: &Path, case: &str) -> Invocation {
    let (name, feature) = case_parts(case);
    derive_package(root, case, name, feature)
}
fn derive_package(root: &Path, case: &str, name: &str, feature: &str) -> Invocation {
    let package = root.join(name);
    let prep = preparation(root, name);
    let dependencies = preparation(root, "package-original");
    let sysroot = read_bounded(&prep.join("sysroot.stdout"), 4096).unwrap();
    assert_eq!(
        sysroot,
        read_bounded(&dependencies.join("sysroot.stdout"), 4096).unwrap()
    );
    let metadata = read_bounded(&prep.join("metadata.stdout"), 16 * 1024 * 1024).unwrap();
    staging::metadata_feature(&metadata, &package, feature).unwrap();
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    let lock = read_bounded(&package.join("Cargo.lock"), 1024 * 1024).unwrap();
    staging::closure_matches(&workspace, &lock).unwrap();
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source_with_dependencies(
        (&prep, &dependencies),
        &package,
        staging::PACKAGE_NAME,
        staging::LIB_NAME,
        Some(feature),
        &package.join("src/lib.rs"),
        "gfx942",
    );
    Invocation {
        schema: "fe2o3-test-ordered-composition-publisher-invocation-v1".into(),
        case: case.into(),
        package: name.into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        manifest_sha256: hash_file(&package.join("Cargo.toml"), 64 * 1024),
        lock_sha256: digest(&lock),
        root_sha256: hash_file(&package.join("src/lib.rs"), 4096),
        leaf_sha256: hash_file(&package.join(staging::LEAF), 72 * 1024),
        workspace_lock_sha256: digest(&workspace),
        metadata_sha256: digest(&metadata),
        sysroot_sha256: digest(&sysroot),
        original_fixture_sha256: digest(&staging::fixture_source()),
        provider_sources: super::inputs::current_sources(),
        dependency_snapshot: super::inputs::dependency_snapshot(&dependencies).0,
        dependency_artifacts_sha256: hash_file(
            &dependencies.join("dependencies.stdout"),
            16 * 1024 * 1024,
        ),
    }
}
struct PublishCallbacks<'a> {
    case: &'a str,
    root: &'a Path,
    started: std::time::Instant,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for PublishCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let target = transaction.lower_ordered_composition_diagnostic_v1()?;
            publication::observe(target, self.case, self.root, self.started)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated staged-package child; use actual_ordered_composition_publish_ladder"]
fn actual_ordered_composition_publish_child() {
    let started = std::time::Instant::now();
    let root = PathBuf::from(std::env::var_os(INPUT_ENV).expect("staging root"));
    assert!(root.is_absolute());
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    let case = std::env::var(CASE_ENV).expect("case");
    let (package, _) = case_parts(&case);
    let record = derive(&root, &case);
    let retained: Invocation = serde_json::from_slice(
        &read_bounded(&root.join(format!("{case}.invocation.json")), 256 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(record, retained);
    for (key, expected) in [
        (CRATE_BINDING_ID_ENV_V1, record.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            record.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", staging::PACKAGE_NAME),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", staging::LIB_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), expected);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        root.join(package)
    );
    super::super::require_canonical_overflow_checks_v1(&record.args).unwrap();
    let mut callbacks = PublishCallbacks {
        case: &case,
        root: &root,
        started,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&record.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let result = callbacks
        .result
        .expect("actual staged-source callback not reached");
    if let Err(error) = &result {
        assert!(error.len() <= 64 * 1024);
        publish_json(
            &root,
            &format!("{case}.callback-error.json"),
            &json!({
                "diagnostic":error,"publication_effects":"consult independently retained publication-facts; unknown if callback never returned",
                "accepted":false,
            }),
        );
    }
    let observed = result.unwrap();
    assert_eq!(
        derive(&root, &case),
        record,
        "actual package/source/dependencies changed"
    );
    assert!(
        fs::read_dir(preparation(&root, package).join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    timely(started.elapsed(), 300).unwrap();
    let frame = serde_json::to_string(&json!({
        "schema":"fe2o3-test-ordered-composition-publisher-observation-v1",
        "case":case,"invocation":record,"observation":observed,
        "actual_rustc_callbacks":callbacks.calls,"fresh_frontend":true,
        "normal_checked_handoff_qualified":false,"native_execution":false,"launch_authority":false,
    }))
    .unwrap();
    assert!(frame.len() <= 256 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "actual staged-source publication/recompile; fresh output and pinned compiler required"]
fn actual_ordered_composition_publish_ladder() {
    let started = std::time::Instant::now();
    let root =
        PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh task-owned staging output"));
    assert!(root.is_absolute());
    fs::create_dir(&root).unwrap();
    let actual_root = root.canonicalize().unwrap();
    let compiler_parent = repository().parent().unwrap().to_path_buf();
    assert!(
        !actual_root.starts_with(&compiler_parent),
        "never publish source in either compiler candidate or its siblings"
    );
    let source_before = staging::fixture_source();
    let provider_before = super::inputs::current_sources();
    let workspace_before = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    let rustc_path = PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned RUSTC"));
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let bytes = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
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
    for name in [
        "package-original",
        "package-promoted-copy",
        "package-promoted-edit",
    ] {
        staging::package(&root, name, name == "package-original");
        staging::prepare(&root, name, &sysroot, started);
        fs::create_dir(preparation(&root, name).join("analysis-output")).unwrap();
    }
    let deps = preparation(&root, "package-original");
    let target = deps.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(root.join("package-original")).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(root.join("package-original/Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &deps,"dependencies",Some(&target));
    let (dependency_before, files) = super::inputs::dependency_snapshot(&deps);
    publish_json(&root, "dependency-files.json", &files);
    drop(files);
    let mut observations = Vec::new();
    for case in CASES {
        let session = std::time::Instant::now();
        let (package, _) = case_parts(case);
        let record = derive(&root, case);
        assert_eq!(record.dependency_snapshot, dependency_before);
        assert_eq!(record.provider_sources, provider_before);
        assert_eq!(record.workspace_lock_sha256, digest(&workspace_before));
        publish_json(&root, &format!("{case}.invocation.json"), &record);
        timely(started.elapsed(), 1200).unwrap();
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
                .env("CARGO_MANIFEST_DIR", root.join(package))
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
        let observed: Value = serde_json::from_str(frames[0]).unwrap();
        assert_eq!(
            observed["schema"],
            "fe2o3-test-ordered-composition-publisher-observation-v1"
        );
        assert_eq!(observed["case"], case);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(derive(&root, case), record);
        if matches!(case, "copy" | "edit") {
            let promoted = if case == "copy" {
                "package-promoted-copy"
            } else {
                "package-promoted-edit"
            };
            let bytes = read_bounded(&root.join(promoted).join(staging::LEAF), 72 * 1024).unwrap();
            assert_eq!(
                observed["observation"]["facts"]["published"]["candidate_sha256"],
                digest(&bytes)
            );
            assert_eq!(
                observed["observation"]["facts"]["published"]["original_sha256"],
                digest(&source_before)
            );
            assert_eq!(
                observed["observation"]["facts"]["publication_may_have_created"],
                true
            );
        }
        if case == "inspection-after-publish" {
            let bytes = read_bounded(&root.join("inspection-after-publish.rs"), 72 * 1024).unwrap();
            assert_eq!(
                observed["observation"]["stage"],
                "post_publication_inspection_refused"
            );
            assert_eq!(
                observed["observation"]["facts"]["published"]["candidate_sha256"],
                digest(&bytes)
            );
            assert_eq!(
                observed["observation"]["facts"]["publication_may_have_created"],
                true
            );
            assert_eq!(observed["observation"]["inspection_accepted"], false);
        }
        if matches!(case, "recompile-copy" | "recompile-edit") {
            let prior = if case == "recompile-copy" {
                "copy"
            } else {
                "edit"
            };
            let publication = observations
                .iter()
                .find(|o: &&Value| o["case"] == prior)
                .unwrap();
            assert_eq!(
                observed["invocation"]["leaf_sha256"],
                publication["observation"]["facts"]["published"]["candidate_sha256"]
            );
            assert_eq!(
                observed["observation"]["stage"],
                "fresh_promoted_source_frontend"
            );
            assert_eq!(observed["observation"]["source_owner_reused"], false);
        }
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&root, &format!("{case}.accepted.json"), &observed);
        timely(session.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observed);
    }
    assert_eq!(staging::fixture_source(), source_before);
    assert_eq!(super::inputs::current_sources(), provider_before);
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        workspace_before
    );
    assert_eq!(
        super::inputs::dependency_snapshot(&deps).0,
        dependency_before
    );
    assert_eq!(observations.len(), 9);
    let cpu_cases: u64 = observations
        .iter()
        .filter_map(|o| o["observation"]["cpu"]["cases"].as_u64())
        .sum();
    assert_eq!(cpu_cases, 128);
    let negatives = observations
        .iter()
        .filter(|o| o["observation"]["stage"] == "exact_hir_publication_refused")
        .count();
    assert_eq!(negatives, 4);
    assert_eq!(
        observations
            .iter()
            .filter(|o| o["observation"]["stage"] == "post_publication_inspection_refused")
            .count(),
        1
    );
    let report = json!({
        "schema":"fe2o3-test-ordered-composition-publisher-ladder-v1",
        "observations":observations,"actual_rustc_sessions":9,"cpu_cases":cpu_cases,
        "actual_source_publications":3,"fresh_promoted_frontends":2,"exact_hir_refusals":negatives,
        "post_publication_inspection_refusal_with_retained_effect_facts":1,
        "actual_wrong_source_hash_refusals":2,"actual_no_replace_refusals":2,
        "fresh_shared_dependency_builds":1,"actual_standalone_metadata_preparations":3,
        "compiler_candidate_sources_written":false,"provider_unchanged":true,
        "edited_program_is_equivalent_to_original":false,"normal_checked_handoff_qualified":false,
        "native_execution":false,"launch_authority":false,
        "acceptance":"requires completed successful parent and runner, not historical JSON",
        "cleanup_scope":"bounded direct children/process groups; no full family proof",
    });
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&root, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn publisher_case_roster_keeps_fresh_metadata_and_source_stages_distinct() {
    assert_eq!(CASES.len(), 9);
    assert_ne!(case_parts("copy").0, case_parts("recompile-copy").0);
    assert_ne!(
        case_parts("recompile-copy").0,
        case_parts("recompile-edit").0
    );
    assert_eq!(case_parts("copy").1, case_parts("recompile-copy").1);
}

#[path = "gfx942_ordered_composition_public_promotion_v1_tests.rs"]
mod public_action;
