#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use std::{
    convert::Infallible,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1;
use fe2o3_artifacts::{
    AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, DigestAlgorithm,
    DigestBytes, Mutability, Name, PayloadDigest, PointerWidth,
};
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::__generated::{
    GeneratedScalarGemmV1ReadDeviceSlice, GeneratedScalarGemmV1ReadWriteDeviceSlice,
    load_admitted_worker_v3_application_v1,
};
use fe2o3_host::{
    __hardware_test::{
        application_handoff_observed_context_fixture_v1,
        generated_shared_f32_argument_pair_fixture_v1,
    },
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKernelProfileV1, CompilerGeneratedSemanticWitnessErrorV1,
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedArgumentLayoutError, GeneratedArgumentPackError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedWorkerV3ArgumentBindingV1,
    GeneratedWorkerV3PrepareErrorV1, HsaAgentIdentityV1, HsaCodeObjectLoadObservationV1,
    HsaDispatchObservationV1, HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1,
    HsaImplicitKernargInitializationObservationV1, HsaKernelObjectIdentityV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, HsaPhysicalDeviceIdentityV1,
    HsaRuntimeIdentityV1, HsaUnloadObservationV1, ObservedContext,
    ProductionWorkerV3ApplicationLoadErrorV1, RecoveredWorkerV3AdmissionErrorV1,
    ReviewedHsaExecutableLifecycleAdapterV1, ReviewedHsaImplicitKernargAdapterV1,
    ValidatedCompilerGeneratedSemanticWitnessV1, WorkerV3AuditorV1,
    WorkerV3GeneratedDispatchErrorV1, WorkerV3SafetyPropertiesV1, WorkerV3VerificationAuditErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerificationDecisionErrorV1,
    WorkerV3VerificationDecisionV1, WorkerV3VerificationRequestV1, WorkerV3VerifierV1,
    admit_recovered_worker_v3_descriptor_v1, audit_recovered_worker_v3_verification_v1,
    semantic_witness_from_backend_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_verifier::{
    build_scalar_gemm_worker_v3_proof_input_v3, validate_compiler_proof_binding_association_v3,
    validate_scalar_gemm_compiler_kir_v3,
};
use fe2o3_worker_v2_bundle::{
    RecoveredWorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeWireV1,
    recover_worker_v3_load_envelope_v1,
};
use fe2o3_worker_v3_authority::{
    PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1,
    ProductionScalarGemmWorkerV3RequestAuditorV1, ProductionScalarGemmWorkerV3VerifierErrorV1,
    ProductionScalarGemmWorkerV3VerifierV1,
};
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};
use sha2::{Digest as _, Sha256};

mod scalar_gemm_marker_fixture {
    use fe2o3_device::{DisjointSlice, kernel};

    // This test owns only the generated host ABI fixture. The reviewed example
    // kernel remains a fixture-layer package and is never a cargo-fe2o3 dependency.
    #[kernel(
        typed,
        namespace = "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
        control_flow(loop_bounds(4294967295))
    )]
    pub fn scalar_gemm_v1(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>, m: u32, n: u32, k: u32) {
        let p = fe2o3_device::thread::index_1d().get();
        let output_extent = (m as usize) * (n as usize);
        if p < output_extent {
            let row = p / (n as usize);
            let column = p % (n as usize);
            let mut accumulator = 0.0_f32;
            let mut inner = 0_u32;
            while inner < k {
                let a_index = row * (k as usize) + (inner as usize);
                let b_index = (inner as usize) * (n as usize) + column;
                accumulator += a[a_index] * b[b_index];
                inner += 1;
            }
            if let Some(output) = c.get_mut(fe2o3_device::thread::index_1d()) {
                *output = accumulator;
            }
        }
    }
}

use scalar_gemm_marker_fixture::scalar_gemm_v1_gpu;

#[path = "../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

const TEST_MARKER_BINDING: [u8; 32] = [0xb1; 32];
const TEST_HOST_CONTRACT: [u8; 32] = [0xb2; 32];
const SCALAR_GEMM_MARKER_BINDING: [u8; 32] = [
    0x78, 0x9a, 0xde, 0xdf, 0xdc, 0x3b, 0xe1, 0xfb, 0x60, 0x51, 0x8d, 0xd2, 0xc7, 0x46, 0x0c, 0x3e,
    0xf8, 0xe6, 0xb9, 0x00, 0x52, 0x7d, 0x1b, 0xcb, 0x22, 0x89, 0xba, 0xa1, 0xe0, 0x14, 0x69, 0x3e,
];

#[unsafe(export_name = "__fe2o3_semantic_witness_v1_789adedfdc3be1fb60518dd2c7460c3ef8e6b900527d1bcb2289baa1e014693e_ptr")]
extern "C" fn scalar_gemm_semantic_witness_pointer_v1() -> *const u8 {
    scalar_gemm_semantic_witness_v1().as_ptr()
}

#[unsafe(export_name = "__fe2o3_semantic_witness_v1_789adedfdc3be1fb60518dd2c7460c3ef8e6b900527d1bcb2289baa1e014693e_len")]
extern "C" fn scalar_gemm_semantic_witness_length_v1() -> usize {
    scalar_gemm_semantic_witness_v1().len()
}

fn scalar_gemm_semantic_witness_v1() -> &'static [u8] {
    static WITNESS: OnceLock<Vec<u8>> = OnceLock::new();
    WITNESS.get_or_init(|| {
        assert_eq!(
            scalar_gemm_v1_gpu::Marker::KERNEL_BINDING_ID_V1,
            SCALAR_GEMM_MARKER_BINDING
        );
        let generated_host_contract_identity = match scalar_gemm_v1_gpu::Marker::PROFILE {
            CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
                generated_host_contract_identity,
            } => generated_host_contract_identity,
            _ => panic!("Scalar GEMM marker no longer uses the general typed V3 profile"),
        };
        encode_semantic_witness_v1(SCALAR_GEMM_MARKER_BINDING, generated_host_contract_identity)
    })
}

