//! Genuine ordinary Rust -> Bundle V6 -> canonical KIR V11 -> CPU simulation.
//! No constructed KIR, graph conversion, GPU launch, or compiler authority.
//! The ignored case requires the root-managed pinned tool/source/dependency
//! closure and descendant containment. Failure is never replaced by a fixture.
#![cfg(target_os = "linux")]

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, MatrixMultiplyProfile, MatrixOperationKind, OperationKind,
    ScalarType, TensorLayoutContractV1, VerifiedCanonicalKernelIrV11, VerifiedSimulationBundleV6,
};
use fe2o3_kir_sim::{MatrixInputRoleV1, SimulationErrorV1, SimulationExecutionErrorKindV1};
use fe2o3_sim_differential::{
    ExactBufferExpectationV4, ProductionSemanticCaseV4, run_production_semantic_conformance_v4,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// These are the existing independent host oracle and geometry, not copies or
// a second implementation of the simulator's integer numerical domain.
// Their original unit tests and new mixed scalar-oracle controls remain active.
#[allow(dead_code)]
#[path = "../../../examples/gfx950_low_precision/src/dimensions.rs"]
mod dimensions;
#[allow(dead_code)]
#[path = "../../../examples/gfx950_low_precision/src/reference.rs"]
mod reference;

const KERNEL: &str = "gfx950_mixed_fp4_fp8_gemm_rust";
const STREAM_BYTES: usize = 8 * 1024 * 1024;
const EXPORT_SECONDS: u64 = 300;
const DRAIN_SECONDS: u64 = 12;
const TAIL: [u32; 4] = [0x3f80_0000, 0xbf80_0000, 0x4080_0000, 0xc100_0000];
const SOURCE_INPUTS: [(&str, &[u8]); 6] = [
    (
        "examples/gfx950_low_precision/Cargo.toml",
        include_bytes!("../../../examples/gfx950_low_precision/Cargo.toml"),
    ),
    (
        "examples/gfx950_low_precision/Cargo.lock",
        include_bytes!("../../../examples/gfx950_low_precision/Cargo.lock"),
    ),
    (
        "examples/gfx950_low_precision/src/lib.rs",
        include_bytes!("../../../examples/gfx950_low_precision/src/lib.rs"),
    ),
    (
        "examples/gfx950_low_precision/src/kernel.rs",
        include_bytes!("../../../examples/gfx950_low_precision/src/kernel.rs"),
    ),
    (
        "examples/gfx950_low_precision/src/dimensions.rs",
        include_bytes!("../../../examples/gfx950_low_precision/src/dimensions.rs"),
    ),
    (
        "examples/gfx950_low_precision/src/reference.rs",
        include_bytes!("../../../examples/gfx950_low_precision/src/reference.rs"),
    ),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}
fn write_json(path: &Path, value: &Value) {
    write_new(path, &serde_json::to_vec_pretty(value).unwrap());
}
fn private_child(parent: &Path, leaf: &str) -> PathBuf {
    assert!(parent.is_absolute());
    assert_eq!(parent.canonicalize().unwrap(), parent);
    assert!(fs::symlink_metadata(parent).unwrap().is_dir());
    let child = parent.join(leaf);
    fs::create_dir(&child).expect("fresh owned output/target only; no reuse");
    fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).unwrap();
    child
}
fn identity(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64, u32) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
        m.mode(),
    )
}
fn file_pin(path: &Path, limit: u64) -> Value {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .unwrap();
    let before = file.metadata().unwrap();
    assert!(before.is_file() && before.len() <= limit);
    assert_eq!(
        identity(&before),
        identity(&fs::symlink_metadata(path).unwrap())
    );
    let mut remaining = before.len();
    let mut hash = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    while remaining != 0 {
        let count = usize::try_from(remaining.min(chunk.len() as u64)).unwrap();
        file.read_exact(&mut chunk[..count]).unwrap();
        hash.update(&chunk[..count]);
        remaining -= count as u64;
    }
    assert_eq!(
        file.read(&mut chunk[..1]).unwrap(),
        0,
        "paid exact-length EOF"
    );
    assert_eq!(identity(&before), identity(&file.metadata().unwrap()));
    assert_eq!(
        identity(&before),
        identity(&fs::symlink_metadata(path).unwrap())
    );
    json!({"path":path, "bytes":before.len(), "sha256":hex(&hash.finalize()),
           "device":before.dev(), "inode":before.ino(), "mode":before.mode(),
           "mtime":[before.mtime(),before.mtime_nsec()], "ctime":[before.ctime(),before.ctime_nsec()]})
}
fn source_pins(root: &Path) -> Vec<Value> {
    SOURCE_INPUTS
        .iter()
        .map(|(relative, expected)| {
            let pin = file_pin(&root.join(relative), 2 * 1024 * 1024);
            assert_eq!(pin["bytes"], json!(expected.len()));
            assert_eq!(pin["sha256"], digest(expected));
            pin
        })
        .collect()
}
fn exporter_args(root: &Path, out: &Path, target: &Path) -> Vec<std::ffi::OsString> {
    [
        "--crate".into(),
        "fe2o3_gfx950_low_precision".into(),
        "--target".into(),
        "gfx950".into(),
        "--bundle-version".into(),
        "6".into(),
        "--output".into(),
        out.as_os_str().to_owned(),
        "--target-dir".into(),
        target.as_os_str().to_owned(),
        "--".into(),
        "--manifest-path".into(),
        root.join("examples/gfx950_low_precision/Cargo.toml")
            .into_os_string(),
        "--features".into(),
        "kernel-mixed-fp4-fp8-gemm".into(),
        "--lib".into(),
        "--offline".into(),
    ]
    .into()
}
fn capture(
    reader: impl Read + Send + 'static,
    mut raw: fs::File,
    total: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut reader = reader;
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 16 * 1024];
        loop {
            let n = reader.read(&mut chunk)?;
            if n == 0 {
                return Ok(bytes);
            }
            if total
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prior| {
                    prior.checked_add(n).filter(|next| *next <= STREAM_BYTES)
                })
                .is_err()
            {
                exceeded.store(true, Ordering::SeqCst);
                return Ok(bytes);
            }
            // Persist every bounded prefix before waiting for pipe EOF. Root
            // can inspect it even if a descendant keeps the pipe open.
            raw.write_all(&chunk[..n])?;
            bytes.extend_from_slice(&chunk[..n]);
        }
    })
}
fn export(root: &Path, out: &Path, target: &Path) {
    let exporter = Path::new(env!("CARGO_BIN_EXE_fe2o3-export-sim"));
    let extract = exporter.with_file_name("fe2o3-rustc-extract");
    let binaries = [
        file_pin(exporter, 512 * 1024 * 1024),
        file_pin(&extract, 512 * 1024 * 1024),
    ];
    let sources = source_pins(root);
    let args = exporter_args(root, &out.join("kernel.fe2sim"), target);
    write_json(
        &out.join("BEFORE.json"),
        &json!({
            "source":sources, "binaries":binaries, "program":exporter,
            "argv":args.iter().map(|arg| arg.to_str().expect("fixed UTF-8 source paths")).collect::<Vec<_>>(),
            "target":"gfx950", "bundle_version":6, "compiler_execution_authenticated":false,
            "hardware_observed":false, "launch_authority":false
        }),
    );
    let mut command = Command::new(exporter);
    command
        .current_dir(root)
        .env("CARGO", env!("CARGO"))
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Root owns the closed inherited environment; the existing exporter owns
    // its wrapper/AMDGPU target policy. Clear all stale extraction selectors.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("FE2O3_EXTRACT_") {
            command.env_remove(key);
        }
    }
    for key in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(key);
    }
    let raw_stdout = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(out.join("export.stdout"))
        .unwrap();
    let raw_stderr = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(out.join("export.stderr"))
        .unwrap();
    let start = Instant::now();
    let mut child = command.spawn().expect("launch exact original exporter");
    let total = Arc::new(AtomicUsize::new(0));
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout = capture(
        child.stdout.take().unwrap(),
        raw_stdout,
        total.clone(),
        exceeded.clone(),
    );
    let stderr = capture(
        child.stderr.take().unwrap(),
        raw_stderr,
        total.clone(),
        exceeded.clone(),
    );
    let mut forced = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(EXPORT_SECONDS) || exceeded.load(Ordering::SeqCst)
        {
            forced = true;
            // This kills only our owned direct child. Root's existing normal
            // process-tree containment must reap descendants on any failure.
            let _ = child.kill();
            break child.wait().expect("reap owned exporter");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let drain = Instant::now();
    while !(stdout.is_finished() && stderr.is_finished())
        && drain.elapsed() < Duration::from_secs(DRAIN_SECONDS)
    {
        thread::sleep(Duration::from_millis(10));
    }
    let drained = stdout.is_finished() && stderr.is_finished();
    write_json(
        &out.join("EXPORT-STATUS.json"),
        &json!({
            "code":status.code(), "success":status.success(), "forced":forced,
            "streams_drained":drained, "aggregate_stream_bytes":total.load(Ordering::SeqCst),
            "stream_limit":STREAM_BYTES, "stream_limit_exceeded":exceeded.load(Ordering::SeqCst),
            "elapsed_ms":start.elapsed().as_millis(), "direct_child_reaped":true,
            "descendant_cleanup_owner":"root-managed normal containment"
        }),
    );
    assert!(
        drained,
        "inherited pipe not closed; root must contain descendants"
    );
    let stdout = stdout.join().unwrap().unwrap();
    let stderr = stderr.join().unwrap().unwrap();
    assert_eq!(
        file_pin(&out.join("export.stdout"), STREAM_BYTES as u64)["sha256"],
        digest(&stdout)
    );
    assert_eq!(
        file_pin(&out.join("export.stderr"), STREAM_BYTES as u64)["sha256"],
        digest(&stderr)
    );
    let after = source_pins(root);
    let binaries_after = [
        file_pin(exporter, 512 * 1024 * 1024),
        file_pin(&extract, 512 * 1024 * 1024),
    ];
    write_json(
        &out.join("AFTER-EXPORT.json"),
        &json!({"source":after,"binaries":binaries_after}),
    );
    assert_eq!(sources, after);
    assert_eq!(binaries, binaries_after);
    assert!(
        !forced && !exceeded.load(Ordering::SeqCst) && status.success(),
        "ordinary mixed FP4-A/FP8-B export failed; retained raw diagnostics are authoritative, no fallback"
    );
    let diagnostic = std::str::from_utf8(&stderr).unwrap();
    assert!(diagnostic.contains("exact same-module KIR V11"));
    assert!(diagnostic.contains("compiler_execution=extraction_only_unavailable"));
    assert!(diagnostic.contains("authority false"));
}
fn data() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    use dimensions::*;
    const A: [u8; 15] = [
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0, 1, 2, 3, 4, 5, 6, 7,
    ];
    const B: [u8; 11] = [
        0xd8, 0xd0, 0xc0, 0xb8, 0xa8, 0, 0x28, 0x38, 0x40, 0x50, 0x58,
    ];
    let mut lhs = vec![0; GFX950_BATCHES * GEMM_M * GEMM_K];
    let mut rhs = vec![0; GFX950_BATCHES * GEMM_K * GEMM_N];
    for batch in 0..GFX950_BATCHES {
        for row in 0..GEMM_M {
            for k in 0..GEMM_K {
                lhs[batch * GEMM_M * GEMM_K + row * GEMM_K + k] =
                    A[(7 * batch + 3 * row + 2 * k + row * k) % A.len()];
            }
        }
        for k in 0..GEMM_K {
            for column in 0..GEMM_N {
                rhs[batch * GEMM_K * GEMM_N + k * GEMM_N + column] =
                    B[(11 * batch + 3 * k + 5 * column + k * column) % B.len()];
            }
        }
        // Additional independent batch signatures, without signed-zero inputs.
        lhs[batch * GEMM_M * GEMM_K] = A[batch % A.len()];
        rhs[batch * GEMM_K * GEMM_N] = B[batch % B.len()];
    }
    let oracle = reference::mixed_gemm_reference(&lhs, &rhs).unwrap();
    let mut initial = vec![0_u8; oracle.len() * 4];
    let mut expected = oracle
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<_>>();
    for bits in TAIL {
        initial.extend_from_slice(&bits.to_le_bytes());
        expected.extend_from_slice(&bits.to_le_bytes());
    }
    (lhs, rhs, initial, expected)
}
fn request(lhs: &[u8], rhs: &[u8], initial: &[u8]) -> Value {
    fn buffer(element: &str, access: &str, alignment: usize, bytes: &[u8]) -> Value {
        json!({"kind":"buffer","element":element,"access":access,"alignment":alignment,
            "bytes":format!("0x{}",hex(bytes))})
    }
    json!({"schema":"fe2o3-simulation-request-v1","kernel":KERNEL,
        // Simulator grid is global invocations, unlike the host block count.
        "grid":[u64::from(dimensions::GFX950_GRID[0]) * u64::from(dimensions::GFX950_WORKGROUP[0]),1,1],
        "workgroup":dimensions::GFX950_WORKGROUP,
        "arguments":[buffer("u8","read_only",1,lhs),buffer("u8","read_only",1,rhs),
            buffer("f32","read_write",4,initial)]})
}
fn authority_and_identity(
    admitted: &fe2o3_kir_sim_cli::AdmittedSimulationBundleInputV6,
    root: &Path,
) -> Value {
    let bundle = admitted.bundle();
    // The bare CLI selector resolves to the exact admitted xnack-off target.
    assert_eq!(bundle.target(), "gfx950:xnack-");
    assert_eq!(bundle.kernel_count(), 1);
    assert!(
        !admitted.grants_proof_authority()
            && !admitted.grants_artifact_authority()
            && !admitted.grants_compiler_authority()
            && !admitted.authenticates_compiler_execution()
            && !admitted.grants_hardware_authority()
            && !admitted.grants_load_authority()
            && !admitted.grants_launch_authority()
    );
    let input = admitted.input();
    assert_eq!(input.module.identity().wire_version(), 11);
    assert_eq!(input.kir_sha256, *bundle.canonical_kir_v11_digest());
    assert_eq!(
        input.module.identity().canonical_length(),
        bundle.canonical_kir_v11_length()
    );
    // Assert the existing CLI envelope; never replace it with wider test limits.
    let limits = input.simulation_limits;
    assert_eq!(limits.max_steps, 1 << 27);
    assert_eq!(limits.max_resident_bytes, 256 * 1024 * 1024);
    assert_eq!(limits.max_memory_access_records, 65_536);
    let (_, module) = VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
        bundle.canonical_kir_v11().to_vec(),
    )
    .unwrap();
    assert!(module.functions.iter().any(|f| f.id.as_str() == KERNEL));
    let mut mixed = 0;
    for function in &module.functions {
        if let Some(body) = &function.body {
            for operation in body.blocks.iter().flat_map(|b| &b.operations) {
                if let OperationKind::Matrix(matrix) = &operation.kind {
                    match &matrix.kind {
                        MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
                            assert_eq!(
                                *profile,
                                MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64()
                            );
                            assert_eq!(matrix.tensor_layout, Some(TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64()));
                            assert_eq!(matrix.active_lanes, 64);
                            assert_eq!(
                                matrix.convergence,
                                fe2o3_kernel_ir::Convergence::uniform(
                                    fe2o3_kernel_ir::SynchronizationScope::Subgroup
                                )
                            );
                            mixed += 1;
                        }
                        _ => panic!("unexpected matrix family in exact mixed source"),
                    }
                }
            }
        }
    }
    assert!(
        mixed > 0,
        "actual retained compiler module must contain the exact mixed operation"
    );
    let debug = DebugSourceMapDocumentV2::from_canonical_json_bytes(bundle.debug_map()).unwrap();
    let relative = "examples/gfx950_low_precision/src/kernel.rs";
    let absolute = root.join(relative);
    let files = debug
        .files()
        .iter()
        .filter(|file| {
            [absolute.to_str().unwrap(), relative, "src/kernel.rs"].contains(&file.display_path())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        files.len(),
        1,
        "one exact ordinary kernel source-map record"
    );
    assert_eq!(files[0].byte_len(), SOURCE_INPUTS[3].1.len() as u64);
    // The compiler stable-file identity is NOT the raw source SHA256.
    json!({"target":bundle.target(),"bundle_identity":hex(bundle.identity().as_bytes()),
        "bundle_raw_sha256":digest(bundle.canonical_bytes()),
        "subject_identity":hex(bundle.subject_identity()),
        "canonical_v11_sha256":hex(bundle.canonical_kir_v11_digest()),
        "canonical_v11_bytes":bundle.canonical_kir_v11_length(),
        "request_sha256":hex(&input.request_sha256), "mixed_fp4_fp8_operations":mixed,
        "kernel_source_stable_identity":hex(&files[0].identity()),
        "kernel_source_raw_sha256":digest(SOURCE_INPUTS[3].1),
        "kernel_source_display_path":files[0].display_path(),
        "hardware_observed":false,"compiler_execution_authenticated":false,"launch_authority":false})
}
fn compare(
    admitted: &fe2o3_kir_sim_cli::AdmittedSimulationBundleInputV6,
    lhs: &[u8],
    rhs: &[u8],
    expected: &[u8],
    name: &str,
) -> Value {
    let buffers = [
        (ScalarType::U8, lhs),
        (ScalarType::U8, rhs),
        (ScalarType::F32, expected),
    ];
    let initialized = buffers
        .iter()
        .map(|(_, bytes)| vec![true; bytes.len()])
        .collect::<Vec<_>>();
    let outputs = buffers
        .iter()
        .enumerate()
        .map(|(i, (element, bytes))| ExactBufferExpectationV4 {
            argument_ordinal: i,
            element: *element,
            bytes,
            initialized: &initialized[i],
        })
        .collect::<Vec<_>>();
    let report = run_production_semantic_conformance_v4(
        admitted,
        ProductionSemanticCaseV4 {
            case_id: name,
            outputs: &outputs,
        },
    )
    .unwrap();
    assert_eq!(report.bundle_version, 6);
    assert_eq!(report.kir_version, 11);
    assert!(!report.hardware_observed && !report.performance_prediction);
    serde_json::to_value(report).unwrap()
}

