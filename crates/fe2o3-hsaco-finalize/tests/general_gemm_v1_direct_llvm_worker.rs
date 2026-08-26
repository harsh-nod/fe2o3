#![cfg(target_os = "linux")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_api::{
    CompileLimitsV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
    KernelInstanceIdentityV1, RequestIdentityV1, SnapshotFormatIdentityV1, SnapshotIdentityV1,
    StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use fe2o3_general_gemm_compiler::{
    GeneralGemmFrontendSemanticBindingV1, GeneralGemmLoweringLimitsV1, GeneralGemmScheduleV1,
    GeneralGemmSymbolicCompilationUnitV1, GeneralGemmSymbolicKirV1, GeneralGemmSymbolicPlanV1,
    general_gemm_symbolic_obligation_set_identity_v1,
    general_gemm_symbolic_pipeline_configuration_identity_v1,
    lower_general_gemm_symbolic_structural_machine_v1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, GeneralGemmBarrierRefinementV1, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerMeasurementV1, execute_symbolic_general_gemm_worker_v2_v1,
    finalize_symbolic_general_gemm_worker_v2_v1,
};
use fe2o3_kernel_descriptor::CANONICAL_CODE_OBJECT_DIGEST_OFFSET;
use fe2o3_verifier::GeneralGemmEvidenceIdentityV1;
use object::{Object as _, ObjectSection as _};
use sha2::{Digest, Sha256};

const WORKER_PATH: &str = "/home/harsh/fe2o3-general-gemm-worker-llvm22/fe2o3-llvm-link-worker";
const WORKER_SHA256: &str = "0b4936777b08d7d9d864bf357ab4f14cac33a0bb0a13c479209a26c1da808d35";
const WORKER_BUILD_ID: &str =
    "fe2o3-worker-v1-sha256-1769826adfd0cc9832015371d9df79bb9128093a28be89144aa797d8155151a0";
const LLVM_BUILD_ID: &str = "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
const OUTPUT_DIRECTORY_ENV: &str = "FE2O3_GENERAL_GEMM_V1_OUTPUT_DIR";
const NUMERICAL_WORKER_JOIN_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM/NUMERICAL-DIRECT-LLVM-WORKER-JOIN/V1\0";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-general-gemm-v1-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create general-GEMM handoff directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn frontend() -> GeneralGemmFrontendSemanticBindingV1 {
    GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
        identity(0x12),
        identity(0x41),
        identity(0x42),
        identity(0x43),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .expect("descriptive symbolic measurement binding")
}

fn request(
    frontend: &GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
) -> CompileRequestV1 {
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
        SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
        b"inert-general-gemm-worker-measurement".to_vec(),
    )
    .expect("bounded measurement input");
    let obligations = general_gemm_symbolic_obligation_set_identity_v1(&input, frontend);
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(identity(0x11)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
        obligations,
        input,
        CompileLimitsV1::new(16, 16, 16, 4096, 16_384, 4096).expect("bounded measurement limits"),
    )
    .expect("valid measurement request")
}

fn unit(schedule: GeneralGemmScheduleV1) -> GeneralGemmSymbolicCompilationUnitV1 {
    let frontend = frontend();
    let request = request(&frontend, schedule);
    GeneralGemmSymbolicCompilationUnitV1::checked(
        &request,
        frontend,
        schedule,
        GeneralGemmLoweringLimitsV1::default(),
    )
    .expect("checked symbolic measurement unit")
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
    schedule_byte: u8,
) -> ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "general_gemm_v1_direct_llvm_worker",
        Some(Path::new("tests/general_gemm_v1_direct_llvm_worker.rs")),
    )
    .expect("general-GEMM test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([schedule_byte; 32]),
        BuildSession::from_bytes([0x94; 16]),
    )
    .expect("begin general-GEMM handoff attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish general-GEMM handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume general-GEMM handoff")
}

fn decode_sha256(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut bytes = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    bytes
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("noncanonical worker SHA-256 pin"),
    }
}