fn static_host_consumer_application_fixture() -> &'static Path {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = std::env::temp_dir().join(format!(
            "cargo-fe2o3-v3-static-host-consumer-{}",
            std::process::id()
        ));
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let built = Command::new(cargo)
            .current_dir(workspace)
            .env(
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
                "-C target-feature=+crt-static",
            )
            .env("FE2O3_HIP_SYS_DISABLE", "1")
            .args([
                "build",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--target-dir",
            ])
            .arg(&target)
            .args([
                "-p",
                "cargo-fe2o3",
                "--features",
                "worker-v3-host-consumer-fixture",
                "--bin",
                "cargo-fe2o3-worker-v3-host-consumer-app-fixture",
            ])
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "failed to build static V3 host consumer: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        target
            .join("x86_64-unknown-linux-gnu/debug/cargo-fe2o3-worker-v3-host-consumer-app-fixture")
    })
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn encode_semantic_witness_v1(
    marker_binding: [u8; 32],
    generated_host_contract: [u8; 32],
) -> Vec<u8> {
    let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
    let length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&marker_binding);
    bytes.extend_from_slice(&generated_host_contract);
    bytes.extend_from_slice(&(profile.len() as u16).to_le_bytes());
    bytes.extend_from_slice(profile);
    assert_eq!(bytes.len(), length);
    bytes
}

struct WorkerV3VecAddMarker;

fn worker_v3_marker_function() {}

unsafe impl KernelMarkerV1 for WorkerV3VecAddMarker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "vecadd";
    const EXPORT_NAME: &'static str = "vecadd";
    const FUNCTION: Self::Function = worker_v3_marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe impl CompilerGeneratedKernelExpectationV1 for WorkerV3VecAddMarker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: TEST_HOST_CONTRACT,
        };
    const KERNEL_BINDING_ID_V1: [u8; 32] = TEST_MARKER_BINDING;

    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        static WITNESS: OnceLock<Vec<u8>> = OnceLock::new();
        let bytes = WITNESS
            .get_or_init(|| encode_semantic_witness_v1(TEST_MARKER_BINDING, TEST_HOST_CONTRACT));
        // SAFETY: `OnceLock` retains these immutable initialized bytes for the process lifetime.
        unsafe {
            semantic_witness_from_backend_v1(
                bytes.as_ptr(),
                bytes.len(),
                TEST_MARKER_BINDING,
                TEST_HOST_CONTRACT,
            )
        }
    }
}

struct WorkerV3SubstitutedScalarMarker;

unsafe impl KernelMarkerV1 for WorkerV3SubstitutedScalarMarker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "scalar_gemm_v1";
    const EXPORT_NAME: &'static str = "scalar_gemm_v1";
    const FUNCTION: Self::Function = worker_v3_marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe impl CompilerGeneratedKernelExpectationV1 for WorkerV3SubstitutedScalarMarker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: TEST_HOST_CONTRACT,
        };
    const KERNEL_BINDING_ID_V1: [u8; 32] = TEST_MARKER_BINDING;

    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        static WITNESS: OnceLock<Vec<u8>> = OnceLock::new();
        let bytes = WITNESS
            .get_or_init(|| encode_semantic_witness_v1(TEST_MARKER_BINDING, TEST_HOST_CONTRACT));
        // SAFETY: `OnceLock` retains these immutable initialized bytes for the process lifetime.
        unsafe {
            semantic_witness_from_backend_v1(
                bytes.as_ptr(),
                bytes.len(),
                TEST_MARKER_BINDING,
                TEST_HOST_CONTRACT,
            )
        }
    }
}

struct WorkerV3VecAddArguments<'allocation> {
    owner: &'allocation (),
    address: usize,
    length: usize,
}

// SAFETY: this integration fixture mirrors the independently produced descriptor's one exact
// shared-`f32` source argument and retains the inert allocation owner through completion.
unsafe impl<'allocation> CompilerGeneratedWorkerV3ArgumentsV1<'allocation, WorkerV3VecAddMarker>
    for WorkerV3VecAddArguments<'allocation>
{
    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        CompilerGeneratedArgumentLayoutV1::new(
            16,
            8,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    Name::new("values").unwrap(),
                    0,
                    16,
                    8,
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    },
                    Mutability::Immutable,
                    Access::ReadOnly,
                    AddressSpace::Global,
                    <f32 as GeneratedDeviceScalarV1>::shared_slice_type_identity_v1(
                        PointerWidth::Bits64,
                    ),
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedReadOnly,
                )
                .unwrap(),
            ],
        )
    }

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedWorkerV3ArgumentBindingV1<'allocation>, GeneratedArgumentPackError> {
        // SAFETY: the inert numeric allocation is retained by `self.owner` for this integration
        // test and is never dereferenced by either fake runtime stage.
        let values = unsafe {
            generated_shared_f32_argument_pair_fixture_v1(
                &application_handoff_observed_context_fixture_v1("gfx942:xnack-"),
                self.owner,
                plan,
                0,
                self.address,
                self.length,
            )
        };
        Ok(
            GeneratedWorkerV3ArgumentBindingV1::from_compiler_generated_parts_v1(
                vec![],
                vec![values],
            ),
        )
    }
}

struct ReviewedTestWorkerV3Verifier {
    substitute_finalized: bool,
}

struct ReviewedTestWorkerV3Auditor;

struct MutatingCurrentPublicationAuditor {
    output: PathBuf,
}