#[test]
#[ignore = "genuine compiler extraction; requires root-managed pinned closure and descendant containment"]
fn ordinary_mixed_fp4_fp8_source_bundle_v6_matches_independent_oracle() {
    let root = workspace();
    let out = private_child(
        &PathBuf::from(
            std::env::var_os("CI_LOG_DIR").expect("root-owned persistent log directory"),
        ),
        "mixed-fp4-fp8-source-simulation-v1",
    );
    let target = private_child(
        &PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("root-owned target directory")),
        "mixed-fp4-fp8-source-export-v1",
    );
    let sources = source_pins(&root);
    export(&root, &out, &target);
    let bundle_path = out.join("kernel.fe2sim");
    let (lhs, rhs, initial, expected) = data();
    let request_path = out.join("positive.json");
    write_json(&request_path, &request(&lhs, &rhs, &initial));
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle_path, &request_path)
        .expect("strict original V6 admission, no graph repair or cap override");
    write_json(
        &out.join("IDENTITY.json"),
        &authority_and_identity(&admitted, &root),
    );
    let report = compare(
        &admitted,
        &lhs,
        &rhs,
        &expected,
        "mixed-fp4-fp8-source-positive",
    );
    write_json(&out.join("POSITIVE.json"), &report);
    assert_eq!(report["status"], "agreement");

    // Oracle refusal is an actual second run of the SAME admitted source owner,
    // not an edit to an observed simulator result or synthetic canonical graph.
    let mut wrong_canary = expected.clone();
    *wrong_canary.last_mut().unwrap() ^= 1;
    let negative = compare(
        &admitted,
        &lhs,
        &rhs,
        &wrong_canary,
        "mixed-fp4-fp8-source-canary-negative",
    );
    write_json(&out.join("CANARY-NEGATIVE.json"), &negative);
    assert_eq!(negative["status"], "mismatch");
    assert_eq!(negative["outputs"][0]["exact_bytes"], true);
    assert_eq!(negative["outputs"][1]["exact_bytes"], true);
    assert_eq!(negative["outputs"][2]["exact_bytes"], false);

    // A[row15,k127] is nibble31/word3; B[k127,col15] is byte31/word7,
    // lane63/component31 in the FIRST actual source wave. No lane bypass.
    for (name, role, bits) in [
        ("late-a-negative-zero", MatrixInputRoleV1::A, 0x08),
        ("late-b-nan", MatrixInputRoleV1::B, 0x7f),
        ("late-b-off-quarter", MatrixInputRoleV1::B, 0x01),
    ] {
        let mut a = lhs.clone();
        let mut b = rhs.clone();
        if role == MatrixInputRoleV1::A {
            a[2047] = bits;
        } else {
            b[2047] = bits;
        }
        let path = out.join(format!("{name}.json"));
        write_json(&path, &request(&a, &b, &initial));
        let invalid =
            fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle_path, &path).unwrap();
        assert_eq!(invalid.bundle().identity(), admitted.bundle().identity());
        let input = invalid.input();
        let before = input.request.clone();
        let error = input
            .module
            .simulate(
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
            )
            .expect_err("unsupported genuine input must remain a typed refusal");
        write_json(
            &out.join(format!("{name}-REFUSAL.json")),
            &json!({
                "error":format!("{error:?}"),"request_sha256":hex(&input.request_sha256),
                "bundle_identity":hex(invalid.bundle().identity().as_bytes()),
                "hardware_observed":false,"partial_execution_is_success":false
            }),
        );
        assert_eq!(
            input.request, before,
            "caller-owned input remains unchanged"
        );
        assert!(matches!(error, SimulationErrorV1::Execution(ref e)
        if e.kind == (SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
            role,lane:63,component:31
        })));
    }
    let mut damaged = admitted.bundle().canonical_bytes().to_vec();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(
        VerifiedSimulationBundleV6::from_canonical_bytes(damaged).is_err(),
        "truthfully preserve complete bundle byte binding"
    );
    let after = source_pins(&root);
    write_json(&out.join("FINAL-SOURCE.json"), &json!(after));
    assert_eq!(sources, after);
    write_json(
        &out.join("RESULT.json"),
        &json!({
            "schema":"fe2o3-genuine-mixed-fp4-fp8-source-simulation-test-v1","status":"passed",
            "source":"new ordinary gfx950_mixed_fp4_fp8_gemm_rust; not expert-rank MoE","bundle_version":6,"kir_version":11,
            "positive_runs":1,"canary_mismatch_runs":1,"typed_domain_refusal_runs":3,
            "damaged_bundle_refused":true,"original_cli_limits_unchanged":true,
            "simulated":true,"hardware_observed":false,"performance_prediction":false,
            "compiler_execution_authenticated":false,"load_authority":false,"launch_authority":false
        }),
    );
}

