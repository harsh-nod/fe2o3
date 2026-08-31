use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::build_config::{PRODUCTION_BUILD_CONFIG_V2_ENV, PreparedProductionBuildConfig};
use crate::capability_broker::{
    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1, SourceIsaObservationCollectionV1,
};
use crate::source_isa_observation::{
    SourceIsaObservationKirVersionV1, SourceIsaObservationOutcomeV1,
    SourceIsaObservationTargetProfileV1,
};
use crate::{
    MAX_SOURCE_ISA_COLLECTION_STDERR_LINE_BYTES_V1, SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1,
};
use serde_json::{Value, json};

const TEST_DRIVER_ENV: &str = "FE2O3_TEST_CARGO_FE2O3_BIN";
const TEST_DRIVER_SHA256_ENV: &str = "FE2O3_TEST_CARGO_FE2O3_SHA256";
const MAX_TEMPLATE_BYTES: u64 = 1024 * 1024;
const MAX_PROTECTED_BUILD_STDERR_BYTES: usize = 1024 * 1024;
const PROTECTED_BUILD_TIMEOUT: Duration = Duration::from_secs(900);
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const AUTHORITY_ENVIRONMENT: [&str; 8] = [
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_BACKEND",
];

#[derive(Debug)]
struct UnitCaseV1 {
    label: &'static str,
    command_directory: PathBuf,
    working_directory: PathBuf,
    crate_name: &'static str,
    source: &'static str,
    cargo_arguments: &'static [&'static str],
    immutable_inputs: &'static [&'static str],
}

#[derive(Debug)]
struct CapturedCollectionV1 {
    encoded: Vec<u8>,
    declared_frames: usize,
    declared_missing: usize,
    declared_failure: u16,
}