impl<K> WorkerV3AuditorV1<K> for ReviewedTestWorkerV3Auditor
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;
    type Evidence = ([u8; 32], u64);

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        let finalized_sha256: [u8; 32] = Sha256::digest(request.finalized_hsaco_bytes()).into();
        assert_eq!(finalized_sha256, request.finalized_hsaco_sha256());
        assert_eq!(
            u64::try_from(request.finalized_hsaco_bytes().len()).unwrap(),
            request.finalized_hsaco_length()
        );
        Ok((finalized_sha256, request.finalized_hsaco_length()))
    }
}

impl WorkerV3AuditorV1<scalar_gemm_v1_gpu::Marker> for MutatingCurrentPublicationAuditor {
    type Error = Infallible;
    type Evidence = ();

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, scalar_gemm_v1_gpu::Marker>,
    ) -> Result<Self::Evidence, Self::Error> {
        let artifact = self.output.join(format!(
            ".fe2o3-link-artifact-v1-{}.bin",
            lower_hex(&request.finalized_hsaco_sha256())
        ));
        let mut bytes = fs::read(&artifact).unwrap();
        assert_eq!(bytes, request.finalized_hsaco_bytes());
        bytes[0] ^= 0xff;
        let mut permissions = fs::metadata(&artifact).unwrap().permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&artifact, permissions).unwrap();
        fs::write(artifact, bytes).unwrap();
        Ok(())
    }
}

// SAFETY: this synthetic verifier is confined to test-only fixtures. It mirrors every requested
// identity and must never be used as production proof authority.
unsafe impl<K> WorkerV3VerifierV1<K> for ReviewedTestWorkerV3Verifier
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;

    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        let capsule = request.semantic_compiler_handoff().capsule();
        assert_eq!(*capsule.identity().sha256(), request.capsule_sha256());
        assert_eq!(capsule.canonical_bytes(), request.semantic_capsule_bytes());
        let finalized_sha256: [u8; 32] = Sha256::digest(request.finalized_hsaco_bytes()).into();
        assert_eq!(finalized_sha256, request.finalized_hsaco_sha256());
        assert_eq!(
            u64::try_from(request.finalized_hsaco_bytes().len()).unwrap(),
            request.finalized_hsaco_length()
        );
        assert_eq!(
            *capsule.receipts().formal_memory().identity().sha256(),
            request.formal_memory_receipt_sha256()
        );
        assert_eq!(
            capsule.receipts().formal_memory().canonical_preimage(),
            request.formal_memory_receipt_bytes()
        );
        assert_eq!(
            *capsule.receipts().proof_binding().identity().sha256(),
            request.proof_binding_receipt_sha256()
        );
        assert_eq!(
            capsule.receipts().proof_binding().canonical_preimage(),
            request.proof_binding_receipt_bytes()
        );
        let receipts = capsule.receipts();
        let proof_binding = validate_compiler_proof_binding_association_v3(
            receipts.proof_binding(),
            receipts.semantic_mir(),
            receipts.middle_end(),
            receipts.kernel_ir(),
            receipts.mir_to_kir_correspondence(),
            receipts.formal_memory(),
        )
        .expect("production compiler proof association must match exact retained receipts");
        assert_eq!(
            proof_binding.receipt_identity(),
            receipts.proof_binding().identity()
        );
        if request.marker_logical_name() == "scalar_gemm_v1" {
            let scalar_kir =
                validate_scalar_gemm_compiler_kir_v3(&proof_binding, receipts.kernel_ir()).expect(
                    "scalar Worker V3 request must retain the exact reviewed canonical KIR",
                );
            let proof_input = build_scalar_gemm_worker_v3_proof_input_v3(
                *request.challenge_identity().as_bytes(),
                &proof_binding,
                &scalar_kir,
            )
            .expect("scalar Worker V3 request must generate exact challenge-bound proof input");
            assert_eq!(
                proof_input.challenge(),
                *request.challenge_identity().as_bytes()
            );
            assert!(proof_input.binds_worker_v3_challenge());
            assert!(proof_input.includes_reviewed_kir_integer_profile_equations());
            assert!(proof_input.binds_exhaustive_decoded_kir_projection());
            assert!(proof_input.includes_reviewed_kir_operational_semantics());
            assert!(proof_input.binds_exact_projection_tlv_framing());
            assert!(!proof_input.authenticates_verus_execution());
            assert!(!proof_input.establishes_source_to_kir_refinement());
            assert!(!proof_input.grants_artifact_or_runtime_authority());
        }
        let mut finalized = request.finalized_hsaco_sha256();
        if self.substitute_finalized {
            finalized[0] ^= 0xff;
        }
        Ok(WorkerV3VerificationDecisionV1::new(
            request.challenge_identity(),
            request.lineage_identity(),
            request.descriptor().kernel_id(),
            request.marker_binding_identity(),
            request.generated_host_contract_identity(),
            request.capsule_sha256(),
            request.formal_memory_receipt_sha256(),
            request.proof_binding_receipt_sha256(),
            finalized,
            request.finalized_hsaco_length(),
            request.target(),
            request.code_object_version(),
            [0xc1; 32],
            [0xc2; 32],
            [0xc3; 32],
            [0xc4; 32],
            [0xc5; 32],
            WorkerV3SafetyPropertiesV1::required(),
        ))
    }
}

#[derive(Debug)]
struct ReviewedTestHsaExecutable {
    identity: HsaExecutableObjectIdentityV1,
}

#[derive(Debug)]
struct ReviewedTestHsaKernel {
    identity: HsaKernelObjectIdentityV1,
}

