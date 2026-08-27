use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const LDS_PIPELINE: &str = "collected-lds-reduction-v1";
const ATOMIC_PIPELINE: &str = "collected-scoped-atomic-v1";
const LDS_CRATE_NAME: &str = "fe2o3_collected_lds_reduction_v1_fixture";
const ATOMIC_CRATE_NAME: &str = "fe2o3_collected_scoped_atomic_v1_fixture";
const LDS_METADATA: &str = "fe2o3-lds-reduction-v1-reviewed";
const ATOMIC_METADATA: &str = "fe2o3-scoped-atomic-v1-reviewed";
const LDS_CRATE_BINDING: &str = "fd63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da";
const ATOMIC_CRATE_BINDING: &str =
    "dede4079399a3df33da7bcc9fc46bc84c3ab329642fa27241feaf10aff06388e";
const CARGO_METADATA_OBSERVATION: &str =
    "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0";
const WORKSPACE_REMAP: &str = "/fe2o3-reviewed-workspace";
const LDS_SOURCE: &str = include_str!("../../../examples/workgroup_sync_v1/src/kernel.rs");
const ATOMIC_SOURCE: &str =
    include_str!("../../../examples/workgroup_sync_v1/src/scoped_atomic.rs");
const REPORT_AUTHORITY_ENV: &str = "FE2O3_WORKGROUP_SYNC_REPORT_AUTHORITY";

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Clone, Copy)]
enum ProfileKind {
    Lds,
    Atomic,
}

impl ProfileKind {
    const fn pipeline(self) -> &'static str {
        match self {
            Self::Lds => LDS_PIPELINE,
            Self::Atomic => ATOMIC_PIPELINE,
        }
    }

    const fn crate_name(self) -> &'static str {
        match self {
            Self::Lds => LDS_CRATE_NAME,
            Self::Atomic => ATOMIC_CRATE_NAME,
        }
    }

    const fn metadata(self) -> &'static str {
        match self {
            Self::Lds => LDS_METADATA,
            Self::Atomic => ATOMIC_METADATA,
        }
    }

    const fn crate_binding(self) -> &'static str {
        match self {
            Self::Lds => LDS_CRATE_BINDING,
            Self::Atomic => ATOMIC_CRATE_BINDING,
        }
    }

    const fn source_remap(self) -> &'static str {
        match self {
            Self::Lds => "/fe2o3-reviewed-workspace/lds-reduction-v1.rs",
            Self::Atomic => "/fe2o3-reviewed-workspace/scoped-atomic-v1.rs",
        }
    }

    const fn fixture(self) -> &'static str {
        match self {
            Self::Lds => "collected-lds-reduction-v1",
            Self::Atomic => "collected-scoped-atomic-v1",
        }
    }
}

struct TestOutput {
    path: PathBuf,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        let path = cargo_target(workspace).join(format!(
            "workgroup-sync-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale workgroup-sync test output");
        }
        std::fs::create_dir_all(&path).expect("create workgroup-sync test output");
        Self { path }
    }
}

impl Drop for TestOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct CleanCargoTarget {
    path: PathBuf,
}

impl CleanCargoTarget {
    fn new(workspace: &Path, label: &str) -> Self {
        let path = cargo_target(workspace).join(format!(
            "workgroup-sync-v1-clean-{label}-{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale clean workgroup-sync target");
        }
        Self { path }
    }
}

impl Drop for CleanCargoTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
struct CompilerProfile<'a> {
    target: &'a str,
    crate_name: &'a str,
    metadata: &'a str,
    crate_binding: &'a str,
    overflow_checks: bool,
}