#[test]
#[ignore = "requires the measured upstream LLVM 22.1.8 worker"]
fn measured_worker_emits_both_inert_general_gemm_schedules() {
    let worker_path = PathBuf::from(WORKER_PATH);
    let worker_length = fs::metadata(&worker_path)
        .expect("measured worker metadata")
        .len();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts(decode_sha256(WORKER_SHA256), worker_length),
        WORKER_BUILD_ID,
        LLVM_BUILD_ID,
    )
    .expect("exact general-GEMM worker measurement");
    let worker = PinnedWorkerV1::open(&worker_path, measurement).expect("open measured worker");
    let output_directory = PathBuf::from(
        env::var(OUTPUT_DIRECTORY_ENV).expect("general-GEMM output directory is required"),
    );
    fs::create_dir_all(&output_directory).expect("create general-GEMM output directory");
    let mut machine_joins = Vec::new();

    for (schedule, label, schedule_byte) in [
        (
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            "reference",
            0x51,
        ),
        (
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
            "vector-a",
            0x52,
        ),
    ] {
        let machine = lower_general_gemm_symbolic_structural_machine_v1(&unit(schedule))
            .expect("lower exact symbolic general-GEMM machine");
        let serialization = machine.compiler_boundary().serialization_receipt();
        assert_eq!(
            serialization.graph_handoff_identity(),
            machine.handoff().identity()
        );
        assert_eq!(serialization.assembly_sha256(), machine.assembly().sha256());
        assert_eq!(
            serialization.worker_admission_identity(),
            machine.worker_admission().admission_identity()
        );
        assert_ne!(machine.compiler_boundary().identity().as_bytes(), &[0; 32]);
        assert!(!machine.compiler_boundary().grants_artifact_authority());
        let live_serialization_identity = serialization.identity().as_bytes();
        let graph_export_identity = serialization.graph_export_identity().as_bytes();
        let non_graph_envelope_identity = serialization.non_graph_envelope_identity().as_bytes();
        let compiler_boundary_identity = *machine.compiler_boundary().identity().as_bytes();
        let directory = TestDirectory::new();
        let consumed = consumed_handoff(&directory, machine.compiler_handoff(), schedule_byte);
        let evidence = execute_symbolic_general_gemm_worker_v2_v1(
            machine,
            consumed,
            &worker,
            WorkerExecutionLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("{label} Worker V2 execution failed: {error:?}"));
        assert!(!evidence.grants_artifact_authority());
        let raw_bytes = evidence.worker_evidence().output_bytes().to_vec();
        fs::write(
            output_directory.join(format!("general-gemm-{label}.raw.hsaco")),
            &raw_bytes,
        )
        .expect("write inert raw HSACO for audit");
        let observation = finalize_symbolic_general_gemm_worker_v2_v1(evidence)
            .unwrap_or_else(|error| panic!("{label} post-link inspection failed: {error:?}"));
        assert_eq!(
            observation.live_serialization_identity(),
            &live_serialization_identity
        );
        assert_eq!(observation.graph_export_identity(), &graph_export_identity);
        assert_eq!(
            observation.non_graph_envelope_identity(),
            &non_graph_envelope_identity
        );
        assert_eq!(
            observation.compiler_boundary_identity(),
            &compiler_boundary_identity
        );
        assert_eq!(observation.schedule(), schedule);
        assert_eq!(
            observation.vector_global_load_count(),
            u32::from(schedule == GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1)
        );
        assert_eq!(observation.barriers_ir(), 2);
        assert_eq!(observation.barriers_isa(), 0);
        assert_eq!(
            observation.barrier_refinement(),
            GeneralGemmBarrierRefinementV1::SingleWaveElision
        );
        assert_eq!(observation.mfma_numerical_refinement().count(), 1);
        assert!(observation.mfma_numerical_refinement().fp_contract_is_off());
        assert!(!observation.grants_artifact_authority());
        assert!(!observation.grants_publication_authority());
        assert!(!observation.grants_load_authority());
        assert!(!observation.grants_launch_authority());
        let numerical_correspondence =
            GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(identity(0xa5));
        let machine_join = observation
            .derive_numerical_machine_join_v1(numerical_correspondence)
            .expect("retained graph, Worker V2, and finalizer owners revalidate");
        assert_eq!(
            observation
                .derive_numerical_machine_join_v1(numerical_correspondence)
                .expect("retained owners survive repeated fresh revalidation"),
            machine_join
        );
        assert_eq!(
            machine_join.numerical_correspondence_identity,
            numerical_correspondence
        );
        assert!(machine_join.has_all_required_identities());
        assert!(!machine_join.grants_compiler_authority());
        let graph_axis = machine_join
            .owner_bound_pliron_graph_serialization_identity
            .expect("live graph axis");
        let worker_axis = machine_join
            .direct_llvm_worker_request_response_identity
            .expect("direct LLVM worker axis");
        let finalizer_axis = machine_join
            .finalizer_post_link_isa_result_identity
            .expect("post-link finalizer axis");
        assert_eq!(
            graph_axis.as_bytes(),
            observation.live_serialization_identity()
        );
        let mut expected_worker = Sha256::new();
        expected_worker.update(NUMERICAL_WORKER_JOIN_DOMAIN_V1);
        expected_worker.update(observation.worker_execution_identity().as_bytes());
        expected_worker.update(observation.worker_request_identity());
        expected_worker.update(observation.worker_response_identity());
        assert_eq!(
            worker_axis,
            GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(expected_worker.finalize().into())
        );
        assert_ne!(finalizer_axis.as_bytes(), observation.identity().as_bytes());
        assert_ne!(finalizer_axis.as_bytes(), &[0; 32]);
        assert_ne!(graph_axis, worker_axis);
        assert_ne!(graph_axis, finalizer_axis);
        assert_ne!(worker_axis, finalizer_axis);
        machine_joins.push((schedule, graph_axis, worker_axis, finalizer_axis));
        let raw_object = object::File::parse(raw_bytes.as_slice()).expect("parse raw HSACO");
        let descriptor = raw_object
            .sections()
            .find(|section| section.name().ok() == Some(".fe2o3.kd.v1"))
            .expect("exact descriptor section");
        let (descriptor_offset, _) = descriptor.file_range().expect("file-backed descriptor");
        let digest_start = descriptor_offset as usize + CANONICAL_CODE_OBJECT_DIGEST_OFFSET;
        let digest_end = digest_start + 32;
        assert_eq!(raw_bytes.len(), observation.exact_finalized_bytes().len());
        for (index, (before, after)) in raw_bytes
            .iter()
            .zip(observation.exact_finalized_bytes())
            .enumerate()
        {
            assert!(
                before == after || (digest_start..digest_end).contains(&index),
                "{label} finalizer changed byte {index} outside the canonical digest field"
            );
        }
        fs::write(
            output_directory.join(format!("general-gemm-{label}.finalized.hsaco")),
            observation.exact_finalized_bytes(),
        )
        .expect("write finalized inert HSACO for audit");
        eprintln!(
            "{label}: machine={:02x?} raw={:02x?} finalized={:02x?}",
            observation.identity().as_bytes(),
            observation.raw_output_identity().sha256(),
            observation.finalized_output_identity().sha256()
        );
    }

    assert_eq!(machine_joins.len(), 2);
    let reference = machine_joins
        .iter()
        .find(|(schedule, ..)| *schedule == GeneralGemmScheduleV1::ReferenceWave64Xor4V1)
        .expect("reference owner-bound join");
    let vector_a = machine_joins
        .iter()
        .find(|(schedule, ..)| {
            *schedule == GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
        })
        .expect("vector-A owner-bound join");
    assert_ne!(reference.1, vector_a.1, "graph owners must not substitute");
    assert_ne!(reference.2, vector_a.2, "Worker owners must not substitute");
    assert_ne!(
        reference.3, vector_a.3,
        "finalizer owners must not substitute"
    );
}