#[derive(Default)]
struct ReviewedTestHsaState {
    unloads: AtomicUsize,
    implicit_initializations: AtomicUsize,
    dispatches: AtomicUsize,
    dispatched_kernarg: Mutex<Option<Vec<u8>>>,
    dispatched_geometry: Mutex<Option<HsaLaunchGeometryV1>>,
    fault: Mutex<ReviewedTestHsaFault>,
}

#[derive(Clone, Copy, Default)]
enum ReviewedTestHsaFault {
    #[default]
    None,
    ImplicitError,
    MutateExplicit,
    ImplicitKernel,
    DispatchError,
    DispatchIncomplete,
}

struct ReviewedTestHsaAdapter {
    environment: HsaEnvironmentObservationV1,
    state: Arc<ReviewedTestHsaState>,
    substitute_load_digest: bool,
}

impl ReviewedTestHsaAdapter {
    fn new() -> (Self, Arc<ReviewedTestHsaState>) {
        let target = AmdTargetId::parse("gfx942:sramecc+:xnack-").unwrap();
        let runtime = HsaRuntimeIdentityV1::new(
            "test-hsa",
            "v1",
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0xd1; 32])),
            [0xd2; 16],
        )
        .unwrap();
        let physical = HsaPhysicalDeviceIdentityV1::new([0xd3; 16], 1, 0, target).unwrap();
        let agent =
            HsaAgentIdentityV1::new(runtime.instance(), 0xd4, physical.uuid(), target).unwrap();
        let environment = HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap();
        let state = Arc::new(ReviewedTestHsaState::default());
        (
            Self {
                environment,
                state: state.clone(),
                substitute_load_digest: false,
            },
            state,
        )
    }

    fn with_substituted_load_digest() -> (Self, Arc<ReviewedTestHsaState>) {
        let (mut adapter, state) = Self::new();
        adapter.substitute_load_digest = true;
        (adapter, state)
    }

    fn executable_identity() -> HsaExecutableObjectIdentityV1 {
        HsaExecutableObjectIdentityV1::new([0xd5; 32]).unwrap()
    }
}

// SAFETY: this test adapter is deterministic, synchronous, and retains no native authority.
unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for ReviewedTestHsaAdapter {
    type Executable = ReviewedTestHsaExecutable;
    type Kernel = ReviewedTestHsaKernel;
    type Error = &'static str;

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        Ok(self.environment.clone())
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        let identity = Self::executable_identity();
        let observed_digest = if self.substitute_load_digest {
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0xdf; 32]))
        } else {
            finalized_digest
        };
        Ok((
            ReviewedTestHsaExecutable { identity },
            HsaCodeObjectLoadObservationV1::new(
                observed_digest,
                u64::try_from(bytes.len()).unwrap(),
                self.environment.runtime().instance(),
                self.environment.agent().agent_handle(),
                identity,
            ),
        ))
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        let identity = HsaKernelObjectIdentityV1::new([0xd6; 32]).unwrap();
        Ok((
            ReviewedTestHsaKernel { identity },
            HsaKernelResolutionObservationV1::new(
                executable.identity,
                identity,
                export_symbol,
                272,
                16,
            )
            .unwrap(),
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        self.state.dispatches.fetch_add(1, Ordering::SeqCst);
        let fault = *self.state.fault.lock().unwrap();
        if matches!(fault, ReviewedTestHsaFault::DispatchError) {
            return Err("fixture dispatch failure");
        }
        *self.state.dispatched_kernarg.lock().unwrap() = Some(kernarg.to_vec());
        *self.state.dispatched_geometry.lock().unwrap() = Some(geometry);
        HsaDispatchObservationV1::new(
            [0xd7; 16],
            executable.identity,
            kernel.identity,
            geometry,
            !matches!(fault, ReviewedTestHsaFault::DispatchIncomplete),
        )
        .map_err(|_| "invalid fixture dispatch observation")
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        self.state.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(HsaUnloadObservationV1::new(
            executable.identity,
            self.environment.runtime().instance(),
            self.environment.agent().agent_handle(),
            true,
        ))
    }
}

// SAFETY: this fake initializer preserves the explicit prefix, initializes the complete supplied
// suffix synchronously, and reports only identities derived from the exact private handles.
unsafe impl ReviewedHsaImplicitKernargAdapterV1 for ReviewedTestHsaAdapter {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        self.state
            .implicit_initializations
            .fetch_add(1, Ordering::SeqCst);
        let fault = *self.state.fault.lock().unwrap();
        if matches!(fault, ReviewedTestHsaFault::ImplicitError) {
            return Err("fixture implicit initialization failure");
        }
        kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0xa5);
        if matches!(fault, ReviewedTestHsaFault::MutateExplicit) {
            kernarg[0] ^= 0xff;
        }
        let kernel_identity = if matches!(fault, ReviewedTestHsaFault::ImplicitKernel) {
            HsaKernelObjectIdentityV1::new([0xde; 32]).unwrap()
        } else {
            kernel.identity
        };
        Ok(HsaImplicitKernargInitializationObservationV1::new(
            executable.identity,
            kernel_identity,
            geometry,
            u64::try_from(explicit_byte_len).unwrap(),
            u64::try_from(implicit_byte_offset).unwrap(),
            u64::try_from(implicit_byte_len).unwrap(),
            true,
        ))
    }
}

fn recovered_host_fixture() -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV1,
) {
    recover_published_worker_v3_fixture(worker_v3_fixture::published_worker_v3_fixture())
}

fn recover_published_worker_v3_fixture(
    fixture: worker_v3_fixture::PublishedWorkerV3Fixture,
) -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV1,
) {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = fixture;
    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    let intent = envelope.wire().publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&directory.0)
        .unwrap();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);
    let recovered = recover_worker_v3_load_envelope_v1(&directory.0, attempt).unwrap();
    (directory, recovered)
}