impl<'a> CompilerProfile<'a> {
    const fn exact(kind: ProfileKind) -> CompilerProfile<'static> {
        CompilerProfile {
            target: "gfx942:xnack-",
            crate_name: kind.crate_name(),
            metadata: kind.metadata(),
            crate_binding: kind.crate_binding(),
            overflow_checks: false,
        }
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

fn profile_name() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

fn frontend_target(workspace: &Path) -> PathBuf {
    cargo_target(workspace).join("workgroup-sync-v1-frontend-target")
}

fn build_frontend_dependencies(workspace: &Path) -> Result<(), String> {
    FRONTEND_DEPENDENCIES
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command.current_dir(workspace).args([
                "build",
                "--locked",
                "-p",
                "fe2o3-device",
                "-p",
                "fe2o3-host",
            ]);
            let mut rustflags = std::env::var_os("RUSTFLAGS").unwrap_or_default();
            if !rustflags.is_empty() {
                rustflags.push(" ");
            }
            rustflags.push("-Zalways-encode-mir");
            command
                .arg("--target-dir")
                .arg(frontend_target(workspace))
                .env("CARGO_INCREMENTAL", "0")
                .env("RUSTFLAGS", rustflags);
            let output = command.output().map_err(|error| error.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "frontend dependency build failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        })
        .clone()
}

fn compile(
    workspace: &Path,
    output: &TestOutput,
    label: &str,
    source: &str,
    kind: ProfileKind,
    profile: CompilerProfile<'_>,
) -> Output {
    build_frontend_dependencies(workspace).expect("build workgroup-sync frontend dependencies");
    let backend_target = cargo_target(workspace).join(profile_name());
    let frontend_target = frontend_target(workspace).join("debug");
    let backend = backend_target.join("librustc_codegen_fe2o3.so");
    let device = frontend_target.join("libfe2o3_device.rlib");
    let host = frontend_target.join("libfe2o3_host.rlib");
    for required in [&backend, &device, &host] {
        assert!(required.is_file(), "missing {}", required.display());
    }

    let source_path = output.path.join(format!("{label}.rs"));
    std::fs::write(&source_path, source).expect("write exact or hostile fixture");
    let artifact_dir = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create empty artifact directory");
    let fixture_manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures")
        .join(kind.fixture());

    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}={}",
            source_path.display(),
            kind.source_remap()
        ))
        .arg(format!(
            "--remap-path-prefix={}={WORKSPACE_REMAP}",
            workspace.display()
        ))
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg(profile.crate_name)
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            frontend_target.join("deps").display()
        ))
        .arg(format!("-Coverflow-checks={}", profile.overflow_checks))
        .arg(format!("-Cmetadata={}", profile.metadata))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-o")
        .arg(output.path.join(format!("lib{label}.rlib")))
        .env("CARGO_MANIFEST_DIR", fixture_manifest)
        .env("FE2O3_CRATE_BINDING_ID_V1", profile.crate_binding)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            CARGO_METADATA_OBSERVATION,
        )
        .env("FE2O3_HSACO_DIR", &artifact_dir)
        .env("FE2O3_TARGET", profile.target)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", kind.pipeline())
        .output()
        .expect("run workgroup-sync compiler fixture")
}

fn mutation(source: &str, old: &str, new: &str) -> String {
    assert_eq!(source.matches(old).count(), 1, "non-unique mutation anchor");
    source.replacen(old, new, 1)
}

fn assert_rejected(result: &Output, label: &str) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "hostile case `{label}` compiled");
    assert!(
        !stderr.contains("authenticated exact source bytes"),
        "hostile case `{label}` reached authenticated receipt:\n{stderr}"
    );
}

fn authenticated_authority(stderr: &str) -> &str {
    let suffix = stderr
        .split_once("consumed sealed authority ")
        .expect("authenticated authority marker")
        .1;
    let authority = suffix
        .split_once(" (bound value ")
        .expect("authenticated authority terminator")
        .0;
    assert_eq!(authority.len(), 64, "authority is not SHA-256 hex");
    assert!(authority.bytes().all(|byte| byte.is_ascii_hexdigit()));
    authority
}

fn admitted_closure(stderr: &str) -> String {
    let marker = "complete reachable portable-MIR closure modulo those identity-bound terminals ";
    let identity: String = stderr
        .split_once(marker)
        .unwrap_or_else(|| panic!("missing admitted closure marker:\n{stderr}"))
        .1
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .take(64)
        .collect();
    assert_eq!(identity.len(), 64, "closure identity is not SHA-256 hex");
    identity
}

