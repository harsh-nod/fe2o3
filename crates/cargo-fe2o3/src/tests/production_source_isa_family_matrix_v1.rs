use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read as _};
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

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

#[derive(Clone, Copy, Debug)]
enum FamilyKindV1 {
    ElementwiseFill,
    NeutralWorkgroupReduction,
    TiledBf16Gemm,
}

#[derive(Debug)]
struct FamilyCaseV1 {
    kind: FamilyKindV1,
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

struct BoundedCommandOutputV1 {
    status: ExitStatus,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ExpectedObservationV1 {
    config: [u8; 32],
    unit: [u8; 32],
    target: SourceIsaObservationTargetProfileV1,
}

#[derive(Debug)]
struct AdmittedCellV1 {
    family: FamilyKindV1,
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
            "fe2o3-source-isa-family-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create source/ISA family scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn family_matrix_protocol_contract_names_only_exact_ordinary_compiler_unit_roots() {
    let workspace = workspace();
    let families = family_cases(&workspace);
    let roots = families
        .iter()
        .map(|family| {
            assert!(family.command_directory.is_dir());
            assert!(family.working_directory.is_dir());
            assert!(family.working_directory.join(family.source).is_file());
            (family.crate_name, family.source, family.cargo_arguments)
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
            FamilyKindV1::ElementwiseFill,
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
#[ignore = "requires the protected production V2 authority service and a measured real Worker V3"]
fn ordinary_source_families_round_trip_through_the_production_observer_on_both_targets() {
    let workspace = workspace();
    let families = family_cases(&workspace);
    let driver = measured_test_driver();
    let template_path = PathBuf::from(
        std::env::var_os(PRODUCTION_BUILD_CONFIG_V2_ENV)
            .expect("set FE2O3_PRODUCTION_BUILD_CONFIG_V2 to a canonical protected V2 template"),
    );
    let template = read_bounded_template(&template_path);
    let mut admitted = Vec::with_capacity(families.len() * 2);

    for family in &families {
        let source_snapshot = snapshot_sources(family);
        let config_scratch = ScratchDirectory::new(family.label);
        let config_path = write_case_config(
            &config_scratch.0,
            &template,
            family,
            family.crate_name,
            family.source,
        );
        let config = PreparedProductionBuildConfig::from_v2_manifest_for_test(&config_path)
            .expect("admit exact production V2 family config");
        assert!(config.source_isa_summary_enabled());
        let expected_unit = config
            .source_isa_unit_identity(
                family.crate_name,
                Path::new(family.source),
                &family.working_directory,
            )
            .expect("V2 family config selects its exact ordinary-source unit");
        let expected_config = *config.identity().as_bytes();

        let substituted_path = write_case_config(
            &config_scratch.0,
            &template,
            family,
            &format!("{}_substituted", family.crate_name),
            family.source,
        );
        let substituted =
            PreparedProductionBuildConfig::from_v2_manifest_for_test(&substituted_path)
                .expect("admit syntactically valid substituted V2 config");
        assert_ne!(substituted.identity(), config.identity());
        let substituted_unit = substituted
            .source_isa_unit_identity(
                &format!("{}_substituted", family.crate_name),
                Path::new(family.source),
                &family.working_directory,
            )
            .expect("substituted V2 config selects only its substituted unit");
        assert_ne!(substituted_unit, expected_unit);

        for (cpu, target) in [
            ("gfx942", SourceIsaObservationTargetProfileV1::Gfx942),
            ("gfx950", SourceIsaObservationTargetProfileV1::Gfx950),
        ] {
            let build_scratch = ScratchDirectory::new(&format!("{}-{cpu}", family.label));
            let captured = run_protected_cell(&driver, family, cpu, &config_path, &build_scratch.0);
            assert_sources_unchanged(&source_snapshot);
            let expected = ExpectedObservationV1 {
                config: expected_config,
                unit: *expected_unit.as_bytes(),
                target,
            };
            let cell = validate_admitted_collection(family.kind, &captured, expected)
                .unwrap_or_else(|error| {
                    panic!("{} {cpu} observation failed: {error}", family.label)
                });

            let substituted_expected = ExpectedObservationV1 {
                config: *substituted.identity().as_bytes(),
                unit: *substituted_unit.as_bytes(),
                target,
            };
            assert_eq!(
                validate_admitted_collection(family.kind, &captured, substituted_expected)
                    .unwrap_err(),
                "collection config identity substitution",
                "{} {cpu} substitution rejection changed",
                family.label
            );
            assert_bounded_outer_integrity_mutations_fail(&cell.encoded, family.label, cpu);
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
                gfx942.family,
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
    let family_units = admitted
        .chunks_exact(2)
        .map(|pair| pair[0].unit)
        .collect::<BTreeSet<_>>();
    assert_eq!(family_units.len(), 3);

    let wrong_family = ExpectedObservationV1 {
        config: admitted[2].config,
        unit: admitted[2].unit,
        target: admitted[0].target,
    };
    assert_eq!(
        validate_admitted_collection(
            admitted[0].family,
            &CapturedCollectionV1 {
                encoded: admitted[0].encoded.clone(),
                declared_frames: 1,
                declared_missing: 0,
                declared_failure: 0,
            },
            wrong_family,
        )
        .unwrap_err(),
        "collection config identity substitution",
        "family/config/unit substitution rejection changed",
    );
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn family_cases(workspace: &Path) -> [FamilyCaseV1; 3] {
    let reduction = "examples/workgroup_sync_v1";
    let tiled = "crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-v1";
    [
        FamilyCaseV1 {
            kind: FamilyKindV1::ElementwiseFill,
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
        FamilyCaseV1 {
            kind: FamilyKindV1::NeutralWorkgroupReduction,
            label: "neutral-workgroup-reduction",
            command_directory: workspace.join(reduction),
            working_directory: workspace.join(reduction),
            crate_name: "fe2o3_workgroup_sync_v1",
            source: "src/lib.rs",
            cargo_arguments: &["--no-default-features", "--features", "lds-kernel"],
            immutable_inputs: &["Cargo.lock", "Cargo.toml", "src/lib.rs", "src/kernel.rs"],
        },
        FamilyCaseV1 {
            kind: FamilyKindV1::TiledBf16Gemm,
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

fn measured_test_driver() -> PathBuf {
    let declared = PathBuf::from(
        std::env::var_os(TEST_DRIVER_ENV)
            .unwrap_or_else(|| panic!("set {TEST_DRIVER_ENV} to the measured production CLI")),
    );
    assert!(declared.is_absolute(), "{TEST_DRIVER_ENV} must be absolute");
    let driver = declared
        .canonicalize()
        .unwrap_or_else(|error| panic!("canonicalize {TEST_DRIVER_ENV}: {error}"));
    assert_eq!(
        driver, declared,
        "{TEST_DRIVER_ENV} must already be canonical"
    );
    assert_eq!(driver.file_name(), Some(OsStr::new("cargo-fe2o3")));
    let metadata = driver
        .symlink_metadata()
        .expect("inspect measured cargo-fe2o3 CLI");
    assert!(metadata.file_type().is_file() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    let expected = parse_sha256(
        &std::env::var_os(TEST_DRIVER_SHA256_ENV)
            .unwrap_or_else(|| panic!("set {TEST_DRIVER_SHA256_ENV}")),
    );
    let actual: [u8; 32] = Sha256::digest(fs::read(&driver).expect("read measured CLI")).into();
    assert_eq!(actual, expected, "measured cargo-fe2o3 CLI changed");
    driver
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
    family: &FamilyCaseV1,
    crate_name: &str,
    source: &str,
) -> PathBuf {
    let mut value = template.clone();
    value["units"] = json!([{
        "crate_name": crate_name,
        "source": source,
        "working_directory": family.working_directory.to_str().expect("UTF-8 working directory"),
    }]);
    let name = if crate_name == family.crate_name {
        "production-v2.json"
    } else {
        "substituted-production-v2.json"
    };
    let path = directory.join(name);
    fs::write(
        &path,
        serde_json::to_vec(&value).expect("encode canonical production V2 config"),
    )
    .expect("write production V2 family config");
    path
}

fn snapshot_sources(family: &FamilyCaseV1) -> BTreeMap<PathBuf, Vec<u8>> {
    family
        .immutable_inputs
        .iter()
        .map(|relative| {
            let path = family.working_directory.join(relative);
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
    driver: &Path,
    family: &FamilyCaseV1,
    cpu: &str,
    config: &Path,
    target_directory: &Path,
) -> CapturedCollectionV1 {
    let mut command = Command::new(driver);
    command
        .current_dir(&family.command_directory)
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
    command.args(family.cargo_arguments);
    command.arg("--lib");
    let output = run_bounded_command(&mut command)
        .unwrap_or_else(|error| panic!("run protected production V2 build: {error}"));
    let stderr = String::from_utf8(output.stderr).expect("protected build stderr is UTF-8");
    assert!(
        output.status.success(),
        "protected {} {cpu} build failed:\n{stderr}",
        family.label,
    );
    parse_collection_line(&stderr).unwrap_or_else(|error| {
        panic!(
            "protected {} {cpu} build omitted a valid observation: {error}\n{stderr}",
            family.label,
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
    let overflowed = Arc::new(AtomicBool::new(false));
    let reader_overflowed = Arc::clone(&overflowed);
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut retained = Vec::new();
        let mut buffer = [0_u8; 8192];
        let result = loop {
            match stderr.read(&mut buffer) {
                Ok(0) => break Ok(retained),
                Ok(count) => {
                    let remaining = MAX_PROTECTED_BUILD_STDERR_BYTES.saturating_sub(retained.len());
                    retained.extend_from_slice(&buffer[..count.min(remaining)]);
                    if count > remaining {
                        reader_overflowed.store(true, Ordering::Release);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => break Err(error),
            }
        };
        let _ = sender.send(result);
    });

    let deadline = Instant::now() + PROTECTED_BUILD_TIMEOUT;
    let status = loop {
        if overflowed.load(Ordering::Acquire) {
            kill_process_group(process_group);
            let _ = child.wait();
            let _ = receiver.recv_timeout(PIPE_CLOSE_TIMEOUT);
            let _ = reader.join();
            return Err(format!(
                "protected build stderr exceeded {MAX_PROTECTED_BUILD_STDERR_BYTES} bytes"
            ));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("poll protected build: {error}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            kill_process_group(process_group);
            let _ = child.wait();
            let _ = receiver.recv_timeout(PIPE_CLOSE_TIMEOUT);
            let _ = reader.join();
            return Err(format!(
                "protected build exceeded its {} second timeout",
                PROTECTED_BUILD_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let captured = match receiver.recv_timeout(PIPE_CLOSE_TIMEOUT) {
        Ok(result) => result.map_err(|error| format!("read protected build stderr: {error}"))?,
        Err(_) => {
            kill_process_group(process_group);
            let _ = receiver.recv_timeout(PIPE_CLOSE_TIMEOUT);
            let _ = reader.join();
            return Err("protected build descendants retained the stderr pipe".to_owned());
        }
    };
    reader
        .join()
        .map_err(|_| "protected build stderr reader panicked".to_owned())?;
    if overflowed.load(Ordering::Acquire) {
        return Err(format!(
            "protected build stderr exceeded {MAX_PROTECTED_BUILD_STDERR_BYTES} bytes"
        ));
    }
    Ok(BoundedCommandOutputV1 {
        status,
        stderr: captured,
    })
}

fn kill_process_group(process_group: libc::pid_t) {
    // SAFETY: the child created a fresh process group whose positive id was retained above.
    let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
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
    family: FamilyKindV1,
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
        family,
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

fn assert_bounded_outer_integrity_mutations_fail(encoded: &[u8], family: &str, cpu: &str) {
    // These flips exercise the outer collection integrity seal. Resealed nested semantic and
    // frame mutations are covered by capability_broker and source_isa_observation decoder tests.
    let offsets = [0, 31, 79, 80, encoded.len() - 33, encoded.len() - 1];
    for offset in offsets.into_iter().collect::<BTreeSet<_>>() {
        let mut mutated = encoded.to_vec();
        mutated[offset] ^= 1;
        assert!(
            SourceIsaObservationCollectionV1::decode_canonical(&mutated).is_err(),
            "{family} {cpu} accepted wire mutation at byte {offset}",
        );
    }
}