#[test]
fn cargo_supervisor_and_static_host_consumer_complete_strict_v3_handoff() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    let intent = envelope.wire().publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&directory.0)
        .unwrap();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);

    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut owner = b"fe2o3-owned-v1\0".to_vec();
    owner.extend_from_slice(&[0x55; 16]);
    let owner_path = directory.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();

    let kernel = directory.0.join("v3-application.kernel-id");
    let report = directory.0.join("v3-application-report.json");
    fs::write(&kernel, "a1".repeat(32)).unwrap();
    let metadata = fs::metadata(&directory.0).unwrap();
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    let runner_context = "3-test-scheduler-tolerant";
    #[cfg(not(feature = "worker-v2-fault-injection-test-only"))]
    let runner_context = "3";
    let completed = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("__fe2o3-runner-v1")
        .arg(runner_context)
        .arg(lower_hex(directory.0.as_os_str().as_encoded_bytes()))
        .arg(metadata.dev().to_string())
        .arg(metadata.ino().to_string())
        .arg("required")
        .arg("0")
        .arg(static_host_consumer_application_fixture())
        .arg(&kernel)
        .arg("gfx942:xnack-")
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "strict V3 application handoff failed: {}; report: {}",
        String::from_utf8_lossy(&completed.stderr),
        fs::read_to_string(&report).unwrap_or_else(|error| format!("unavailable ({error})"))
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report["host_consumer"], true);
    assert_eq!(report["loader_environment_clear"], true);
    assert_eq!(report["admitted"], true);
    assert_eq!(report["current"], true);

    let recovered = recover_worker_v3_load_envelope_v1(&directory.0, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
}

#[test]
fn completed_v3_publication_becomes_restartable_inert_envelope_custody() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let output_dir = directory.0.clone();
    let exact_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();

    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    assert_eq!(envelope.exact_artifact_bytes(), exact_artifact);
    assert!(!envelope.grants_load_authority());
    assert!(!envelope.grants_launch_authority());

    let canonical = envelope.encode_canonical().unwrap();
    let inert = WorkerV3LoadEnvelopeWireV1::decode_canonical(&canonical).unwrap();
    inert
        .validate_reacquired_publication_lease_v1(envelope.current_publication_lease())
        .unwrap();
    assert_eq!(inert.encode_canonical().unwrap(), canonical);
    assert!(!inert.grants_publication_authority());
    assert!(!inert.grants_load_authority());
    assert!(!inert.grants_launch_authority());

    let intent = inert.publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&output_dir)
        .unwrap();
    assert_eq!(readiness.exact_envelope_bytes(), canonical);
    assert!(!readiness.authenticates_descriptor_source());
    assert!(!readiness.establishes_hsa_readiness());
    assert!(!readiness.grants_load_authority());
    assert!(!readiness.grants_launch_authority());

    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &output_dir,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);

    let recovered = recover_worker_v3_load_envelope_v1(&output_dir, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
    assert_eq!(recovered.wire().encode_canonical().unwrap(), canonical);
    assert_eq!(recovered.exact_artifact_bytes(), exact_artifact);
    assert!(!recovered.authenticates_descriptor_source());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    assert_eq!(admitted.descriptor().entry_name().as_str(), "vecadd");
    assert_eq!(
        admitted.descriptor().descriptor_symbol().as_str(),
        "vecadd.kd"
    );
    assert_eq!(admitted.physical_kernel().name(), "vecadd");
    assert_eq!(admitted.physical_kernel().symbol(), "vecadd.kd");
    assert_eq!(admitted.descriptor_binding().kernel_index(), 0);
    assert_eq!(admitted.target().to_string(), "gfx942:xnack-");
    assert_eq!(admitted.code_object_version().number(), 6);
    assert!(admitted.authenticates_descriptor_source());
    assert!(!admitted.authenticates_compiler_origin());
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    admitted.revalidate_currentness().unwrap();

    let (adapter, adapter_state) = ReviewedTestHsaAdapter::new();
    let mut loaded = load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
        admitted,
        &mut ReviewedTestWorkerV3Verifier {
            substitute_finalized: false,
        },
        adapter,
    )
    .unwrap();
    assert!(!loaded.grants_load_authority());
    assert!(!loaded.grants_launch_authority());
    assert_eq!(loaded.kernel_observation().export_symbol(), "vecadd");
    loaded.revalidate_currentness().unwrap();

    let owner = ();
    for rejected_geometry in [
        HsaLaunchGeometryV1::new([0, 1, 1], [64, 1, 1], 0),
        HsaLaunchGeometryV1::new([5, 1, 1], [257, 1, 1], 0),
        HsaLaunchGeometryV1::new([5, 1, 1], [64, 1, 1], 1),
    ] {
        match loaded.prepare_generated_worker_v3_v1(
            &observed,
            rejected_geometry,
            WorkerV3VecAddArguments {
                owner: &owner,
                address: 0x10_000,
                length: 257,
            },
        ) {
            Err(GeneratedWorkerV3PrepareErrorV1::LaunchAuthorization(_)) => {}
            Err(other) => panic!("unexpected rejected-geometry error: {other:?}"),
            Ok(_) => panic!("rejected geometry unexpectedly prepared"),
        }
    }
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 0);

    let geometry = HsaLaunchGeometryV1::new([5, 1, 1], [64, 1, 1], 0);
    for fault in [
        ReviewedTestHsaFault::ImplicitError,
        ReviewedTestHsaFault::MutateExplicit,
        ReviewedTestHsaFault::ImplicitKernel,
        ReviewedTestHsaFault::DispatchError,
        ReviewedTestHsaFault::DispatchIncomplete,
    ] {
        *adapter_state.fault.lock().unwrap() = fault;
        let result = loaded
            .prepare_generated_worker_v3_v1(
                &observed,
                geometry,
                WorkerV3VecAddArguments {
                    owner: &owner,
                    address: 0x10_000,
                    length: 257,
                },
            )
            .unwrap()
            .dispatch();
        let rejected_at_expected_stage = matches!(
            (fault, result),
            (
                ReviewedTestHsaFault::ImplicitError,
                Err(WorkerV3GeneratedDispatchErrorV1::ImplicitAdapter(_)),
            ) | (
                ReviewedTestHsaFault::MutateExplicit,
                Err(WorkerV3GeneratedDispatchErrorV1::ExplicitKernargMutation),
            ) | (
                ReviewedTestHsaFault::ImplicitKernel,
                Err(WorkerV3GeneratedDispatchErrorV1::ImplicitObservationMismatch(_)),
            ) | (
                ReviewedTestHsaFault::DispatchError,
                Err(WorkerV3GeneratedDispatchErrorV1::DispatchAdapter(_)),
            ) | (
                ReviewedTestHsaFault::DispatchIncomplete,
                Err(WorkerV3GeneratedDispatchErrorV1::DispatchObservationMismatch(_)),
            )
        );
        assert!(rejected_at_expected_stage);
    }
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        5
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 2);
    *adapter_state.fault.lock().unwrap() = ReviewedTestHsaFault::None;

    let prepared = loaded
        .prepare_generated_worker_v3_v1(
            &observed,
            geometry,
            WorkerV3VecAddArguments {
                owner: &owner,
                address: 0x10_000,
                length: 257,
            },
        )
        .unwrap();
    assert_eq!(prepared.geometry(), geometry);
    assert_eq!(prepared.explicit_byte_len(), 16);
    assert_eq!(prepared.implicit_byte_len(), 256);
    assert_eq!(prepared.physical_kernarg_byte_len(), 272);
    assert_eq!(prepared.physical_kernarg_alignment(), 16);

    let completed = prepared.dispatch().unwrap();
    assert_eq!(completed.kernel_id().as_bytes(), &[0xa1; 32]);
    assert_eq!(completed.completed_dispatch().geometry(), geometry);
    assert!(completed.completed_dispatch().dispatch().completed());
    loaded.revalidate_currentness().unwrap();
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        6
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 3);
    assert_eq!(
        *adapter_state.dispatched_geometry.lock().unwrap(),
        Some(geometry)
    );
    let kernarg_guard = adapter_state.dispatched_kernarg.lock().unwrap();
    let kernarg = kernarg_guard.as_ref().unwrap();
    assert_eq!(&kernarg[..8], &0x10_000_u64.to_le_bytes());
    assert_eq!(&kernarg[8..16], &257_u64.to_le_bytes());
    assert!(kernarg[16..].iter().all(|byte| *byte == 0xa5));
    drop(kernarg_guard);

    let unloaded = loaded.unload().unwrap();
    assert!(unloaded.unload_observation().released());
    assert!(!unloaded.grants_load_authority());
    assert!(!unloaded.grants_launch_authority());
    assert_eq!(adapter_state.unloads.load(Ordering::SeqCst), 1);
}