fn internal_helper_exports(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix("  [internal-helper] "))
        .map(str::to_owned)
        .collect()
}

fn copy_relocated_workspace(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create relocated workspace");
    for file in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        std::fs::copy(source.join(file), destination.join(file))
            .unwrap_or_else(|error| panic!("copy relocated {file}: {error}"));
    }
    copy_tree(&source.join("crates"), &destination.join("crates"));
    copy_tree(&source.join("examples"), &destination.join("examples"));
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination)
        .unwrap_or_else(|error| panic!("create {}: {error}", destination.display()));
    for entry in std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("read {}: {error}", source.display()))
    {
        let entry = entry.expect("read source-tree entry");
        let file_type = entry.file_type().expect("inspect source-tree entry");
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", entry.path().display()));
        } else {
            panic!(
                "relocated workspace source is not a regular file: {}",
                entry.path().display()
            );
        }
    }
}

fn run_relocated_exact_profiles(workspace: &Path, target: &Path) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace).args(["test", "--locked"]);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    command
        .args([
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "workgroup_sync_v1",
            "--target-dir",
        ])
        .arg(target)
        .args([
            "source_authenticates_complete_profile",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env(REPORT_AUTHORITY_ENV, "1")
        .output()
        .expect("run relocated exact-profile suite")
}

fn assert_relocated_success(label: &str, output: &Output) {
    let text = command_text(output);
    assert!(output.status.success(), "{label} failed:\n{text}");
    let authorities = reported_authorities(output);
    assert_eq!(
        authorities.len(),
        2,
        "{label} omitted authority reports:\n{text}"
    );
    assert!(authorities.contains_key("exact-lds"));
    assert!(authorities.contains_key("exact-atomic"));
}

fn reported_authorities(output: &Output) -> BTreeMap<String, String> {
    command_text(output)
        .lines()
        .filter_map(|line| {
            let marker = "WORKGROUP_SYNC_AUTHORITY ";
            let report = line
                .find(marker)
                .map(|offset| &line[offset + marker.len()..])?;
            let (label, identity) = report.split_once(' ')?;
            Some((label.to_owned(), identity.trim().to_owned()))
        })
        .collect()
}

fn reported_closures(output: &Output) -> BTreeMap<String, String> {
    command_text(output)
        .lines()
        .filter_map(|line| {
            let marker = "WORKGROUP_SYNC_CLOSURE ";
            let report = line
                .find(marker)
                .map(|offset| &line[offset + marker.len()..])?;
            let (label, identity) = report.split_once(' ')?;
            Some((label.to_owned(), identity.trim().to_owned()))
        })
        .collect()
}

fn reported_helpers(output: &Output) -> BTreeMap<String, Vec<String>> {
    let mut helpers = BTreeMap::<String, Vec<String>>::new();
    for line in command_text(output).lines() {
        let marker = "WORKGROUP_SYNC_HELPER ";
        let Some(report) = line
            .find(marker)
            .map(|offset| &line[offset + marker.len()..])
        else {
            continue;
        };
        let Some((label, export)) = report.split_once(' ') else {
            continue;
        };
        helpers
            .entry(label.to_owned())
            .or_default()
            .push(export.trim().to_owned());
    }
    helpers
}

fn run_clean_exact_profiles(
    workspace: &Path,
    target: &CleanCargoTarget,
    incremental: Option<&str>,
    rustflags: Option<&str>,
) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args([
            "test",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "workgroup_sync_v1",
            "source_authenticates_complete_profile",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CARGO_TARGET_DIR", &target.path)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("FE2O3_VERBOSE", "1")
        .env(REPORT_AUTHORITY_ENV, "1");
    match incremental {
        Some(value) => {
            command.env("CARGO_INCREMENTAL", value);
        }
        None => {
            command.env_remove("CARGO_INCREMENTAL");
        }
    }
    match rustflags {
        Some(value) => {
            command.env("RUSTFLAGS", value);
        }
        None => {
            command.env_remove("RUSTFLAGS");
        }
    }
    command
        .output()
        .expect("run clean workgroup-sync exact fixtures")
}

fn command_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_exact_profile_authenticates(kind: ProfileKind, source: &str, label: &str) {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let result = compile(
        &workspace,
        &output,
        label,
        source,
        kind,
        CompilerProfile::exact(kind),
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "admission-only profile emitted code"
    );
    for marker in [
        "authenticated exact source bytes",
        "separate reviewed profile namespace, distinct compiler-derived ordinary #[kernel(typed)] root",
        "exact rustc FnAbi, frozen V4 provider-semantic definitions and reviewed semantic-terminal manifest",
        "complete reachable portable-MIR closure modulo those identity-bound terminals",
        "reviewed source-to-profile and source-to-terminal correspondence only",
        "no generic lowering, terminal-body refinement, compiler-refinement proof, LLVM lowering, Worker V2",
    ] {
        assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
    }
    if std::env::var_os(REPORT_AUTHORITY_ENV).is_some() {
        println!(
            "WORKGROUP_SYNC_CLOSURE {label} {}",
            admitted_closure(&stderr)
        );
        for helper in internal_helper_exports(&stderr) {
            println!("WORKGROUP_SYNC_HELPER {label} {helper}");
        }
        println!(
            "WORKGROUP_SYNC_AUTHORITY {label} {}",
            authenticated_authority(&stderr)
        );
    }
}

#[test]
fn clean_build_modes_admit_the_same_workgroup_sync_closures() {
    let workspace = workspace();
    let default_target = CleanCargoTarget::new(&workspace, "incremental-default");
    let disabled_target = CleanCargoTarget::new(&workspace, "incremental-disabled");
    let metadata_target = CleanCargoTarget::new(&workspace, "provider-metadata-varied");

    let default = run_clean_exact_profiles(&workspace, &default_target, None, None);
    let default_text = command_text(&default);
    assert!(
        default.status.success(),
        "clean default-incremental workgroup-sync run failed:\n{default_text}"
    );

    let disabled = run_clean_exact_profiles(&workspace, &disabled_target, Some("0"), None);
    let disabled_text = command_text(&disabled);
    assert!(
        disabled.status.success(),
        "clean CARGO_INCREMENTAL=0 workgroup-sync run failed:\n{disabled_text}"
    );

    assert_eq!(
        reported_authorities(&default),
        reported_authorities(&disabled),
        "nonsemantic Cargo incremental mode changed the sealed authorities"
    );
    assert_eq!(
        reported_closures(&default),
        reported_closures(&disabled),
        "nonsemantic Cargo incremental mode changed the admitted MIR closures"
    );

    let metadata = run_clean_exact_profiles(
        &workspace,
        &metadata_target,
        None,
        Some("-Cmetadata=fe2o3-provider-disambiguator-regression"),
    );
    let metadata_text = command_text(&metadata);
    assert!(
        metadata.status.success(),
        "clean provider-metadata-varied workgroup-sync run failed:\n{metadata_text}"
    );
    assert_eq!(
        reported_authorities(&default),
        reported_authorities(&metadata),
        "nonsemantic provider crate metadata changed the sealed authorities"
    );
    assert_eq!(
        reported_closures(&default),
        reported_closures(&metadata),
        "nonsemantic provider crate metadata changed the admitted MIR closures"
    );

    let default_helpers = reported_helpers(&default);
    let metadata_helpers = reported_helpers(&metadata);
    assert_eq!(
        default_helpers.keys().collect::<Vec<_>>(),
        metadata_helpers.keys().collect::<Vec<_>>(),
        "provider metadata variation changed the helper-report profile set"
    );
    assert!(
        default_helpers.values().all(|helpers| !helpers.is_empty()),
        "clean exact profiles omitted internal helper exports"
    );
    assert_eq!(
        default_helpers.values().map(Vec::len).collect::<Vec<_>>(),
        metadata_helpers.values().map(Vec::len).collect::<Vec<_>>(),
        "provider metadata variation changed helper-closure cardinality"
    );
    assert_ne!(
        default_helpers, metadata_helpers,
        "provider metadata variation did not change rustc export disambiguators"
    );
}

#[test]
fn exact_lds_source_authenticates_complete_profile() {
    assert_exact_profile_authenticates(ProfileKind::Lds, LDS_SOURCE, "exact-lds");
}

#[test]
fn exact_atomic_source_authenticates_complete_profile() {
    assert_exact_profile_authenticates(ProfileKind::Atomic, ATOMIC_SOURCE, "exact-atomic");
}

#[test]
fn exact_profile_authority_is_location_independent_and_source_bound() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let location_a = output.path.join("canonical-workspace-a");
    let location_b = output.path.join("canonical-workspace-b");
    copy_relocated_workspace(&workspace, &location_a);
    copy_relocated_workspace(&workspace, &location_b);
    let location_a = location_a
        .canonicalize()
        .expect("canonical relocated workspace A");
    let location_b = location_b
        .canonicalize()
        .expect("canonical relocated workspace B");
    assert_ne!(location_a, location_b);

    let target_a = output.path.join("relocated-target-a");
    let target_b = output.path.join("relocated-target-b");
    let first = run_relocated_exact_profiles(&location_a, &target_a);
    let second = run_relocated_exact_profiles(&location_b, &target_b);
    assert_relocated_success("workspace A", &first);
    assert_relocated_success("workspace B", &second);
    assert_eq!(reported_authorities(&first), reported_authorities(&second));

    let thread_source = location_b.join("crates/fe2o3-device/src/thread.rs");
    let mut hostile_source =
        std::fs::read_to_string(&thread_source).expect("read device thread source");
    hostile_source.push_str("\n// hostile provider source substitution\n");
    std::fs::write(&thread_source, hostile_source).expect("mutate relocated device source");
    let hostile = run_relocated_exact_profiles(
        &location_b,
        &output.path.join("relocated-target-hostile-source"),
    );
    let hostile_text = command_text(&hostile);
    assert!(
        !hostile.status.success(),
        "mutated provider source authenticated"
    );
    assert!(
        hostile_text.contains(
            "safe execution provider source closure does not match the reviewed V1 identity"
        ),
        "mutated provider source did not fail at provider source identity:\n{hostile_text}"
    );
}

#[test]
fn hostile_lds_source_and_compiler_mutations_fail_closed() {
    assert_exact_profile_authenticates(ProfileKind::Lds, LDS_SOURCE, "hostile-lds-baseline");
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let sources = [
        ("lds-source-byte", format!("{LDS_SOURCE}\n// hostile\n")),
        (
            "lds-explicit-namespace",
            mutation(
                LDS_SOURCE,
                "    typed,\n",
                "    typed,\n    namespace = \"6bc8f449f458cf8f31b4625b38b7204dd34f20beeabb80b55454a5666be749b5\",\n",
            ),
        ),
        (
            "lds-element",
            mutation(LDS_SOURCE, "DynamicLds::<i32>", "DynamicLds::<u32>"),
        ),
        (
            "lds-extent",
            mutation(
                LDS_SOURCE,
                "DynamicLds::<i32>::exact_current::<64>",
                "DynamicLds::<i32>::exact_current::<32>",
            ),
        ),
        (
            "lds-launch-geometry",
            mutation(
                LDS_SOURCE,
                "launch(required = [64, 1, 1], max = [64, 1, 1])",
                "launch(required = [32, 1, 1], max = [32, 1, 1])",
            ),
        ),
        (
            "lds-collective",
            mutation(
                LDS_SOURCE,
                "group.reduce_sum(&context, &mut scratch, value)",
                "group.inclusive_scan_sum(&context, &mut scratch, value)",
            ),
        ),
        (
            "lds-output-owner",
            mutation(LDS_SOURCE, "if lane == 0 {", "if lane == 1 {"),
        ),
        (
            "lds-output-index",
            mutation(
                LDS_SOURCE,
                "output.get_mut_exclusive(&leader, 0)",
                "output.get_mut_exclusive(&leader, 1)",
            ),
        ),
        (
            "lds-terminal-substitution",
            mutation(
                LDS_SOURCE,
                "let lane = thread::thread_idx_x();",
                "let lane = thread::block_dim_x();",
            ),
        ),
    ];
    for (label, source) in sources {
        let result = compile(
            &workspace,
            &output,
            label,
            &source,
            ProfileKind::Lds,
            CompilerProfile::exact(ProfileKind::Lds),
        );
        assert_rejected(&result, label);
    }
    for (label, profile) in hostile_compiler_profiles(ProfileKind::Lds) {
        let result = compile(
            &workspace,
            &output,
            label,
            LDS_SOURCE,
            ProfileKind::Lds,
            profile,
        );
        assert_rejected(&result, label);
    }
}

#[test]
fn hostile_atomic_source_and_compiler_mutations_fail_closed() {
    assert_exact_profile_authenticates(
        ProfileKind::Atomic,
        ATOMIC_SOURCE,
        "hostile-atomic-baseline",
    );
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let sources = [
        (
            "atomic-source-byte",
            format!("{ATOMIC_SOURCE}\n// hostile\n"),
        ),
        (
            "atomic-explicit-namespace",
            mutation(
                ATOMIC_SOURCE,
                "    typed,\n",
                "    typed,\n    namespace = \"409357ef99d9ec78c960cca0e21a4e153c60af522c1c4d726a9f23b5c7271b91\",\n",
            ),
        ),
        (
            "atomic-ordering",
            mutation(
                ATOMIC_SOURCE,
                ".fetch_add(values[lane], Ordering::Relaxed)",
                ".fetch_add(values[lane], Ordering::SeqCst)",
            ),
        ),
        (
            "atomic-eligibility",
            mutation(ATOMIC_SOURCE, "eligible[lane] != 0", "eligible[lane] == 0"),
        ),
        (
            "atomic-operation",
            mutation(ATOMIC_SOURCE, "fetch_add", "fetch_max"),
        ),
        (
            "atomic-address-space",
            mutation(
                ATOMIC_SOURCE,
                "target: DeviceGlobalMutPtr<u32>",
                "target: fe2o3_device::DeviceWorkgroupMutPtr<u32>",
            ),
        ),
        (
            "atomic-scalar-type",
            mutation(
                ATOMIC_SOURCE,
                "target: DeviceGlobalMutPtr<u32>",
                "target: DeviceGlobalMutPtr<i32>",
            ),
        ),
        (
            "atomic-argument-roles",
            mutation(
                ATOMIC_SOURCE,
                "values: &[u32], eligible: &[u32]",
                "eligible: &[u32], values: &[u32]",
            ),
        ),
        (
            "atomic-terminal-substitution",
            mutation(
                ATOMIC_SOURCE,
                "let lane = thread::index_1d().get();",
                "let lane = thread::block_dim_x() as usize;",
            ),
        ),
    ];
    for (label, source) in sources {
        let result = compile(
            &workspace,
            &output,
            label,
            &source,
            ProfileKind::Atomic,
            CompilerProfile::exact(ProfileKind::Atomic),
        );
        assert_rejected(&result, label);
    }
    for (label, profile) in hostile_compiler_profiles(ProfileKind::Atomic) {
        let result = compile(
            &workspace,
            &output,
            label,
            ATOMIC_SOURCE,
            ProfileKind::Atomic,
            profile,
        );
        assert_rejected(&result, label);
    }
}

fn hostile_compiler_profiles(kind: ProfileKind) -> Vec<(&'static str, CompilerProfile<'static>)> {
    vec![
        (
            "wrong-target",
            CompilerProfile {
                target: "gfx942",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-crate",
            CompilerProfile {
                crate_name: "fe2o3_workgroup_sync_impostor",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-metadata",
            CompilerProfile {
                metadata: "fe2o3-workgroup-sync-unreviewed",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-binding",
            CompilerProfile {
                crate_binding: "aa63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "overflow-policy",
            CompilerProfile {
                overflow_checks: true,
                ..CompilerProfile::exact(kind)
            },
        ),
    ]
}
