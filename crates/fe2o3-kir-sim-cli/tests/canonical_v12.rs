#![cfg(target_os = "linux")]

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, Constant, Function, IndexKind,
    IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerificationContractKeyV12, VerificationContractOperationV12, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV12, WorkgroupPipelineEventKindV12,
};
use fe2o3_kir_sim::{PersistedSimulationScheduleArtifactV1, PersistedSimulationScheduleDocumentV1};
use serde_json::Value;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-kir-sim-canonical-v12-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"));
    command.env_clear();
    command
}

fn run(kir: &Path, request: &Path) -> Command {
    let mut command = binary();
    command
        .arg("--kir-v12")
        .arg(kir)
        .arg("--request")
        .arg(request);
    command
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn failure(output: Output) -> Value {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["status"], "error");
    error
}

fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").unwrap();
    }
    result
}

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn memory_module(increment: u32) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let input = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(
            3,
            input.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        op(
            4,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            5,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(increment)),
        ),
        op(
            6,
            scalar,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
        op(
            7,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("canonical-v12-memory");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![input, output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "transform",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

const REQUEST: &[u8] = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"transform","grid":[4,1,1],"workgroup":[2,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_only","alignment":4,"bytes":"0x00000000010000002900000064000000"},{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0xa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5deadbeef"}]}"#;

fn fixture(directory: &TestDirectory) -> (PathBuf, PathBuf, VerifiedCanonicalKernelIrV12) {
    let owner = VerifiedCanonicalKernelIrV12::from_module(memory_module(7)).unwrap();
    let kir = directory.path("kernel-v12.kir");
    let request = directory.path("request.json");
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    fs::write(&request, REQUEST).unwrap();
    (kir, request, owner)
}

fn assert_result(result: &Value, owner: &VerifiedCanonicalKernelIrV12) {
    assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    for flag in [
        "hardware_observed",
        "hardware_validation",
        "performance_prediction",
    ] {
        assert_eq!(result[flag], false, "{flag}");
    }
    assert_eq!(
        result["target_profile"]["identity"],
        "amdgpu_64_little_endian_v1"
    );
    assert_eq!(result["target_profile"]["index_bits"], 64);
    assert_eq!(result["kir"]["sha256"], hex(owner.identity().digest()));
    assert_eq!(
        result["kir"]["canonical_bytes"],
        owner.identity().canonical_length()
    );
    assert_eq!(result["counts"]["invocations_executed"], 4);
    assert_eq!(result["counts"]["workgroups_visited"], 2);
    assert_eq!(
        result["arguments"][0]["value"]["bytes"],
        "0x00000000010000002900000064000000"
    );
    assert_eq!(
        result["arguments"][1]["value"]["bytes"],
        "0x0700000008000000300000006b000000deadbeef"
    );
}

#[test]
fn canonical_v12_executes_scalar_memory_with_exact_identity() {
    let directory = TestDirectory::new();
    let (kir, request, owner) = fixture(&directory);
    assert_result(&success(run(&kir, &request).output().unwrap()), &owner);

    let destination = directory.path("result.json");
    let output = run(&kir, &request)
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_result(
        &serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap(),
        &owner,
    );
    assert_eq!(
        fs::metadata(destination).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn canonical_v12_records_and_replays_canonical_and_seeded_schedules() {
    let directory = TestDirectory::new();
    let (kir, request, owner) = fixture(&directory);
    for (flag, seed, identity) in [
        (
            "--record-canonical-schedule",
            None,
            "workgroup_major_local_zyx_cooperative_v1",
        ),
        (
            "--record-seeded-schedule",
            Some("18446744073709551615"),
            "workgroup_major_seeded_runnable_cooperative_v1",
        ),
    ] {
        let path = directory.path(&format!("{}.json", flag.trim_start_matches("--")));
        let mut command = run(&kir, &request);
        command
            .arg(flag)
            .arg(&path)
            .args(["--schedule-max-decisions", "128"]);
        if let Some(seed) = seed {
            command.args(["--schedule-seed", seed]);
        }
        let recorded = success(command.output().unwrap());
        assert_result(&recorded, &owner);
        assert_eq!(recorded["schedule"]["identity"], identity);
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.ends_with(b"\n"));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["schema"], "fe2o3-simulation-schedule-v1");
        assert_eq!(wire["artifact"]["kind"], "canonical_kir_v12");
        assert_eq!(
            wire["artifact"]["kir_sha256"],
            hex(owner.identity().digest())
        );
        assert_eq!(
            wire["artifact"]["kir_canonical_bytes"],
            owner.identity().canonical_length()
        );
        assert_eq!(wire["coverage"]["complete"], true);
        let decoded = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(
            decoded.binding().artifact(),
            PersistedSimulationScheduleArtifactV1::CanonicalKirV12
        );
        assert_eq!(decoded.binding().kir_wire_version(), 12);
        assert_eq!(decoded.to_canonical_bytes().unwrap(), bytes);
        let replayed = success(
            run(&kir, &request)
                .arg("--replay-schedule")
                .arg(path)
                .output()
                .unwrap(),
        );
        assert_result(&replayed, &owner);
        assert_eq!(replayed["schedule"]["identity"], identity);
        assert_eq!(
            replayed["schedule"]["transcript_sha256"],
            recorded["schedule"]["transcript_sha256"]
        );
        assert_eq!(replayed["arguments"], recorded["arguments"]);
    }
}

#[test]
fn canonical_v12_replay_rejects_body_request_and_version_substitution() {
    let directory = TestDirectory::new();
    let (kir, request, _) = fixture(&directory);
    let schedule = directory.path("schedule.json");
    success(
        run(&kir, &request)
            .arg("--record-canonical-schedule")
            .arg(&schedule)
            .output()
            .unwrap(),
    );

    let changed_kir = directory.path("changed-body.kir");
    let changed = VerifiedCanonicalKernelIrV12::from_module(memory_module(8)).unwrap();
    fs::write(&changed_kir, changed.canonical_bytes()).unwrap();
    let changed_request = directory.path("changed-request.json");
    let mut request_bytes = REQUEST.to_vec();
    request_bytes.push(b'\n');
    fs::write(&changed_request, request_bytes).unwrap();
    let changed_schedule = directory.path("changed-version.json");
    let original = String::from_utf8(fs::read(&schedule).unwrap()).unwrap();
    let substituted = original.replacen(
        "\"kind\":\"canonical_kir_v12\"",
        "\"kind\":\"canonical_kir_v11\"",
        1,
    );
    assert_ne!(original, substituted);
    fs::write(&changed_schedule, substituted).unwrap();

    for (name, kir, request, schedule) in [
        ("body", &changed_kir, &request, &schedule),
        ("request", &kir, &changed_request, &schedule),
        ("version", &kir, &request, &changed_schedule),
    ] {
        let destination = directory.path(&format!("{name}-result.json"));
        let error = failure(
            run(kir, request)
                .arg("--replay-schedule")
                .arg(schedule)
                .arg("--output")
                .arg(&destination)
                .output()
                .unwrap(),
        );
        assert_eq!(
            error["kind"], "schedule_binding_mismatch",
            "{name}: {error}"
        );
        assert_eq!(error["input"], "semantic_schedule");
        assert!(!destination.exists());
    }

    // Identical scalar semantics in a V7 envelope cannot replay a V12 schedule.
    let legacy = directory.path("legacy.kir");
    fs::write(
        &legacy,
        VerifiedCanonicalKernelIrV7::from_module(memory_module(7))
            .unwrap()
            .canonical_bytes(),
    )
    .unwrap();
    let error = failure(
        binary()
            .arg("--kir-v7")
            .arg(&legacy)
            .arg("--request")
            .arg(&request)
            .arg("--replay-schedule")
            .arg(&schedule)
            .output()
            .unwrap(),
    );
    assert_eq!(error["kind"], "schedule_binding_mismatch");
    let error = failure(run(&legacy, &request).output().unwrap());
    assert_eq!(error["kind"], "kir_v12_wrong_version");
    assert_eq!(error["input"], "kir_v12");
}

#[test]
fn canonical_v12_rejects_malformed_inputs_and_conflicting_selectors_without_outputs() {
    let directory = TestDirectory::new();
    let (kir, request, owner) = fixture(&directory);
    for selector in [
        "--kir-v7",
        "--kir-v12",
        "--bundle",
        "--bundle-v5",
        "--bundle-v6",
    ] {
        let destination = directory.path("exclusive-result.json");
        let schedule = directory.path("exclusive-schedule.json");
        let error = failure(
            run(&kir, &request)
                .arg(selector)
                .arg(&kir)
                .arg("--output")
                .arg(&destination)
                .arg("--record-canonical-schedule")
                .arg(&schedule)
                .output()
                .unwrap(),
        );
        assert_eq!(error["stage"], "arguments", "{selector}: {error}");
        assert_eq!(error["kind"], "invalid_command_line");
        assert!(!destination.exists());
        assert!(!schedule.exists());
    }
    for arguments in [vec!["--kir-v12"], vec!["--kir-v12", "--request"]] {
        let error = failure(binary().args(arguments).output().unwrap());
        assert_eq!(error["kind"], "invalid_command_line");
    }

    let mut truncated = owner.canonical_bytes().to_vec();
    truncated.pop();
    let mut trailing = owner.canonical_bytes().to_vec();
    trailing.push(0);
    let mut corrupted = owner.canonical_bytes().to_vec();
    corrupted[0] ^= 1;
    for (name, bytes) in [
        ("truncated", truncated),
        ("trailing", trailing),
        ("corrupted", corrupted),
    ] {
        let invalid = directory.path(&format!("{name}.kir"));
        let destination = directory.path(&format!("{name}-result.json"));
        let schedule = directory.path(&format!("{name}-schedule.json"));
        fs::write(&invalid, bytes).unwrap();
        let error = failure(
            run(&invalid, &request)
                .arg("--output")
                .arg(&destination)
                .arg("--record-canonical-schedule")
                .arg(&schedule)
                .output()
                .unwrap(),
        );
        assert_eq!(error["kind"], "kir_v12_decode_failed", "{name}: {error}");
        assert_eq!(error["input"], "kir_v12");
        assert!(!destination.exists());
        assert!(!schedule.exists());
    }

    let malformed_request = directory.path("malformed-request.json");
    fs::write(&malformed_request, b"{").unwrap();
    let destination = directory.path("malformed-request-result.json");
    let schedule = directory.path("malformed-request-schedule.json");
    failure(
        run(&kir, &malformed_request)
            .arg("--output")
            .arg(&destination)
            .arg("--record-canonical-schedule")
            .arg(&schedule)
            .output()
            .unwrap(),
    );
    assert!(!destination.exists());
    assert!(!schedule.exists());
}

#[test]
fn canonical_v12_recording_preserves_existing_files_and_failed_runs_publish_nothing() {
    let directory = TestDirectory::new();
    let (kir, request, _) = fixture(&directory);
    let existing = directory.path("existing.json");
    fs::write(&existing, b"retained").unwrap();
    for flag in ["--output", "--record-canonical-schedule"] {
        failure(
            run(&kir, &request)
                .arg(flag)
                .arg(&existing)
                .output()
                .unwrap(),
        );
        assert_eq!(fs::read(&existing).unwrap(), b"retained");
    }

    let destination = directory.path("failed-result.json");
    let schedule = directory.path("failed-schedule.json");
    let error = failure(
        run(&kir, &request)
            .arg("--output")
            .arg(&destination)
            .arg("--record-canonical-schedule")
            .arg(&schedule)
            .args(["--schedule-max-decisions", "1"])
            .output()
            .unwrap(),
    );
    assert_eq!(error["kind"], "execution_schedule_decision_limit");
    assert!(!destination.exists());
    assert!(!schedule.exists());
}

#[test]
fn canonical_v12_inert_verification_carrier_is_not_execution_authority() {
    let directory = TestDirectory::new();
    let mut module = memory_module(7);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let storage = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    operations.insert(
        0,
        op(
            9,
            storage,
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Workgroup,
                alignment: 4,
            },
        ),
    );
    operations.insert(
        1,
        op(10, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
    );
    operations.insert(
        2,
        Operation::new(
            vec![],
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: VerificationContractKeyV12::new(0),
                    kind: WorkgroupPipelineEventKindV12::Stage,
                    storage: ValueId(9),
                    epoch: ValueId(10),
                },
            ),
        ),
    );
    let owner = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    let kir = directory.path("inert.kir");
    let request = directory.path("request.json");
    let destination = directory.path("result.json");
    let schedule = directory.path("schedule.json");
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    fs::write(&request, REQUEST).unwrap();
    let error = failure(
        run(&kir, &request)
            .arg("--output")
            .arg(&destination)
            .arg("--record-canonical-schedule")
            .arg(&schedule)
            .output()
            .unwrap(),
    );
    assert_eq!(error["kind"], "preflight_unsupported");
    assert!(
        error["unsupported"]["sites"]
            .as_array()
            .unwrap()
            .iter()
            .any(|site| {
                site["feature"] == "inert_v12_carrier"
                    && site["function"] == "entry"
                    && site["operation"] == 2
            }),
        "{error}"
    );
    assert!(!destination.exists());
    assert!(!schedule.exists());
}