#[test]
#[ignore = "requires FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO and a gfx942:xnack- GPU"]
fn synthetic_verifier_executes_real_scalar_gemm_through_strict_v3() {
    const M: u32 = 3;
    const N: u32 = 5;
    const K: u32 = 7;
    const CANARY: f32 = f32::from_bits(0x7fc0_5a5a);

    let path = std::env::var_os("FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO")
        .expect("FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO is not set");
    let raw_hsaco = fs::read(path).unwrap();
    let inspection = fe2o3_hsaco_finalize::inspect_unfinalized(&raw_hsaco).unwrap();
    assert_eq!(
        inspection
            .descriptor_table()
            .device_target()
            .as_amd_target_id()
            .processor(),
        "gfx942"
    );
    let descriptor = inspection
        .descriptor_table()
        .kernels()
        .iter()
        .find(|descriptor| descriptor.entry_name().as_str() == "scalar_gemm_v1")
        .expect("raw HSACO contains scalar_gemm_v1");
    let kernel_id = descriptor.kernel_id();
    drop(inspection);

    let fixture = worker_v3_fixture::published_worker_v3_fixture_from_raw_hsaco(
        raw_hsaco,
        "scalar_gemm_v1",
        "scalar_gemm_v1.kd",
    );
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);

    let context = GpuContext::new(0).unwrap();
    let observed = ObservedContext::observe(&context).unwrap();
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, kernel_id, &observed).unwrap();
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone()).unwrap();
    let mut loaded = load_admitted_worker_v3_application_v1::<scalar_gemm_v1_gpu::Marker, _, _>(
        admitted,
        &mut ReviewedTestWorkerV3Verifier {
            substitute_finalized: false,
        },
        adapter,
    )
    .unwrap();

    let stream = context.default_stream();
    let a_host = (0..usize::try_from(M * K).unwrap())
        .map(|index| (index % 11) as f32 - 5.0)
        .collect::<Vec<_>>();
    let b_host = (0..usize::try_from(K * N).unwrap())
        .map(|index| (index % 7) as f32 - 3.0)
        .collect::<Vec<_>>();
    let expected = scalar_gemm_reference(&a_host, &b_host, M, N, K);
    let a = DeviceBuffer::from_host(&stream, &a_host).unwrap();
    let b = DeviceBuffer::from_host(&stream, &b_host).unwrap();
    let mut guarded = DeviceBuffer::from_host(
        &stream,
        &std::iter::once(CANARY)
            .chain(std::iter::repeat_n(CANARY, expected.len()))
            .chain(std::iter::once(CANARY))
            .collect::<Vec<_>>(),
    )
    .unwrap();

    {
        let (_left, output, _right) = guarded.split_range_mut(1..1 + expected.len()).unwrap();
        let arguments = scalar_gemm_v1_gpu::Arguments::new(
            GeneratedScalarGemmV1ReadDeviceSlice::new(&observed, &a).unwrap(),
            GeneratedScalarGemmV1ReadDeviceSlice::new(&observed, &b).unwrap(),
            GeneratedScalarGemmV1ReadWriteDeviceSlice::from_view_mut(&observed, output).unwrap(),
            M,
            N,
            K,
        );
        let geometry = HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0);
        let completed = arguments
            .prepare_worker_v3(&mut loaded, &observed, geometry)
            .unwrap()
            .dispatch()
            .unwrap();
        assert_eq!(completed.kernel_id(), kernel_id);
        assert_eq!(completed.completed_dispatch().geometry(), geometry);
        assert!(completed.completed_dispatch().dispatch().completed());
    }

    let guarded_after = guarded.to_host_vec(&stream).unwrap();
    assert_eq!(guarded_after[0].to_bits(), CANARY.to_bits());
    assert_eq!(guarded_after.last().unwrap().to_bits(), CANARY.to_bits());
    for (actual, expected) in guarded_after[1..1 + expected.len()].iter().zip(&expected) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(a.to_host_vec(&stream).unwrap(), a_host);
    assert_eq!(b.to_host_vec(&stream).unwrap(), b_host);
    assert!(loaded.unload().unwrap().unload_observation().released());
}