#[test]
fn mixed_fp4_fp8_source_request_uses_all_sixteen_tiles_and_tail_canaries() {
    let (a, b, initial, expected) = data();
    assert_eq!(
        (a.len(), b.len(), initial.len(), expected.len()),
        (32768, 32768, 16400, 16400)
    );
    let request = request(&a, &b, &initial);
    assert_eq!(request["grid"], json!([1024, 1, 1]));
    assert_eq!(request["workgroup"], json!([256, 1, 1]));
    assert_eq!(&initial[16384..], &expected[16384..]);
    assert_ne!(&expected[..1024], &expected[1024..2048]);
}
#[test]
fn mixed_fp4_fp8_source_export_command_is_closed_and_offline() {
    let args = exporter_args(
        Path::new("/source"),
        Path::new("/out/kernel"),
        Path::new("/target"),
    );
    let args = args.iter().map(|x| x.to_str().unwrap()).collect::<Vec<_>>();
    assert_eq!(
        args,
        [
            "--crate",
            "fe2o3_gfx950_low_precision",
            "--target",
            "gfx950",
            "--bundle-version",
            "6",
            "--output",
            "/out/kernel",
            "--target-dir",
            "/target",
            "--",
            "--manifest-path",
            "/source/examples/gfx950_low_precision/Cargo.toml",
            "--features",
            "kernel-mixed-fp4-fp8-gemm",
            "--lib",
            "--offline"
        ]
    );
}
#[test]
fn mixed_fp4_fp8_source_oracle_rejects_shapes_and_distinguishes_late_bytes() {
    let (a, b, _, expected) = data();
    assert!(reference::mixed_gemm_reference(&a[..32767], &b).is_err());
    let mut altered = a.clone();
    altered[2047] = if a[2047] == 0x07 { 0x0f } else { 0x07 };
    let changed = reference::mixed_gemm_reference(&altered, &b).unwrap();
    let changed_bytes = changed
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<_>>();
    assert_ne!(&changed_bytes[..], &expected[..16384]);
    assert!(reference::decode_fp4_e2m1(0x08).is_sign_negative());
    assert!(reference::decode_fp8_e4m3(0x7f).is_nan());
    assert_eq!(reference::decode_fp8_e4m3(0x01), 1.0 / 512.0);
    assert!(
        expected[..16384]
            .chunks_exact(4)
            .all(|x| f32::from_le_bytes(x.try_into().unwrap()).is_finite())
    );
}