#[derive(Debug)]
struct BoundedCommandOutputV1 {
    status: ExitStatus,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StderrDrainStateV1 {
    Open,
    Closed,
    Overflow,
}

#[derive(Clone, Copy, Debug)]
struct ExpectedObservationV1 {
    config: [u8; 32],
    unit: [u8; 32],
    target: SourceIsaObservationTargetProfileV1,
}

#[derive(Debug)]
struct AdmittedCellV1 {
    target: SourceIsaObservationTargetProfileV1,
    encoded: Vec<u8>,
    config: [u8; 32],
    unit: [u8; 32],
    neutral_kir: [u8; 32],
    target_kir: [u8; 32],
    artifact: [u8; 32],
    correlation: [u8; 32],
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-source-isa-unit-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create source/ISA unit scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unit_matrix_protocol_contract_names_only_exact_ordinary_compiler_unit_roots() {
    let workspace = workspace();
    let units = unit_cases(&workspace);
    let roots = units
        .iter()
        .map(|unit| {
            assert!(unit.command_directory.is_dir());
            assert!(unit.working_directory.is_dir());
            assert!(unit.working_directory.join(unit.source).is_file());
            (unit.crate_name, unit.source, unit.cargo_arguments)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        roots,
        BTreeSet::from([
            (
                "fe2o3_collected_tiled_gemm_v1_fixture",
                "src/lib.rs",
                &[][..],
            ),
            (
                "fe2o3_production_extraction_fixture",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
                &["-p", "fe2o3-production-extraction-fixture"][..],
            ),
            (
                "fe2o3_workgroup_sync_v1",
                "src/lib.rs",
                &["--no-default-features", "--features", "lds-kernel"][..],
            ),
        ])
    );
}

#[test]
fn collection_stderr_parser_protocol_contract_is_bounded_and_rejects_hostile_mutations() {
    let valid = format!(
        "unrelated diagnostic\n{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001abff authority=observation-only\n"
    );
    let parsed = parse_collection_line(&valid).expect("parse bounded protocol fixture");
    assert_eq!(parsed.encoded, [0x00, 0x01, 0xab, 0xff]);
    assert_eq!(parsed.declared_frames, 1);
    assert_eq!(parsed.declared_missing, 0);
    assert_eq!(parsed.declared_failure, 0);
    assert!(
        validate_admitted_collection(
            &parsed,
            ExpectedObservationV1 {
                config: [1; 32],
                unit: [2; 32],
                target: SourceIsaObservationTargetProfileV1::Gfx942,
            },
        )
        .is_err(),
        "stderr framing alone must not fabricate an admitted observation",
    );

    for hostile in [
        String::new(),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001ABFF authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:001 authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:00gg authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001 authority=compiler"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} missing=0 frames=1 failure=0 encoding=hex:0001 authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001 authority=observation-only extra=1"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001 authority=observation-only\n{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1 missing=0 failure=0 encoding=hex:0001 authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=01 missing=0 failure=0 encoding=hex:0001 authority=observation-only"
        ),
        format!(
            "{SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1} frames=1  missing=0 failure=0 encoding=hex:0001 authority=observation-only"
        ),
    ] {
        assert!(
            parse_collection_line(&hostile).is_err(),
            "accepted {hostile:?}"
        );
    }

    let over_bound = "aa".repeat(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1 / 2 + 1);
    assert!(decode_lower_hex(&over_bound).is_err());
}

#[test]
fn bounded_command_does_not_join_an_escaped_descendant_holding_stderr() {
    let setsid = [Path::new("/usr/bin/setsid"), Path::new("/bin/setsid")]
        .into_iter()
        .find(|path| path.is_file())
        .expect("Linux protected-build tests require setsid");
    let scratch = ScratchDirectory::new("escaped-stderr-holder");
    let pid_path = scratch.0.join("escaped-pgid");
    let script = format!(
        "{} /bin/sh -c 'printf \"%s\\n\" \"$$\" > \"$1\"; sleep 30' escaped \"$1\" &",
        setsid.display(),
    );
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(script).arg("outer").arg(&pid_path);

    let started = Instant::now();
    let error = run_bounded_command(&mut command).unwrap_err();
    let elapsed = started.elapsed();
    let escaped_process_group = read_escaped_process_group(&pid_path);
    // SAFETY: the hostile child published the positive PID returned by setsid before sleeping.
    let _ = unsafe { libc::kill(-escaped_process_group, libc::SIGKILL) };

    assert_eq!(
        error,
        "protected build descendants retained the stderr pipe"
    );
    assert!(
        elapsed < PIPE_CLOSE_TIMEOUT + Duration::from_secs(3),
        "bounded command waited {elapsed:?} for an escaped stderr holder",
    );
}

#[test]
#[ignore = "requires the protected production V2 authority service and a measured real Worker V3"]
fn ordinary_source_units_round_trip_through_the_production_observer_on_both_targets() {
    // V1 proves exact unit/target binding plus one bidirectional source/ISA witness. It does not
    // retain the target-KIR operation coordinate needed to claim distinctive collective or matrix
    // preservation. That T1 exit requires a separately versioned characteristic-witness contract.
    let workspace = workspace();
    let units = unit_cases(&workspace);
    let driver = measured_test_driver();
    let template_path = PathBuf::from(
        std::env::var_os(PRODUCTION_BUILD_CONFIG_V2_ENV)
            .expect("set FE2O3_PRODUCTION_BUILD_CONFIG_V2 to a canonical protected V2 template"),
    );
    let template = read_bounded_template(&template_path);
    let mut admitted = Vec::with_capacity(units.len() * 2);

    for unit in &units {
        let source_snapshot = snapshot_sources(unit);
        let config_scratch = ScratchDirectory::new(unit.label);
        let config_path = write_case_config(
            &config_scratch.0,
            &template,
            unit,
            unit.crate_name,
            unit.source,
        );
        let config = PreparedProductionBuildConfig::from_v2_manifest_for_test(&config_path)
            .expect("admit exact production V2 unit config");
        assert!(config.source_isa_summary_enabled());
        let expected_unit = config
            .source_isa_unit_identity(
                unit.crate_name,
                Path::new(unit.source),
                &unit.working_directory,
            )
            .expect("V2 config selects its exact ordinary-source unit");
        let expected_config = *config.identity().as_bytes();

        let substituted_path = write_case_config(
            &config_scratch.0,
            &template,
            unit,
            &format!("{}_substituted", unit.crate_name),
            unit.source,
        );
        let substituted =
            PreparedProductionBuildConfig::from_v2_manifest_for_test(&substituted_path)
                .expect("admit syntactically valid substituted V2 config");
        assert_ne!(substituted.identity(), config.identity());
        let substituted_unit = substituted
            .source_isa_unit_identity(
                &format!("{}_substituted", unit.crate_name),
                Path::new(unit.source),
                &unit.working_directory,
            )
            .expect("substituted V2 config selects only its substituted unit");
        assert_ne!(substituted_unit, expected_unit);

        for (cpu, target) in [
            ("gfx942", SourceIsaObservationTargetProfileV1::Gfx942),
            ("gfx950", SourceIsaObservationTargetProfileV1::Gfx950),
        ] {
            let build_scratch = ScratchDirectory::new(&format!("{}-{cpu}", unit.label));
            let captured = run_protected_cell(&driver, unit, cpu, &config_path, &build_scratch.0);
            assert_sources_unchanged(&source_snapshot);
            let expected = ExpectedObservationV1 {
                config: expected_config,
                unit: *expected_unit.as_bytes(),
                target,
            };
            let cell = validate_admitted_collection(&captured, expected)
                .unwrap_or_else(|error| panic!("{} {cpu} observation failed: {error}", unit.label));

            let substituted_expected = ExpectedObservationV1 {
                config: *substituted.identity().as_bytes(),
                unit: *substituted_unit.as_bytes(),
                target,
            };
            assert_eq!(
                validate_admitted_collection(&captured, substituted_expected).unwrap_err(),
                "collection config identity substitution",
                "{} {cpu} substitution rejection changed",
                unit.label
            );
            assert_bounded_outer_integrity_mutations_fail(&cell.encoded, unit.label, cpu);
            admitted.push(cell);
        }
    }

    assert_eq!(admitted.len(), 6);
    for pair in admitted.chunks_exact(2) {
        let [gfx942, gfx950] = pair else {
            unreachable!()
        };
        assert!(matches!(
            gfx942.target,
            SourceIsaObservationTargetProfileV1::Gfx942
        ));
        assert!(matches!(
            gfx950.target,
            SourceIsaObservationTargetProfileV1::Gfx950
        ));
        assert_eq!(gfx942.config, gfx950.config);
        assert_eq!(gfx942.unit, gfx950.unit);
        assert_eq!(gfx942.neutral_kir, gfx950.neutral_kir);
        assert_ne!(gfx942.target_kir, gfx950.target_kir);
        assert_ne!(gfx942.artifact, gfx950.artifact);
        assert_ne!(gfx942.correlation, gfx950.correlation);
        assert_eq!(
            validate_admitted_collection(
                &CapturedCollectionV1 {
                    encoded: gfx942.encoded.clone(),
                    declared_frames: 1,
                    declared_missing: 0,
                    declared_failure: 0,
                },
                ExpectedObservationV1 {
                    config: gfx942.config,
                    unit: gfx942.unit,
                    target: gfx950.target,
                },
            )
            .unwrap_err(),
            "target or KIR-version substitution",
            "target-profile substitution rejection changed",
        );
    }

    let neutral_identities = admitted
        .chunks_exact(2)
        .map(|pair| pair[0].neutral_kir)
        .collect::<BTreeSet<_>>();
    assert_eq!(neutral_identities.len(), 3);
    let unit_identities = admitted
        .chunks_exact(2)
        .map(|pair| pair[0].unit)
        .collect::<BTreeSet<_>>();
    assert_eq!(unit_identities.len(), 3);

    let wrong_unit = ExpectedObservationV1 {
        config: admitted[2].config,
        unit: admitted[2].unit,
        target: admitted[0].target,
    };
    assert_eq!(
        validate_admitted_collection(
            &CapturedCollectionV1 {
                encoded: admitted[0].encoded.clone(),
                declared_frames: 1,
                declared_missing: 0,
                declared_failure: 0,
            },
            wrong_unit,
        )
        .unwrap_err(),
        "collection config identity substitution",
        "cross-unit config/unit substitution rejection changed",
    );
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn unit_cases(workspace: &Path) -> [UnitCaseV1; 3] {
    let reduction = "examples/workgroup_sync_v1";
    let tiled = "crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-v1";
    [
        UnitCaseV1 {
            label: "elementwise-fill",
            command_directory: workspace.to_path_buf(),
            working_directory: workspace.to_path_buf(),
            crate_name: "fe2o3_production_extraction_fixture",
            source: "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
            cargo_arguments: &["-p", "fe2o3-production-extraction-fixture"],
            immutable_inputs: &[
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/Cargo.toml",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
            ],
        },
        UnitCaseV1 {
            label: "neutral-workgroup-reduction",
            command_directory: workspace.join(reduction),
            working_directory: workspace.join(reduction),
            crate_name: "fe2o3_workgroup_sync_v1",
            source: "src/lib.rs",
            cargo_arguments: &["--no-default-features", "--features", "lds-kernel"],
            immutable_inputs: &["Cargo.lock", "Cargo.toml", "src/lib.rs", "src/kernel.rs"],
        },
        UnitCaseV1 {
            label: "tiled-bf16-gemm",
            command_directory: workspace.join(tiled),
            working_directory: workspace.join(tiled),
            crate_name: "fe2o3_collected_tiled_gemm_v1_fixture",
            source: "src/lib.rs",
            cargo_arguments: &[],
            immutable_inputs: &["Cargo.lock", "Cargo.toml", "src/lib.rs"],
        },
    ]
}

fn measured_test_driver() -> crate::pinned_executable::PinnedExecutable {
    let declared = PathBuf::from(
        std::env::var_os(TEST_DRIVER_ENV)
            .unwrap_or_else(|| panic!("set {TEST_DRIVER_ENV} to the measured production CLI")),
    );
    assert!(declared.is_absolute(), "{TEST_DRIVER_ENV} must be absolute");
    assert_eq!(declared.file_name(), Some(OsStr::new("cargo-fe2o3")));
    let metadata = declared
        .symlink_metadata()
        .expect("inspect measured cargo-fe2o3 CLI");
    assert!(metadata.file_type().is_file() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    let expected = parse_sha256(
        &std::env::var_os(TEST_DRIVER_SHA256_ENV)
            .unwrap_or_else(|| panic!("set {TEST_DRIVER_SHA256_ENV}")),
    );
    let opened = crate::pinned_executable::PinnedExecutable::open(&declared)
        .unwrap_or_else(|error| panic!("open measured cargo-fe2o3 CLI by descriptor: {error}"));
    assert_eq!(
        opened.sha256(),
        &expected,
        "measured cargo-fe2o3 CLI changed"
    );
    let sealed = opened
        .seal_executable_image()
        .unwrap_or_else(|error| panic!("seal measured cargo-fe2o3 CLI image: {error}"));
    assert_eq!(sealed.sha256(), &expected, "sealed cargo-fe2o3 CLI changed");
    sealed
}

fn parse_sha256(value: &OsStr) -> [u8; 32] {
    let value = value
        .to_str()
        .unwrap_or_else(|| panic!("{TEST_DRIVER_SHA256_ENV} is not UTF-8"));
    assert_eq!(value.len(), 64, "{TEST_DRIVER_SHA256_ENV} length");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{TEST_DRIVER_SHA256_ENV} is not hexadecimal",
    );
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .expect("decode measured CLI SHA-256");
    }
    output
}

fn read_bounded_template(path: &Path) -> Value {
    assert!(
        path.is_absolute(),
        "production V2 template path must be absolute"
    );
    let metadata = path
        .symlink_metadata()
        .expect("inspect production V2 template");
    assert!(metadata.file_type().is_file() && !metadata.file_type().is_symlink());
    assert!(
        metadata.len() > 0 && metadata.len() <= MAX_TEMPLATE_BYTES,
        "production V2 template exceeds its bound",
    );
    let bytes = fs::read(path).expect("read production V2 template");
    let value: Value = serde_json::from_slice(&bytes).expect("parse production V2 template");
    assert_eq!(value["format"], "fe2o3-production-build-config-v2");
    assert_eq!(
        value["observation"],
        json!({"kind": "source-isa-summary-v1"})
    );
    value
}

fn write_case_config(
    directory: &Path,
    template: &Value,
    unit: &UnitCaseV1,
    crate_name: &str,
    source: &str,
) -> PathBuf {
    let mut value = template.clone();
    value["units"] = json!([{
        "crate_name": crate_name,
        "source": source,
        "working_directory": unit.working_directory.to_str().expect("UTF-8 working directory"),
    }]);
    let name = if crate_name == unit.crate_name {
        "production-v2.json"
    } else {
        "substituted-production-v2.json"
    };
    let path = directory.join(name);
    fs::write(
        &path,
        serde_json::to_vec(&value).expect("encode canonical production V2 config"),
    )
    .expect("write production V2 unit config");
    path
}

fn snapshot_sources(unit: &UnitCaseV1) -> BTreeMap<PathBuf, Vec<u8>> {
    unit.immutable_inputs
        .iter()
        .map(|relative| {
            let path = unit.working_directory.join(relative);
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("read ordinary source {}: {error}", path.display()));
            (path, bytes)
        })
        .collect()
}

fn assert_sources_unchanged(snapshot: &BTreeMap<PathBuf, Vec<u8>>) {
    for (path, expected) in snapshot {
        assert_eq!(
            fs::read(path).unwrap_or_else(|error| panic!("re-read {}: {error}", path.display())),
            expected.as_slice(),
            "protected observer build changed ordinary source {}",
            path.display(),
        );
    }
}

fn run_protected_cell(
    driver: &crate::pinned_executable::PinnedExecutable,
    unit: &UnitCaseV1,
    cpu: &str,
    config: &Path,
    target_directory: &Path,
) -> CapturedCollectionV1 {
    let mut pinned_command = driver
        .command()
        .unwrap_or_else(|error| panic!("prepare sealed cargo-fe2o3 CLI command: {error}"));
    let command = pinned_command.as_command_mut();
    command
        .current_dir(&unit.command_directory)
        .env_clear()
        .env("CARGO", env!("CARGO"))
        .env("FE2O3_TARGET", cpu)
        .env(PRODUCTION_BUILD_CONFIG_V2_ENV, config)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    for name in AUTHORITY_ENVIRONMENT {
        command.env(
            name,
            std::env::var_os(name)
                .unwrap_or_else(|| panic!("protected observer matrix requires {name}")),
        );
    }
    command.args([
        "authority",
        "release",
        "build",
        "--release",
        "--locked",
        "--target-dir",
    ]);
    command.arg(target_directory.join("cargo"));
    command.args(unit.cargo_arguments);
    command.arg("--lib");
    let output = run_bounded_command(command)
        .unwrap_or_else(|error| panic!("run protected production V2 build: {error}"));
    let stderr = String::from_utf8(output.stderr).expect("protected build stderr is UTF-8");
    assert!(
        output.status.success(),
        "protected {} {cpu} build failed:\n{stderr}",
        unit.label,
    );
    parse_collection_line(&stderr).unwrap_or_else(|error| {
        panic!(
            "protected {} {cpu} build omitted a valid observation: {error}\n{stderr}",
            unit.label,
        )
    })
}

fn run_bounded_command(command: &mut Command) -> Result<BoundedCommandOutputV1, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: this callback invokes only the async-signal-safe setpgid syscall.
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }
    let mut child = crate::process_execution::spawn(command)
        .map_err(|error| format!("spawn protected build: {error}"))?;
    let process_group = child.id() as libc::pid_t;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "protected build omitted its stderr pipe".to_owned())?;
    if let Err(error) = set_nonblocking(&stderr) {
        terminate_and_reap(&mut child, process_group);
        return Err(error);
    }
    let mut retained = Vec::new();
    let mut stderr_closed = false;

    let deadline = Instant::now() + PROTECTED_BUILD_TIMEOUT;
    let status = loop {
        match drain_nonblocking_stderr(&mut stderr, &mut retained) {
            Ok(StderrDrainStateV1::Open) => {}
            Ok(StderrDrainStateV1::Closed) => stderr_closed = true,
            Ok(StderrDrainStateV1::Overflow) => {
                terminate_and_reap(&mut child, process_group);
                return Err(format!(
                    "protected build stderr exceeded {MAX_PROTECTED_BUILD_STDERR_BYTES} bytes"
                ));
            }
            Err(error) => {
                terminate_and_reap(&mut child, process_group);
                return Err(error);
            }
        }
        let polled = match child.try_wait() {
            Ok(polled) => polled,
            Err(error) => {
                terminate_and_reap(&mut child, process_group);
                return Err(format!("poll protected build: {error}"));
            }
        };
        if let Some(status) = polled {
            break status;
        }
        if Instant::now() >= deadline {
            terminate_and_reap(&mut child, process_group);
            return Err(format!(
                "protected build exceeded its {} second timeout",
                PROTECTED_BUILD_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let pipe_deadline = Instant::now() + PIPE_CLOSE_TIMEOUT;
    while !stderr_closed {
        let state = drain_nonblocking_stderr(&mut stderr, &mut retained).map_err(|error| {
            kill_process_group(process_group);
            error
        })?;
        match state {
            StderrDrainStateV1::Open => {}
            StderrDrainStateV1::Closed => {
                stderr_closed = true;
                continue;
            }
            StderrDrainStateV1::Overflow => {
                kill_process_group(process_group);
                return Err(format!(
                    "protected build stderr exceeded {MAX_PROTECTED_BUILD_STDERR_BYTES} bytes"
                ));
            }
        }
        if Instant::now() >= pipe_deadline {
            kill_process_group(process_group);
            return Err("protected build descendants retained the stderr pipe".to_owned());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(BoundedCommandOutputV1 {
        status,
        stderr: retained,
    })
}

fn set_nonblocking(stderr: &std::process::ChildStderr) -> Result<(), String> {
    let descriptor = stderr.as_raw_fd();
    // SAFETY: fcntl observes and updates flags on the owned live stderr descriptor.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags == -1
        || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(format!(
            "make protected build stderr nonblocking: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn drain_nonblocking_stderr(
    stderr: &mut std::process::ChildStderr,
    retained: &mut Vec<u8>,
) -> Result<StderrDrainStateV1, String> {
    let mut buffer = [0_u8; 8192];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => return Ok(StderrDrainStateV1::Closed),
            Ok(count) => {
                let remaining = MAX_PROTECTED_BUILD_STDERR_BYTES.saturating_sub(retained.len());
                retained.extend_from_slice(&buffer[..count.min(remaining)]);
                if count > remaining {
                    return Ok(StderrDrainStateV1::Overflow);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(StderrDrainStateV1::Open);
            }
            Err(error) => return Err(format!("read protected build stderr: {error}")),
        }
    }
}

fn terminate_and_reap(child: &mut std::process::Child, process_group: libc::pid_t) {
    kill_process_group(process_group);
    let _ = child.wait();
}

fn kill_process_group(process_group: libc::pid_t) {
    // SAFETY: the child created a fresh process group whose positive id was retained above.
    let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
}

fn read_escaped_process_group(path: &Path) -> libc::pid_t {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(value) = fs::read_to_string(path) {
            let pid = value
                .trim()
                .parse::<libc::pid_t>()
                .expect("escaped process group PID is decimal");
            assert!(pid > 0, "escaped process group PID must be positive");
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "escaped descendant did not publish its process group"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_collection_line(stderr: &str) -> Result<CapturedCollectionV1, String> {
    let lines = stderr
        .lines()
        .filter(|line| line.starts_with(SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1))
        .collect::<Vec<_>>();
    let [line] = lines.as_slice() else {
        return Err(format!(
            "expected one collection line, found {}",
            lines.len()
        ));
    };
    if line.len() > MAX_SOURCE_ISA_COLLECTION_STDERR_LINE_BYTES_V1 {
        return Err("collection line exceeds its canonical bound".to_owned());
    }
    let suffix = line
        .strip_prefix(SOURCE_ISA_COLLECTION_STDERR_PREFIX_V1)
        .and_then(|suffix| suffix.strip_prefix(' '))
        .ok_or_else(|| "collection line has a noncanonical prefix".to_owned())?;
    let fields = suffix.split_ascii_whitespace().collect::<Vec<_>>();
    let [frames, missing, failure, encoding, authority] = fields.as_slice() else {
        return Err("collection line has a noncanonical field count".to_owned());
    };
    let frames = parse_usize_field(frames, "frames")?;
    let missing = parse_usize_field(missing, "missing")?;
    let failure = parse_u16_field(failure, "failure")?;
    if *authority != "authority=observation-only" {
        return Err("collection line changed its authority label".to_owned());
    }
    let encoded = encoding
        .strip_prefix("encoding=hex:")
        .ok_or_else(|| "collection line lacks its exact hex encoding".to_owned())?;
    let canonical = format!(
        "frames={frames} missing={missing} failure={failure} encoding=hex:{encoded} authority=observation-only"
    );
    if suffix != canonical {
        return Err("collection line has noncanonical spacing or decimal fields".to_owned());
    }
    Ok(CapturedCollectionV1 {
        encoded: decode_lower_hex(encoded)?,
        declared_frames: frames,
        declared_missing: missing,
        declared_failure: failure,
    })
}

fn parse_usize_field(field: &str, name: &str) -> Result<usize, String> {
    field
        .strip_prefix(&format!("{name}="))
        .ok_or_else(|| format!("missing {name} field"))?
        .parse()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn parse_u16_field(field: &str, name: &str) -> Result<u16, String> {
    field
        .strip_prefix(&format!("{name}="))
        .ok_or_else(|| format!("missing {name} field"))?
        .parse()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn decode_lower_hex(encoded: &str) -> Result<Vec<u8>, String> {
    if encoded.is_empty()
        || encoded.len() > MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1
        || !encoded.len().is_multiple_of(2)
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("collection encoding is not bounded canonical lowercase hex".to_owned());
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex pair is ASCII");
            u8::from_str_radix(pair, 16).map_err(|error| format!("invalid hex pair: {error}"))
        })
        .collect()
}

fn validate_admitted_collection(
    captured: &CapturedCollectionV1,
    expected: ExpectedObservationV1,
) -> Result<AdmittedCellV1, String> {
    let collection = SourceIsaObservationCollectionV1::decode_canonical(&captured.encoded)?;
    if collection.encode_canonical()? != captured.encoded {
        return Err("collection is not a canonical round trip".to_owned());
    }
    if collection.config_identity() != expected.config {
        return Err("collection config identity substitution".to_owned());
    }
    if collection.missing_units().len() != captured.declared_missing
        || collection.frames().len() != captured.declared_frames
        || collection.failure().map_or(0, |failure| failure.code()) != captured.declared_failure
    {
        return Err("collection text summary differs from canonical bytes".to_owned());
    }
    if captured.declared_frames != 1
        || captured.declared_missing != 0
        || captured.declared_failure != 0
        || !collection.missing_units().is_empty()
        || collection.failure().is_some()
    {
        return Err("production observer did not return exactly one successful cell".to_owned());
    }
    if collection.grants_compiler_authority()
        || collection.grants_publication_authority()
        || collection.grants_runtime_authority()
    {
        return Err("observer collection acquired authority".to_owned());
    }
    let mut frames = collection.frames();
    let frame = frames
        .next()
        .ok_or_else(|| "missing production observer frame".to_owned())?;
    if frames.next().is_some() {
        return Err("multiple production observer frames".to_owned());
    }
    let context = frame.context();
    if context.config() != expected.config || context.unit() != expected.unit {
        return Err("frame config/unit substitution".to_owned());
    }
    if frame.identity() == [0; 32] || context.finalization() == [0; 32] {
        return Err("frame retained a zero evidence identity".to_owned());
    }
    let SourceIsaObservationOutcomeV1::Admitted(observation) = frame.outcome() else {
        return Err(format!(
            "source/ISA round trip was not admitted: {:?}",
            frame.outcome()
        ));
    };
    let structural = observation.structural();
    if structural.target_profile() != expected.target
        || structural.kir_version() != SourceIsaObservationKirVersionV1::V8
    {
        return Err("target or KIR-version substitution".to_owned());
    }
    let structure = structural.counts();
    if structure.functions == 0
        || structure.defined_bodies != 1
        || structure.blocks == 0
        || structure.operations == 0
    {
        return Err("observer did not retain one nonempty single-kernel body".to_owned());
    }
    let counts = observation.counts();
    let records = counts.records();
    let queries = counts.queries();
    if records.records
        != records
            .source_anchored
            .checked_add(records.eliminated)
            .and_then(|count| count.checked_add(records.no_source))
            .ok_or_else(|| "observer record counts overflowed".to_owned())?
        || records.source_anchored == 0
        || records.isa_references == 0
        || queries.distinct_source_nodes == 0
        || queries.distinct_source_spans == 0
        || queries.distinct_isa_points == 0
    {
        return Err("observer omitted a nonempty bidirectional source/ISA population".to_owned());
    }
    let witness = observation
        .round_trip_witness()
        .ok_or_else(|| "observer omitted its exact round-trip witness".to_owned())?;
    if witness.source_node_identity() == [0; 32]
        || witness.source_span().file_identity() == [0; 32]
        || witness.source_span().byte_start() >= witness.source_span().byte_end()
        || witness.source_span().line() == 0
        || witness.source_span().column() == 0
        || witness.isa_point().kernel_ordinal() != 0
        || !witness.isa_point().symbol_relative_pc().is_multiple_of(4)
        || witness.source_node_query_matches() == 0
        || witness.source_span_query_matches() == 0
        || witness.isa_point_query_matches() == 0
        || witness.source_node_query_matches() > queries.max_source_node_cardinality
        || witness.source_span_query_matches() > queries.max_source_span_cardinality
        || witness.isa_point_query_matches() > queries.max_exact_pc_cardinality
    {
        return Err("observer retained an invalid round-trip witness".to_owned());
    }
    Ok(AdmittedCellV1 {
        target: expected.target,
        encoded: captured.encoded.clone(),
        config: expected.config,
        unit: expected.unit,
        neutral_kir: structural.neutral_kir().sha256(),
        target_kir: structural.target_kir().sha256(),
        artifact: observation.artifact().sha256(),
        correlation: observation.correlation(),
    })
}

fn assert_bounded_outer_integrity_mutations_fail(encoded: &[u8], unit: &str, cpu: &str) {
    // These flips exercise the outer collection integrity seal. Resealed nested semantic and
    // frame mutations are covered by capability_broker and source_isa_observation decoder tests.
    let offsets = [0, 31, 79, 80, encoded.len() - 33, encoded.len() - 1];
    for offset in offsets.into_iter().collect::<BTreeSet<_>>() {
        let mut mutated = encoded.to_vec();
        mutated[offset] ^= 1;
        assert!(
            SourceIsaObservationCollectionV1::decode_canonical(&mutated).is_err(),
            "{unit} {cpu} accepted wire mutation at byte {offset}",
        );
    }
}