#[test]
fn production_scalar_request_auditor_validates_exact_deterministic_hsaco_and_kir() {
    let fixture = worker_v3_fixture::published_scalar_gemm_worker_v3_fixture();
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes(SCALAR_GEMM_MARKER_BINDING),
        &observed,
    )
    .unwrap();
    let lineage = admitted.lineage_identity();
    let mut auditor =
        ProductionScalarGemmWorkerV3RequestAuditorV1::<scalar_gemm_v1_gpu::Marker>::new();

    let prepared = audit_recovered_worker_v3_verification_v1::<scalar_gemm_v1_gpu::Marker, _>(
        &admitted,
        &mut auditor,
    )
    .unwrap();

    assert_ne!(prepared.finalized_hsaco_sha256(), [0; 32]);
    assert_ne!(prepared.finalized_hsaco_length(), 0);
    assert_ne!(prepared.proof_input().challenge(), [0; 32]);
    assert!(prepared.proof_input().binds_worker_v3_challenge());
    assert!(
        prepared
            .proof_input()
            .binds_exhaustive_decoded_kir_projection()
    );
    assert!(
        prepared
            .proof_input()
            .includes_reviewed_kir_operational_semantics()
    );
    assert!(!prepared.authenticates_verus_execution());
    assert!(!prepared.can_enter_worker_v3_gate());
    assert!(!prepared.grants_artifact_or_runtime_authority());
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn production_scalar_request_auditor_rejects_substituted_canonical_kir() {
    let fixture = worker_v3_fixture::published_scalar_gemm_worker_v3_fixture_with_substituted_kir();
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes(SCALAR_GEMM_MARKER_BINDING),
        &observed,
    )
    .unwrap();
    let lineage = admitted.lineage_identity();
    let mut auditor =
        ProductionScalarGemmWorkerV3RequestAuditorV1::<scalar_gemm_v1_gpu::Marker>::new();

    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<scalar_gemm_v1_gpu::Marker, _>(
            &admitted,
            &mut auditor,
        ),
        Err(WorkerV3VerificationAuditErrorV1::Auditor(
            ProductionScalarGemmWorkerV3VerifierErrorV1::ScalarKernelIr(_)
        ))
    ));
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn production_scalar_request_auditor_rejects_a_different_generated_kernel_profile() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    let lineage = admitted.lineage_identity();
    let mut auditor = ProductionScalarGemmWorkerV3RequestAuditorV1::<WorkerV3VecAddMarker>::new();

    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<WorkerV3VecAddMarker, _>(
            &admitted,
            &mut auditor,
        ),
        Err(WorkerV3VerificationAuditErrorV1::Auditor(
            ProductionScalarGemmWorkerV3VerifierErrorV1::UnsupportedKernel
        ))
    ));
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
}

#[test]
fn production_scalar_request_auditor_rejects_substituted_descriptor_identity() {
    let fixture =
        worker_v3_fixture::published_scalar_gemm_worker_v3_fixture_with_substituted_descriptor_binding();
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xc1; 32]),
        &observed,
    )
    .unwrap();
    let mut auditor =
        ProductionScalarGemmWorkerV3RequestAuditorV1::<scalar_gemm_v1_gpu::Marker>::new();

    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<scalar_gemm_v1_gpu::Marker, _>(
            &admitted,
            &mut auditor,
        ),
        Err(WorkerV3VerificationAuditErrorV1::Auditor(
            ProductionScalarGemmWorkerV3VerifierErrorV1::ScalarDescriptor(_)
        ))
    ));
    admitted.revalidate_currentness().unwrap();
}

#[test]
fn production_scalar_request_auditor_rejects_substituted_marker_binding() {
    let fixture = worker_v3_fixture::published_scalar_gemm_worker_v3_fixture();
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes(SCALAR_GEMM_MARKER_BINDING),
        &observed,
    )
    .unwrap();
    let mut auditor =
        ProductionScalarGemmWorkerV3RequestAuditorV1::<WorkerV3SubstitutedScalarMarker>::new();

    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<WorkerV3SubstitutedScalarMarker, _>(
            &admitted,
            &mut auditor,
        ),
        Err(WorkerV3VerificationAuditErrorV1::Auditor(
            ProductionScalarGemmWorkerV3VerifierErrorV1::MarkerDescriptorBindingMismatch
        ))
    ));
    admitted.revalidate_currentness().unwrap();
}