#[test]
fn mixed_fp4_fp8_source_data_uses_distinct_domains_and_full_coordinate_range() {
    let (a, b, _, _) = data();
    assert!(a.iter().all(|byte| *byte < 16 && *byte != 8));
    assert!(a.contains(&7) && a.contains(&15));
    assert!(b.contains(&0x58) && b.contains(&0xd8) && b.contains(&0x28));
    for batch in 0..16 {
        assert_ne!(
            &a[batch * 2048..(batch + 1) * 2048],
            &b[batch * 2048..(batch + 1) * 2048]
        );
    }
    // Derive lane/component from source scalar coordinates, not a packed fixture.
    let (row, k, column) = (15, 127, 15);
    assert_eq!((row + 16 * (k / 32), k % 32), (63, 31));
    assert_eq!(
        (column + 16 * ((k % 64) / 16), k % 16 + 16 * (k / 64)),
        (63, 31)
    );
}

#[test]
fn mixed_source_has_a_distinct_explicit_feature_and_no_expert_rank_claim() {
    let manifest = std::str::from_utf8(SOURCE_INPUTS[0].1).unwrap();
    let lib = std::str::from_utf8(SOURCE_INPUTS[2].1).unwrap();
    let compact_lib: String = lib.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let kernel = std::str::from_utf8(SOURCE_INPUTS[3].1).unwrap();
    assert!(manifest.contains("kernel-mixed-fp4-fp8-gemm = []"));
    for original in [
        "kernel-fp4-gemm",
        "kernel-fp8-gemm",
        "kernel-fp4-attention",
        "kernel-fp8-attention",
    ] {
        let guard = format!("all(feature=\"kernel-mixed-fp4-fp8-gemm\",feature=\"{original}\"");
        assert!(
            compact_lib.contains(&format!("{guard})"))
                || compact_lib.contains(&format!("{guard},)"))
        );
    }
    let body = kernel
        .split("pub fn gfx950_mixed_fp4_fp8_gemm_rust(")
        .nth(1)
        .unwrap()
        .split("#[cfg(")
        .next()
        .unwrap();
    assert!(body.contains("Gfx950Fp4MfmaAMatrix::row_major"));
    assert!(body.contains("Gfx950Fp8MfmaBMatrix::row_major"));
    assert!(body.contains(".multiply_accumulate_fp4_fp8("));
    assert!(!body.contains("exp_f32") && !body.contains("unsafe"));
}