#[test]
fn borrowed_scalar_request_audit_rejects_publication_mutation_during_validation() {
    let fixture = worker_v3_fixture::published_scalar_gemm_worker_v3_fixture();
    let (directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes(SCALAR_GEMM_MARKER_BINDING),
        &observed,
    )
    .unwrap();
    let mut auditor = MutatingCurrentPublicationAuditor {
        output: directory.0.clone(),
    };

    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<scalar_gemm_v1_gpu::Marker, _>(
            &admitted,
            &mut auditor,
        ),
        Err(WorkerV3VerificationAuditErrorV1::CurrentPublication(_))
    ));
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
#[ignore = "requires exact scalar HSACO and a protected general-GEMM Verus runtime closure"]
fn production_verifier_audits_exact_proof_and_preserves_admission_custody() {
    let hsaco_path = std::env::var_os("FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO")
        .expect("FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO is not set");
    let runtime_root = std::env::var_os("FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT")
        .expect("FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT is not set");
    let raw_hsaco = fs::read(hsaco_path).unwrap();
    let inspection = fe2o3_hsaco_finalize::inspect_unfinalized(&raw_hsaco).unwrap();
    let kernel_id = inspection
        .descriptor_table()
        .kernels()
        .iter()
        .find(|descriptor| descriptor.entry_name().as_str() == "scalar_gemm_v1")
        .expect("raw HSACO contains scalar_gemm_v1")
        .kernel_id();
    drop(inspection);

    let fixture = worker_v3_fixture::published_worker_v3_fixture_from_raw_hsaco(
        raw_hsaco,
        "scalar_gemm_v1",
        "scalar_gemm_v1.kd",
    );
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, kernel_id, &observed).unwrap();
    let mut verifier = ProductionScalarGemmWorkerV3VerifierV1::<scalar_gemm_v1_gpu::Marker>::open(
        runtime_root,
        120,
    )
    .unwrap();

    let audit = audit_recovered_worker_v3_verification_v1::<scalar_gemm_v1_gpu::Marker, _>(
        &admitted,
        &mut verifier,
    )
    .unwrap();
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    assert_eq!(
        audit.open_authority_obligations(),
        PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1
    );
    assert_ne!(audit.finalized_hsaco_sha256(), [0; 32]);
    assert_ne!(audit.finalized_hsaco_length(), 0);
    assert!(!audit.establishes_proof_executable_binding());
    assert!(!audit.can_enter_worker_v3_gate());
    assert!(!audit.grants_artifact_or_runtime_authority());
    let proof = audit.proof();
    assert!(proof.authenticates_retained_verus_execution());
    assert!(proof.binds_worker_v3_challenge());
    assert!(proof.establishes_exact_scalar_gemm_kir_profile());
    assert!(proof.establishes_kir_to_integer_model_refinement());
    assert!(!proof.establishes_source_to_kir_refinement());
    assert!(!proof.establishes_rust_or_f32_semantics());
    assert!(!proof.establishes_emitted_machine_refinement());
    assert!(!proof.can_enter_worker_v3_gate());
    assert!(!proof.grants_artifact_or_runtime_authority());
}

fn scalar_gemm_reference(a: &[f32], b: &[f32], m: u32, n: u32, k: u32) -> Vec<f32> {
    let mut output = vec![0.0; usize::try_from(m * n).unwrap()];
    for row in 0..usize::try_from(m).unwrap() {
        for column in 0..usize::try_from(n).unwrap() {
            for inner in 0..usize::try_from(k).unwrap() {
                output[row * n as usize + column] +=
                    a[row * k as usize + inner] * b[inner * n as usize + column];
            }
        }
    }
    output
}

#[test]
fn v3_host_admission_rejects_an_unknown_kernel_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xff; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
}

#[test]
fn v3_host_admission_rejects_incompatible_observed_target_features() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack+");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xa1; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::ObservedTargetMismatch)
    ));
}

#[test]
fn borrowed_v3_audit_preserves_exact_admission_custody_without_authority() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    let lineage = admitted.lineage_identity();
    let (finalized_sha256, finalized_length) = audit_recovered_worker_v3_verification_v1::<
        WorkerV3VecAddMarker,
        _,
    >(&admitted, &mut ReviewedTestWorkerV3Auditor)
    .unwrap();
    assert_ne!(finalized_sha256, [0; 32]);
    assert_ne!(finalized_length, 0);
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn v3_verification_rejects_a_substituted_finalized_hsaco_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    assert!(matches!(
        load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
            admitted,
            &mut ReviewedTestWorkerV3Verifier {
                substitute_finalized: true,
            },
            ReviewedTestHsaAdapter::new().0,
        ),
        Err(ProductionWorkerV3ApplicationLoadErrorV1::Verification(
            WorkerV3VerificationAuthenticationErrorV1::Decision(
                WorkerV3VerificationDecisionErrorV1::IdentityMismatch("finalized HSACO")
            )
        ))
    ));
}

#[test]
fn v3_hsa_load_rejects_and_cleans_up_a_substituted_adapter_digest() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    let (adapter, adapter_state) = ReviewedTestHsaAdapter::with_substituted_load_digest();
    assert!(matches!(
        load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
            admitted,
            &mut ReviewedTestWorkerV3Verifier {
                substitute_finalized: false,
            },
            adapter,
        ),
        Err(ProductionWorkerV3ApplicationLoadErrorV1::ExecutableLoad(
            fe2o3_host::WorkerV3HsaExecutableLoadErrorV1::LoadObservationMismatch {
                field: "finalized digest"
            }
        ))
    ));
    assert_eq!(adapter_state.unloads.load(Ordering::SeqCst), 1);
}
