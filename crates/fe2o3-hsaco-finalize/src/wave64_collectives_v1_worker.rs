//! Exact Worker V2 admission for the fixed masked Wave64 collectives profile.
//!
//! The compiler phase currently publishes no handoff, so this boundary requires
//! an independently pinned, complete handoff identity and source-authority
//! commitment. Reconstructing the closed LLVM module here proves correspondence
//! with the reviewed profile; it does not authenticate who produced the pin.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind, ExplicitValueType, KernelKind,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
use fe2o3_kernel_ir::{
    WAVE64_COLLECTIVES_V1_COMPLETE_COV6_KERNARG_BYTES, WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL,
    WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES, WAVE64_COLLECTIVES_V1_KERNEL_ID,
    WAVE64_COLLECTIVES_V1_SOURCE_SHA256, Wave64CollectivesProfileV1, verify_wave64_collectives_v1,
    wave64_collectives_v1_kernel_ir,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_kernel_binding_id_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoV1, WorkerCompilerFfiEnvelopeIdentityV2, WorkerInputKindV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerProtocolError, WorkerStageV1,
    WorkerV2RawHsacoInspectionError,
    worker_v2_hsaco_admission::{
        WorkerV2RawLaunchContractV1, WorkerV2RawLaunchDiagnosticProfileV1,
        inspect_worker_v2_raw_hsaco_with_launch_v1,
    },
};

const TARGET: &str = "gfx942:xnack-";
const COMPILER_CRATE_BINDING: &str =
    "ba3fa024069d9cee1b86cf6fc1ad80a77d9de5457de020b70182cdc265e64569";
const RUSTC_RELEASE: &str = "1.96.0-nightly";
const RUSTC_COMMIT: [u8; 20] = [
    0x55, 0xe8, 0x6c, 0x99, 0x68, 0x09, 0x90, 0x2e, 0x8b, 0xba, 0xd5, 0x12, 0xcf, 0xb4, 0xd2, 0xc1,
    0x8b, 0xe4, 0x46, 0xd9,
];
const PORTABLE_MIR_CLOSURE_IDENTITY: [u8; 32] = [
    0x55, 0x04, 0x3a, 0x3a, 0xc1, 0xaa, 0x25, 0xbd, 0x5e, 0x47, 0x58, 0x8b, 0x61, 0xc0, 0xb5, 0xfe,
    0xdd, 0x0c, 0x9f, 0x4e, 0xbd, 0x1c, 0x59, 0x25, 0x5d, 0x0c, 0xfd, 0xbb, 0xd3, 0x06, 0x41, 0x4c,
];
const CANONICAL_IR_BINDING: &[u8] = b"fe2o3::wave64_collectives_v1;args=input-f32-slice,active-mask-u64,three-lane-owned-f32-slices;ordered-collectives=reduce-sum,inclusive-scan-sum,exclusive-scan-sum;inactive=contribute-and-publish-positive-zero";
const DESCRIPTOR_PROFILE_BINDING: &[u8] = b"logical=wave64_collectives_v1;export=wave64_collectives_v1;descriptor=wave64_collectives_v1.kd;explicit-kernarg=72;complete-cov6-kernarg=328;wg=64,1,1;wave=64";
const COMPILER_AUTHORITY_SECTION: &str = ".fe2o3.wave64-auth.v1";
const PORTABLE_MIR_SECTION: &str = ".fe2o3.wave64-mir.v1";
const CANONICAL_IR_SECTION: &str = ".fe2o3.wave64-kir.v1";
const DESCRIPTOR_PROFILE_SECTION: &str = ".fe2o3.wave64-descriptor.v1";
const EXCHANGE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WAVE64-COLLECTIVES-V1/DIRECT-WORKER-DUAL-EXCHANGE/V1\0";
const FINAL_SYMBOLS: [&str; 2] = [
    WAVE64_COLLECTIVES_V1_KERNEL_ID,
    WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL,
];
const SUCCESS_DIAGNOSTICS: [&str; 6] = [
    "post_link.check=exports status=ok symbols=[wave64_collectives_v1,wave64_collectives_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.check=wave64_collectives_v1_profile status=ok workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=72 kernarg_size=328 kernarg_align=8 group_size=0 private_size=0 wavefront_size=64 calls=0 spills=0 dynamic_stack=false descriptor_binding=byte_exact rust_descriptor_admission=required",
    "post_link.kernel name=wave64_collectives_v1 symbol=wave64_collectives_v1.kd kernarg_size=328 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
];

/// Independently provisioned identity of the authenticated compiler selection.
///
/// Public construction is intentionally authority-free. The value only lets a
/// caller state which nonzero compiler authority it expects to see retained in
/// an exact handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesV1CompilerPinsV1 {
    source_authority: [u8; 32],
}

impl Wave64CollectivesV1CompilerPinsV1 {
    pub fn new(source_authority: [u8; 32]) -> Result<Self, Wave64CollectivesV1WorkerErrorV1> {
        if source_authority == [0; 32] {
            return Err(profile_mismatch("compiler source-authority pin"));
        }
        Ok(Self { source_authority })
    }

    pub const fn source_authority(&self) -> &[u8; 32] {
        &self.source_authority
    }

    pub const fn source_sha256(&self) -> &[u8; 32] {
        &WAVE64_COLLECTIVES_V1_SOURCE_SHA256
    }

    pub const fn portable_mir_sha256(&self) -> &[u8; 32] {
        &PORTABLE_MIR_CLOSURE_IDENTITY
    }

    pub fn canonical_kernel_ir_identity(&self) -> [u8; 32] {
        sha256(CANONICAL_IR_BINDING)
    }

    pub fn descriptor_profile_identity(&self) -> [u8; 32] {
        sha256(DESCRIPTOR_PROFILE_BINDING)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
}

/// Independent pins for the direct upstream-LLVM Worker V2 executable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesV1DirectWorkerPinsV1 {
    executable: ContentIdentityV1,
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
}

impl Wave64CollectivesV1DirectWorkerPinsV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Result<Self, Wave64CollectivesV1WorkerErrorV1> {
        if executable.byte_len() == 0 || executable.sha256() == &[0; 32] {
            return Err(profile_mismatch("worker executable pin"));
        }
        validate_identity_text(worker_build_identity, "worker build-identity pin")?;
        validate_identity_text(llvm_build_identity, "LLVM build-identity pin")?;
        Ok(Self {
            executable,
            worker_build_identity_sha256: sha256(worker_build_identity.as_bytes()),
            llvm_build_identity_sha256: sha256(llvm_build_identity.as_bytes()),
        })
    }

    pub const fn executable(self) -> ContentIdentityV1 {
        self.executable
    }

    pub const fn worker_build_identity_sha256(self) -> [u8; 32] {
        self.worker_build_identity_sha256
    }

    pub const fn llvm_build_identity_sha256(self) -> [u8; 32] {
        self.llvm_build_identity_sha256
    }
}

/// Exact out-of-band expectation for one Wave64 compiler handoff and worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesV1DirectWorkerExpectationV1 {
    handoff_sha256: [u8; 32],
    compiler: Wave64CollectivesV1CompilerPinsV1,
    worker: Wave64CollectivesV1DirectWorkerPinsV1,
}

impl Wave64CollectivesV1DirectWorkerExpectationV1 {
    pub fn from_pinned_handoff(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        compiler: Wave64CollectivesV1CompilerPinsV1,
        worker: Wave64CollectivesV1DirectWorkerPinsV1,
    ) -> Result<Self, Wave64CollectivesV1WorkerErrorV1> {
        if expected_handoff_sha256 == [0; 32]
            || handoff.identity().sha256() != &expected_handoff_sha256
        {
            return Err(profile_mismatch("pinned compiler handoff identity"));
        }
        validate_handoff(handoff, compiler)?;
        Ok(Self {
            handoff_sha256: expected_handoff_sha256,
            compiler,
            worker,
        })
    }

    pub const fn handoff_sha256(&self) -> &[u8; 32] {
        &self.handoff_sha256
    }

    pub const fn compiler_pins(self) -> Wave64CollectivesV1CompilerPinsV1 {
        self.compiler
    }

    pub const fn worker_pins(self) -> Wave64CollectivesV1DirectWorkerPinsV1 {
        self.worker
    }

    pub const fn authenticates_pin_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Stable identity of the two measured Worker V2 executions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Wave64CollectivesV1WorkerExchangeIdentityV1([u8; 32]);

impl Wave64CollectivesV1WorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert admission of the exact bootstrap and replay exchanges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedWave64CollectivesV1WorkerExchangeV1 {
    identity: Wave64CollectivesV1WorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    compiler: Wave64CollectivesV1CompilerPinsV1,
    worker: Wave64CollectivesV1DirectWorkerPinsV1,
}

impl ValidatedWave64CollectivesV1WorkerExchangeV1 {
    pub const fn identity(&self) -> Wave64CollectivesV1WorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn compiler_pins(&self) -> Wave64CollectivesV1CompilerPinsV1 {
        self.compiler
    }

    pub const fn worker_pins(&self) -> Wave64CollectivesV1DirectWorkerPinsV1 {
        self.worker
    }

    pub const fn code_object_version_was_inspected(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Exact raw-HSACO inspection retaining the consumed measured exchange.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedWave64CollectivesV1WorkerV2HsacoV1 {
    exchange: ValidatedWave64CollectivesV1WorkerExchangeV1,
    raw: InspectedRawWorkerV2HsacoV1,
}

impl InspectedWave64CollectivesV1WorkerV2HsacoV1 {
    pub const fn exchange(&self) -> ValidatedWave64CollectivesV1WorkerExchangeV1 {
        self.exchange
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn raw_inspection(&self) -> &InspectedRawWorkerV2HsacoV1 {
        &self.raw
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedWave64CollectivesV1WorkerExchangeV1,
        InspectedRawWorkerV2HsacoV1,
    ) {
        (self.exchange, self.raw)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Wave64CollectivesV1WorkerErrorV1 {
    CompilerHandoff(CompilerModuleHandoffErrorV2),
    WorkerProtocol(WorkerProtocolError),
    RawHsaco(WorkerV2RawHsacoInspectionError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for Wave64CollectivesV1WorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerHandoff(error) => write!(formatter, "Wave64 handoff failed: {error}"),
            Self::WorkerProtocol(error) => write!(formatter, "Wave64 Worker V2 failed: {error}"),
            Self::RawHsaco(error) => write!(formatter, "Wave64 raw HSACO failed: {error}"),
            Self::ProfileMismatch(field) => {
                write!(formatter, "Wave64 collectives V1 profile mismatch: {field}")
            }
        }
    }
}

impl Error for Wave64CollectivesV1WorkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerHandoff(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::RawHsaco(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Constructs the unique inert handoff expected by this finalizer profile.
///
/// This helper is deliberately public and authority-free so a later compiler
/// phase can share the exact bytes. Callers still need an independently
/// authenticated handoff identity before admission.
pub fn construct_inert_wave64_collectives_v1_compiler_handoff_v1(
    pins: Wave64CollectivesV1CompilerPinsV1,
) -> Result<CompilerModuleHandoffV2, Wave64CollectivesV1WorkerErrorV1> {
    let body = canonical_llvm_body()?;
    let descriptor = exact_descriptor_source(pins, body.as_bytes())?;
    let mut module = body.into_bytes();
    append_module_assembly_section(
        &mut module,
        COMPILER_DESCRIPTOR_SECTION_NAME_V1,
        descriptor.canonical_bytes(),
    );
    append_module_assembly_section(
        &mut module,
        COMPILER_AUTHORITY_SECTION,
        pins.source_authority(),
    );
    append_module_assembly_section(
        &mut module,
        PORTABLE_MIR_SECTION,
        pins.portable_mir_sha256(),
    );
    append_module_assembly_section(
        &mut module,
        CANONICAL_IR_SECTION,
        &pins.canonical_kernel_ir_identity(),
    );
    append_module_assembly_section(
        &mut module,
        DESCRIPTOR_PROFILE_SECTION,
        &pins.descriptor_profile_identity(),
    );
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        exact_target(),
        CodeObjectVersion::V6,
        exact_envelope()?,
        exact_manifest()?,
        &module,
    )
    .map_err(Wave64CollectivesV1WorkerErrorV1::CompilerHandoff)
}

/// Validates both measured Worker V2 executions without consuming HSACO bytes.
pub fn validate_wave64_collectives_v1_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: Wave64CollectivesV1DirectWorkerExpectationV1,
) -> Result<ValidatedWave64CollectivesV1WorkerExchangeV1, Wave64CollectivesV1WorkerErrorV1> {
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(Wave64CollectivesV1WorkerErrorV1::WorkerProtocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(Wave64CollectivesV1WorkerErrorV1::WorkerProtocol)?;
    let expected_handoff =
        construct_inert_wave64_collectives_v1_compiler_handoff_v1(expected.compiler)?;
    if expected_handoff.identity().sha256() != &expected.handoff_sha256 {
        return Err(profile_mismatch("reconstructed compiler handoff identity"));
    }
    for exchange in [&bootstrap, &replay] {
        validate_exchange(exchange, &expected_handoff, expected.worker)?;
    }
    if bootstrap.response().output().map(|value| value.bytes())
        != replay.response().output().map(|value| value.bytes())
    {
        return Err(profile_mismatch("bootstrap/replay output determinism"));
    }
    if source.compiler_envelope() != expected_handoff.envelope()
        || source.symbol_manifest() != expected_handoff.symbol_manifest()
        || source.plan().target() != replay.request().target()
        || source.worker_measurement().executable() != expected.worker.executable
        || sha256(
            source
                .worker_measurement()
                .worker_build_identity()
                .as_bytes(),
        ) != expected.worker.worker_build_identity_sha256
        || sha256(source.worker_measurement().llvm_build_identity().as_bytes())
            != expected.worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("retained first-build lineage"));
    }
    let output = replay
        .response()
        .output()
        .ok_or_else(|| profile_mismatch("completed replay output"))?;
    if source.output_identity() != output.identity()
        || !source.output_identity().matches(source.output_bytes())
    {
        return Err(profile_mismatch("retained linked-output identity"));
    }
    Ok(ValidatedWave64CollectivesV1WorkerExchangeV1 {
        identity: calculate_exchange_identity(&bootstrap, &replay),
        compiler_module: replay.request().compiler_module().identity(),
        linked_output: output.identity(),
        compiler: expected.compiler,
        worker: expected.worker,
    })
}

/// Consumes the measured exchange and independently inspects exact COV6 HSACO.
pub fn inspect_wave64_collectives_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: Wave64CollectivesV1DirectWorkerExpectationV1,
) -> Result<InspectedWave64CollectivesV1WorkerV2HsacoV1, Wave64CollectivesV1WorkerErrorV1> {
    let exchange = validate_wave64_collectives_v1_worker_exchange_v1(&source, expected)?;
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::WAVE64_COLLECTIVES_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1,
    )
    .map_err(Wave64CollectivesV1WorkerErrorV1::RawHsaco)?;
    if raw.target() != exact_target() || raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("inspected target/code-object version"));
    }
    if raw.policy().observed_kernels().len() != 1
        || raw.policy().observed_kernels()[0].entry() != WAVE64_COLLECTIVES_V1_KERNEL_ID
        || raw.policy().observed_kernels()[0].descriptor()
            != WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL
    {
        return Err(profile_mismatch("inspected kernel/descriptor closure"));
    }
    validate_hsaco_metadata(raw.exact_bytes())?;
    Ok(InspectedWave64CollectivesV1WorkerV2HsacoV1 { exchange, raw })
}

fn validate_handoff(
    handoff: &CompilerModuleHandoffV2,
    pins: Wave64CollectivesV1CompilerPinsV1,
) -> Result<(), Wave64CollectivesV1WorkerErrorV1> {
    let expected = construct_inert_wave64_collectives_v1_compiler_handoff_v1(pins)?;
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr
        || handoff.target() != expected.target()
        || handoff.code_object_version() != CodeObjectVersion::V6
        || handoff.envelope() != expected.envelope()
        || handoff.symbol_manifest() != expected.symbol_manifest()
        || handoff.module_bytes() != expected.module_bytes()
        || handoff.canonical_bytes() != expected.canonical_bytes()
    {
        return Err(profile_mismatch("complete canonical compiler handoff"));
    }
    Ok(())
}

fn validate_exchange(
    exchange: &InertDecodedWorkerExchangeV2,
    handoff: &CompilerModuleHandoffV2,
    worker: Wave64CollectivesV1DirectWorkerPinsV1,
) -> Result<(), Wave64CollectivesV1WorkerErrorV1> {
    let request = exchange.request();
    let response = exchange.response();
    if request.target() != exact_target()
        || request.code_object_version() != CodeObjectVersion::V6
        || request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
        || request.compiler_envelope_identity()
            != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
                handoff.envelope().identity(),
            )
        || request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr
        || request.compiler_module().bytes() != handoff.module_bytes()
        || !request.external_providers().is_empty()
        || !request.import_symbols().is_empty()
        || !request.export_symbols().is_empty()
        || request.final_symbols() != FINAL_SYMBOLS
        || request.worker_executable() != worker.executable
        || sha256(request.worker_build_identity().as_bytes()) != worker.worker_build_identity_sha256
        || sha256(request.llvm_build_identity().as_bytes()) != worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("exact Worker V2 request"));
    }
    if !response.binds_request(request)
        || response.worker_build_identity() != request.worker_build_identity()
        || response.stage() != WorkerStageV1::Complete
        || response.device_library_provider().is_some()
        || response.diagnostics().len() != SUCCESS_DIAGNOSTICS.len()
        || response
            .diagnostics()
            .iter()
            .zip(SUCCESS_DIAGNOSTICS)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(profile_mismatch("exact Worker V2 response"));
    }
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed Worker V2 output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || output.identity().byte_len() > request.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("Worker V2 output binding"));
    }
    Ok(())
}

fn validate_hsaco_metadata(bytes: &[u8]) -> Result<(), Wave64CollectivesV1WorkerErrorV1> {
    let inspected =
        fe2o3_hsaco::inspect(bytes).map_err(|_| profile_mismatch("independent COV6 metadata"))?;
    let [kernel] = inspected.kernels() else {
        return Err(profile_mismatch("metadata kernel count"));
    };
    if kernel.name() != WAVE64_COLLECTIVES_V1_KERNEL_ID
        || kernel.symbol() != WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL
        || kernel.kernarg_segment_size()
            != u64::from(WAVE64_COLLECTIVES_V1_COMPLETE_COV6_KERNARG_BYTES)
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset()
            != Some(u64::from(WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES))
        || kernel.implicit_argument_size() != 256
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.wavefront_size() != 64
        || kernel.max_flat_workgroup_size() != 64
        || kernel.required_workgroup_size() != Some([64, 1, 1])
        || kernel.cluster_dims().is_some()
        || kernel.kind() != KernelKind::Normal
        || kernel.uniform_work_group_size()
        || kernel.uses_dynamic_stack()
        || kernel.device_enqueue_symbol().is_some()
        || kernel.sgpr_count() == 0
        || kernel.vgpr_count() == 0
        || kernel.sgpr_spill_count().unwrap_or(0) != 0
        || kernel.vgpr_spill_count().unwrap_or(0) != 0
    {
        return Err(profile_mismatch("metadata launch/resource closure"));
    }
    const FIELDS: [(u64, u64, ExplicitValueKind, Option<ExplicitValueType>); 9] = [
        (
            0,
            8,
            ExplicitValueKind::GlobalBuffer,
            Some(ExplicitValueType::F32),
        ),
        (
            8,
            8,
            ExplicitValueKind::ByValue,
            Some(ExplicitValueType::U64),
        ),
        (
            16,
            8,
            ExplicitValueKind::ByValue,
            Some(ExplicitValueType::U64),
        ),
        (
            24,
            8,
            ExplicitValueKind::GlobalBuffer,
            Some(ExplicitValueType::F32),
        ),
        (
            32,
            8,
            ExplicitValueKind::ByValue,
            Some(ExplicitValueType::U64),
        ),
        (
            40,
            8,
            ExplicitValueKind::GlobalBuffer,
            Some(ExplicitValueType::F32),
        ),
        (
            48,
            8,
            ExplicitValueKind::ByValue,
            Some(ExplicitValueType::U64),
        ),
        (
            56,
            8,
            ExplicitValueKind::GlobalBuffer,
            Some(ExplicitValueType::F32),
        ),
        (
            64,
            8,
            ExplicitValueKind::ByValue,
            Some(ExplicitValueType::U64),
        ),
    ];
    if kernel.explicit_arguments().len() != FIELDS.len() {
        return Err(profile_mismatch("metadata explicit argument count"));
    }
    const ABI_FIELDS: [&str; 9] = [
        "metadata input ABI",
        "metadata input length ABI",
        "metadata active mask ABI",
        "metadata reduction output ABI",
        "metadata reduction length ABI",
        "metadata inclusive output ABI",
        "metadata inclusive length ABI",
        "metadata exclusive output ABI",
        "metadata exclusive length ABI",
    ];
    for (index, (argument, expected)) in kernel.explicit_arguments().iter().zip(FIELDS).enumerate()
    {
        if (argument.offset(), argument.size(), argument.value_kind())
            != (expected.0, expected.1, expected.2)
            || (argument.value_type().is_some() && argument.value_type() != expected.3)
        {
            return Err(profile_mismatch(ABI_FIELDS[index]));
        }
        if matches!(index, 0 | 3 | 5 | 7) {
            if argument.address_space() != Some(ArgumentAddressSpace::Global) {
                return Err(profile_mismatch("metadata pointer address space"));
            }
            if argument
                .pointee_alignment()
                .is_some_and(|alignment| alignment != 4)
            {
                return Err(profile_mismatch("metadata pointer pointee alignment"));
            }
            if argument.access()
                != Some(if index == 0 {
                    ArgumentAccess::ReadOnly
                } else {
                    ArgumentAccess::WriteOnly
                })
            {
                return Err(profile_mismatch("metadata pointer access"));
            }
        }
    }
    Ok(())
}

fn canonical_llvm_body() -> Result<String, Wave64CollectivesV1WorkerErrorV1> {
    verify_wave64_collectives_v1(
        &wave64_collectives_v1_kernel_ir(),
        &Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
    .map_err(|_| profile_mismatch("canonical semantic Kernel IR"))?;
    let llvm = CANONICAL_LLVM_BODY.to_owned();
    audit_canonical_llvm(&llvm)?;
    Ok(llvm)
}

fn audit_canonical_llvm(llvm: &str) -> Result<(), Wave64CollectivesV1WorkerErrorV1> {
    let required = [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @wave64_collectives_v1(",
        "i64 %active_mask",
        "call i32 @llvm.amdgcn.workitem.id.x()",
        "call i32 @llvm.amdgcn.ds.bpermute",
        "call void @llvm.trap()",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"fp-contract\"=\"off\"",
        "!0 = !{i32 64, i32 1, i32 1}",
    ];
    if required.iter().any(|needle| !llvm.contains(needle))
        || llvm.matches("define amdgpu_kernel").count() != 1
        || llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count() != 13
        || llvm.matches("fadd float").count() != 12
        || llvm.matches("store float").count() != 3
        || llvm.matches("load float").count() != 1
        || [" fast ", " reassoc ", " contract ", "COMGR", "comgr"]
            .iter()
            .any(|forbidden| llvm.contains(forbidden))
    {
        return Err(profile_mismatch("canonical upstream-LLVM lowering"));
    }
    Ok(())
}

fn exact_descriptor_source(
    pins: Wave64CollectivesV1CompilerPinsV1,
    llvm_body: &[u8],
) -> Result<CompilerDescriptorSourceV1, Wave64CollectivesV1WorkerErrorV1> {
    let shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let scalar_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U64));
    let disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let arguments = vec![
        LogicalArgumentV1::shared_slice(0, name("input")?, &shared_source, &shared_layout, 0)
            .map_err(|_| profile_mismatch("input descriptor"))?,
        LogicalArgumentV1::scalar(1, name("active_mask")?, &scalar_source, &scalar_layout, 16)
            .map_err(|_| profile_mismatch("mask descriptor"))?,
        LogicalArgumentV1::disjoint_slice(
            2,
            name("reduction_output")?,
            &disjoint_source,
            &disjoint_layout,
            AccessMode::WriteOnly,
            24,
        )
        .map_err(|_| profile_mismatch("reduction descriptor"))?,
        LogicalArgumentV1::disjoint_slice(
            3,
            name("inclusive_output")?,
            &disjoint_source,
            &disjoint_layout,
            AccessMode::WriteOnly,
            40,
        )
        .map_err(|_| profile_mismatch("inclusive descriptor"))?,
        LogicalArgumentV1::disjoint_slice(
            4,
            name("exclusive_output")?,
            &disjoint_source,
            &disjoint_layout,
            AccessMode::WriteOnly,
            56,
        )
        .map_err(|_| profile_mismatch("exclusive descriptor"))?,
    ];
    let compiler_binding = CrateBindingIdV1::from_hex(COMPILER_CRATE_BINDING)
        .map_err(|_| profile_mismatch("compiler crate binding"))?;
    let kernel_binding = derive_kernel_binding_id_v1(
        compiler_binding,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        WAVE64_COLLECTIVES_V1_KERNEL_ID,
        WAVE64_COLLECTIVES_V1_KERNEL_ID,
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(kernel_binding.as_bytes()),
        name(WAVE64_COLLECTIVES_V1_KERNEL_ID)?,
        name(WAVE64_COLLECTIVES_V1_KERNEL_ID)?,
        name(WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL)?,
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(WAVE64_COLLECTIVES_V1_SOURCE_SHA256),
            EvidenceDigest::from_sha256_bytes(*pins.source_authority()),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(pins.canonical_kernel_ir_identity()),
            EvidenceDigest::from_sha256_bytes(sha256(llvm_body)),
        ),
        vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(
            WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES,
            WAVE64_COLLECTIVES_V1_COMPLETE_COV6_KERNARG_BYTES,
            8,
        )
        .map_err(|_| profile_mismatch("descriptor kernarg layout"))?,
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(
                DimensionsV1::new(64, 1, 1)
                    .map_err(|_| profile_mismatch("descriptor workgroup"))?,
            ),
            DimensionsV1::new(1, 1, 1).map_err(|_| profile_mismatch("descriptor grid"))?,
            64,
            0,
            0,
        )
        .map_err(|_| profile_mismatch("descriptor launch"))?,
        arguments,
    )
    .map_err(|_| profile_mismatch("kernel descriptor"))?;
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            text("rustc-codegen-fe2o3")?,
            text(RUSTC_RELEASE)?,
            RUSTC_COMMIT,
        ),
        ProducerIdentityV1::new(
            text("rustc-codegen-fe2o3-worker-v2")?,
            text("typed-wave64-collectives-gfx942-cov6-v1")?,
        ),
        exact_target(),
        vec![shared_source, scalar_source, disjoint_source],
        vec![shared_layout, scalar_layout, disjoint_layout],
        vec![kernel],
    )
    .map_err(|_| profile_mismatch("device descriptor table"))?;
    CompilerDescriptorSourceV1::new(table)
        .map_err(|_| profile_mismatch("compiler descriptor source"))
}

fn exact_envelope() -> Result<CompilerFfiEnvelopeV1, Wave64CollectivesV1WorkerErrorV1> {
    CompilerFfiEnvelopeV1::for_module_without_device_ffi(exact_target(), CodeObjectVersion::V6)
        .map_err(|_| profile_mismatch("empty compiler FFI envelope"))
}

fn exact_manifest() -> Result<CompilerModuleSymbolManifestV1, Wave64CollectivesV1WorkerErrorV1> {
    CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            WAVE64_COLLECTIVES_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL,
        ),
    ])
    .map_err(|_| profile_mismatch("compiler symbol manifest"))
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(TARGET).expect("fixed Wave64 target is valid")
}

fn calculate_exchange_identity(
    bootstrap: &InertDecodedWorkerExchangeV2,
    replay: &InertDecodedWorkerExchangeV2,
) -> Wave64CollectivesV1WorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN);
    for exchange in [bootstrap, replay] {
        hash_field(&mut digest, exchange.request().canonical_bytes());
        hash_field(&mut digest, exchange.response().canonical_bytes());
    }
    Wave64CollectivesV1WorkerExchangeIdentityV1(digest.finalize().into())
}

fn append_module_assembly_section(module: &mut Vec<u8>, section: &str, bytes: &[u8]) {
    module.extend_from_slice(
        format!(
            "\nmodule asm \".section {section},\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n"
        )
        .as_bytes(),
    );
    for chunk in bytes.chunks(16) {
        module.extend_from_slice(b"module asm \".byte ");
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
                module.extend_from_slice(b", ");
            }
            module.extend_from_slice(format!("0x{byte:02x}").as_bytes());
        }
        module.extend_from_slice(b"\"\n");
    }
}

fn validate_identity_text(
    value: &str,
    field: &'static str,
) -> Result<(), Wave64CollectivesV1WorkerErrorV1> {
    if value.is_empty()
        || value.len() > crate::MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(profile_mismatch(field));
    }
    Ok(())
}

fn name(value: &str) -> Result<ValidName, Wave64CollectivesV1WorkerErrorV1> {
    ValidName::new(value).map_err(|_| profile_mismatch("descriptor name"))
}

fn text(value: &str) -> Result<Text, Wave64CollectivesV1WorkerErrorV1> {
    Text::new(value).map_err(|_| profile_mismatch("descriptor text"))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

const fn profile_mismatch(field: &'static str) -> Wave64CollectivesV1WorkerErrorV1 {
    Wave64CollectivesV1WorkerErrorV1::ProfileMismatch(field)
}

const CANONICAL_LLVM_BODY: &str = r#"target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.ds.bpermute(i32, i32) #2
declare void @llvm.trap() #3

define amdgpu_kernel void @wave64_collectives_v1(ptr addrspace(1) noalias nocapture readonly align 4 %input.data, i64 %input.len, i64 %active_mask, ptr addrspace(1) noalias nocapture writeonly align 4 %reduction_output.data, i64 %reduction_output.len, ptr addrspace(1) noalias nocapture writeonly align 4 %inclusive_output.data, i64 %inclusive_output.len, ptr addrspace(1) noalias nocapture writeonly align 4 %exclusive_output.data, i64 %exclusive_output.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %lane.ok = icmp ult i32 %lane, 64
  %input.ok = icmp eq i64 %input.len, 64
  %reduction.ok = icmp eq i64 %reduction_output.len, 64
  %inclusive.ok = icmp eq i64 %inclusive_output.len, 64
  %exclusive.ok = icmp eq i64 %exclusive_output.len, 64
  %lengths.0 = and i1 %input.ok, %reduction.ok
  %lengths.1 = and i1 %inclusive.ok, %exclusive.ok
  %lengths.ok = and i1 %lengths.0, %lengths.1
  %valid = and i1 %lane.ok, %lengths.ok
  br i1 %valid, label %body, label %trap

trap:
  call void @llvm.trap()
  unreachable

body:
  %lane64 = zext i32 %lane to i64
  %mask.shifted = lshr i64 %active_mask, %lane64
  %mask.bit = and i64 %mask.shifted, 1
  %active = icmp eq i64 %mask.bit, 1
  %input.ptr = getelementptr inbounds float, ptr addrspace(1) %input.data, i64 %lane64
  %input.value = load float, ptr addrspace(1) %input.ptr, align 4
  %contribution = select i1 %active, float %input.value, float 0.000000e+00
  %r.bits.0 = bitcast float %contribution to i32
  %r.lane.1 = xor i32 %lane, 1
  %r.byte.1 = shl i32 %r.lane.1, 2
  %r.peer.bits.1 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.1, i32 %r.bits.0)
  %r.peer.1 = bitcast i32 %r.peer.bits.1 to float
  %r.1 = fadd float %contribution, %r.peer.1
  %r.bits.1 = bitcast float %r.1 to i32
  %r.lane.2 = xor i32 %lane, 2
  %r.byte.2 = shl i32 %r.lane.2, 2
  %r.peer.bits.2 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.2, i32 %r.bits.1)
  %r.peer.2 = bitcast i32 %r.peer.bits.2 to float
  %r.2 = fadd float %r.1, %r.peer.2
  %r.bits.2 = bitcast float %r.2 to i32
  %r.lane.4 = xor i32 %lane, 4
  %r.byte.4 = shl i32 %r.lane.4, 2
  %r.peer.bits.4 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.4, i32 %r.bits.2)
  %r.peer.4 = bitcast i32 %r.peer.bits.4 to float
  %r.4 = fadd float %r.2, %r.peer.4
  %r.bits.4 = bitcast float %r.4 to i32
  %r.lane.8 = xor i32 %lane, 8
  %r.byte.8 = shl i32 %r.lane.8, 2
  %r.peer.bits.8 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.8, i32 %r.bits.4)
  %r.peer.8 = bitcast i32 %r.peer.bits.8 to float
  %r.8 = fadd float %r.4, %r.peer.8
  %r.bits.8 = bitcast float %r.8 to i32
  %r.lane.16 = xor i32 %lane, 16
  %r.byte.16 = shl i32 %r.lane.16, 2
  %r.peer.bits.16 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.16, i32 %r.bits.8)
  %r.peer.16 = bitcast i32 %r.peer.bits.16 to float
  %r.16 = fadd float %r.8, %r.peer.16
  %r.bits.16 = bitcast float %r.16 to i32
  %r.lane.32 = xor i32 %lane, 32
  %r.byte.32 = shl i32 %r.lane.32, 2
  %r.peer.bits.32 = call i32 @llvm.amdgcn.ds.bpermute(i32 %r.byte.32, i32 %r.bits.16)
  %r.peer.32 = bitcast i32 %r.peer.bits.32 to float
  %reduction = fadd float %r.16, %r.peer.32
  %s.has.1 = icmp uge i32 %lane, 1
  %s.sub.1 = sub i32 %lane, 1
  %s.lane.1 = select i1 %s.has.1, i32 %s.sub.1, i32 %lane
  %s.byte.1 = shl i32 %s.lane.1, 2
  %s.bits.0 = bitcast float %contribution to i32
  %s.peer.bits.1 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.1, i32 %s.bits.0)
  %s.peer.1 = bitcast i32 %s.peer.bits.1 to float
  %s.sum.1 = fadd float %contribution, %s.peer.1
  %s.1 = select i1 %s.has.1, float %s.sum.1, float %contribution
  %s.has.2 = icmp uge i32 %lane, 2
  %s.sub.2 = sub i32 %lane, 2
  %s.lane.2 = select i1 %s.has.2, i32 %s.sub.2, i32 %lane
  %s.byte.2 = shl i32 %s.lane.2, 2
  %s.bits.1 = bitcast float %s.1 to i32
  %s.peer.bits.2 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.2, i32 %s.bits.1)
  %s.peer.2 = bitcast i32 %s.peer.bits.2 to float
  %s.sum.2 = fadd float %s.1, %s.peer.2
  %s.2 = select i1 %s.has.2, float %s.sum.2, float %s.1
  %s.has.4 = icmp uge i32 %lane, 4
  %s.sub.4 = sub i32 %lane, 4
  %s.lane.4 = select i1 %s.has.4, i32 %s.sub.4, i32 %lane
  %s.byte.4 = shl i32 %s.lane.4, 2
  %s.bits.2 = bitcast float %s.2 to i32
  %s.peer.bits.4 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.4, i32 %s.bits.2)
  %s.peer.4 = bitcast i32 %s.peer.bits.4 to float
  %s.sum.4 = fadd float %s.2, %s.peer.4
  %s.4 = select i1 %s.has.4, float %s.sum.4, float %s.2
  %s.has.8 = icmp uge i32 %lane, 8
  %s.sub.8 = sub i32 %lane, 8
  %s.lane.8 = select i1 %s.has.8, i32 %s.sub.8, i32 %lane
  %s.byte.8 = shl i32 %s.lane.8, 2
  %s.bits.4 = bitcast float %s.4 to i32
  %s.peer.bits.8 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.8, i32 %s.bits.4)
  %s.peer.8 = bitcast i32 %s.peer.bits.8 to float
  %s.sum.8 = fadd float %s.4, %s.peer.8
  %s.8 = select i1 %s.has.8, float %s.sum.8, float %s.4
  %s.has.16 = icmp uge i32 %lane, 16
  %s.sub.16 = sub i32 %lane, 16
  %s.lane.16 = select i1 %s.has.16, i32 %s.sub.16, i32 %lane
  %s.byte.16 = shl i32 %s.lane.16, 2
  %s.bits.8 = bitcast float %s.8 to i32
  %s.peer.bits.16 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.16, i32 %s.bits.8)
  %s.peer.16 = bitcast i32 %s.peer.bits.16 to float
  %s.sum.16 = fadd float %s.8, %s.peer.16
  %s.16 = select i1 %s.has.16, float %s.sum.16, float %s.8
  %s.has.32 = icmp uge i32 %lane, 32
  %s.sub.32 = sub i32 %lane, 32
  %s.lane.32 = select i1 %s.has.32, i32 %s.sub.32, i32 %lane
  %s.byte.32 = shl i32 %s.lane.32, 2
  %s.bits.16 = bitcast float %s.16 to i32
  %s.peer.bits.32 = call i32 @llvm.amdgcn.ds.bpermute(i32 %s.byte.32, i32 %s.bits.16)
  %s.peer.32 = bitcast i32 %s.peer.bits.32 to float
  %s.sum.32 = fadd float %s.16, %s.peer.32
  %inclusive = select i1 %s.has.32, float %s.sum.32, float %s.16
  %e.has = icmp ne i32 %lane, 0
  %e.sub = sub i32 %lane, 1
  %e.lane = select i1 %e.has, i32 %e.sub, i32 %lane
  %e.byte = shl i32 %e.lane, 2
  %e.bits = bitcast float %inclusive to i32
  %e.peer.bits = call i32 @llvm.amdgcn.ds.bpermute(i32 %e.byte, i32 %e.bits)
  %e.peer = bitcast i32 %e.peer.bits to float
  %exclusive = select i1 %e.has, float %e.peer, float 0.000000e+00
  %published.reduction = select i1 %active, float %reduction, float 0.000000e+00
  %published.inclusive = select i1 %active, float %inclusive, float 0.000000e+00
  %published.exclusive = select i1 %active, float %exclusive, float 0.000000e+00
  %reduction.ptr = getelementptr inbounds float, ptr addrspace(1) %reduction_output.data, i64 %lane64
  %inclusive.ptr = getelementptr inbounds float, ptr addrspace(1) %inclusive_output.data, i64 %lane64
  %exclusive.ptr = getelementptr inbounds float, ptr addrspace(1) %exclusive_output.data, i64 %lane64
  store float %published.reduction, ptr addrspace(1) %reduction.ptr, align 4
  store float %published.inclusive, ptr addrspace(1) %inclusive.ptr, align 4
  store float %published.exclusive, ptr addrspace(1) %exclusive.ptr, align 4
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" "fp-contract"="off" }
attributes #1 = { nounwind readnone speculatable willreturn }
attributes #2 = { convergent nounwind }
attributes #3 = { cold noreturn nounwind }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{!"read_only", !"none", !"none", !"write_only", !"none", !"write_only", !"none", !"write_only", !"none"}
!2 = !{!"float*", !"ulong", !"ulong", !"float*", !"ulong", !"float*", !"ulong", !"float*", !"ulong"}
!3 = !{!"const", !"", !"", !"restrict", !"", !"restrict", !"", !"restrict", !""}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_descriptor::{AliasSemantics, OwnershipSemantics, PhysicalAbiComponentKind};

    fn pins(seed: u8) -> Wave64CollectivesV1CompilerPinsV1 {
        Wave64CollectivesV1CompilerPinsV1::new([seed; 32]).unwrap()
    }

    #[test]
    fn duplicated_bindings_match_the_integrated_compiler_profile() {
        let integrated =
            include_str!("../../rustc-codegen-fe2o3/src/collected_wave64_collectives_v1.rs");
        for binding in [
            COMPILER_CRATE_BINDING,
            RUSTC_RELEASE,
            "55e86c996809902e8bbad512cfb4d2c18be446d9",
            std::str::from_utf8(CANONICAL_IR_BINDING).unwrap(),
            std::str::from_utf8(DESCRIPTOR_PROFILE_BINDING).unwrap(),
        ] {
            assert!(integrated.contains(binding), "missing binding {binding}");
        }
        let portable_mir = PORTABLE_MIR_CLOSURE_IDENTITY
            .iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(",");
        let compact_integrated = integrated.split_whitespace().collect::<String>();
        assert!(compact_integrated.contains(&portable_mir));
    }

    #[test]
    fn exact_handoff_binds_source_mir_kir_descriptor_and_abi() {
        let compiler_pins = pins(0xa5);
        let handoff =
            construct_inert_wave64_collectives_v1_compiler_handoff_v1(compiler_pins).unwrap();
        let worker = Wave64CollectivesV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x22; 32], 4096),
            "fe2o3-direct-llvm-lld-worker-v2-wave64",
            "upstream-llvm-22-wave64",
        )
        .unwrap();
        let expectation = Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
            &handoff,
            *handoff.identity().sha256(),
            compiler_pins,
            worker,
        )
        .unwrap();
        assert_eq!(expectation.handoff_sha256(), handoff.identity().sha256());
        assert!(!expectation.authenticates_pin_origin());
        assert!(!expectation.grants_launch_authority());
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        for section in [
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            COMPILER_AUTHORITY_SECTION,
            PORTABLE_MIR_SECTION,
            CANONICAL_IR_SECTION,
            DESCRIPTOR_PROFILE_SECTION,
        ] {
            assert!(text.contains(section));
        }
        assert_eq!(
            text.matches("call i32 @llvm.amdgcn.ds.bpermute").count(),
            13
        );
        assert_eq!(text.matches("store float").count(), 3);
    }

    #[test]
    fn every_pin_byte_and_each_semantic_module_region_is_fail_closed() {
        let compiler_pins = pins(0xa5);
        let handoff =
            construct_inert_wave64_collectives_v1_compiler_handoff_v1(compiler_pins).unwrap();
        let worker = Wave64CollectivesV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x22; 32], 4096),
            "worker",
            "llvm",
        )
        .unwrap();
        for index in 0..32 {
            let mut wrong_identity = *handoff.identity().sha256();
            wrong_identity[index] ^= 1;
            assert!(
                Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
                    &handoff,
                    wrong_identity,
                    compiler_pins,
                    worker,
                )
                .is_err()
            );
            let mut wrong_authority = *compiler_pins.source_authority();
            wrong_authority[index] ^= 1;
            let wrong_pins = Wave64CollectivesV1CompilerPinsV1::new(wrong_authority).unwrap();
            assert!(
                Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
                    &handoff,
                    *handoff.identity().sha256(),
                    wrong_pins,
                    worker,
                )
                .is_err()
            );
        }
        assert!(Wave64CollectivesV1CompilerPinsV1::new([0; 32]).is_err());
        assert!(
            Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
                &handoff,
                *handoff.identity().sha256(),
                pins(0xa4),
                worker,
            )
            .is_err()
        );

        for needle in [
            "gfx942",
            "xnack",
            "%active_mask",
            "%mask.shifted",
            "%contribution",
            "%reduction",
            "%inclusive",
            "%exclusive",
            "%published.reduction",
            "%published.inclusive",
            "%published.exclusive",
            "amdgpu-flat-work-group-size",
            "reqd_work_group_size",
            "kernel_arg_access_qual",
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            COMPILER_AUTHORITY_SECTION,
            PORTABLE_MIR_SECTION,
            CANONICAL_IR_SECTION,
            DESCRIPTOR_PROFILE_SECTION,
        ] {
            let mut module = handoff.module_bytes().to_vec();
            let index = module
                .windows(needle.len())
                .position(|window| window == needle.as_bytes())
                .unwrap_or_else(|| panic!("missing mutation region {needle}"));
            module[index] ^= 1;
            let mutated = CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                handoff.target(),
                handoff.code_object_version(),
                handoff.envelope().clone(),
                handoff.symbol_manifest().clone(),
                &module,
            );
            if let Ok(mutated) = mutated {
                assert!(
                    validate_handoff(&mutated, compiler_pins).is_err(),
                    "region {needle}"
                );
            }
        }
    }

    #[test]
    fn descriptor_retains_five_logical_arguments_and_exact_physical_components() {
        let pins = pins(0x5a);
        let body = canonical_llvm_body().unwrap();
        let source = exact_descriptor_source(pins, body.as_bytes()).unwrap();
        let [kernel] = source.table().kernels() else {
            panic!("exactly one descriptor")
        };
        assert_eq!(kernel.arguments().len(), 5);
        assert_eq!(kernel.abi_layout().explicit_argument_size(), 72);
        assert_eq!(kernel.abi_layout().kernarg_segment_size(), 328);
        assert_eq!(
            kernel.capabilities(),
            [CapabilityV1::Subgroup, CapabilityV1::AmdWave]
        );
        let expected = [
            vec![
                (PhysicalAbiComponentKind::GlobalPointer, 0, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, 8, 8, 8),
            ],
            vec![(
                PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U64),
                16,
                8,
                8,
            )],
            vec![
                (PhysicalAbiComponentKind::GlobalPointer, 24, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, 32, 8, 8),
            ],
            vec![
                (PhysicalAbiComponentKind::GlobalPointer, 40, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, 48, 8, 8),
            ],
            vec![
                (PhysicalAbiComponentKind::GlobalPointer, 56, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, 64, 8, 8),
            ],
        ];
        for (argument, expected) in kernel.arguments().iter().zip(expected) {
            assert_eq!(argument.physical_components().collect::<Vec<_>>(), expected);
        }
        assert_eq!(
            kernel.arguments()[0].ownership(),
            OwnershipSemantics::SharedBorrow
        );
        for output in &kernel.arguments()[2..] {
            assert_eq!(output.ownership(), OwnershipSemantics::UniqueBorrow);
            assert_eq!(output.alias(), AliasSemantics::Exclusive);
        }
    }

    #[test]
    fn lowering_has_exact_mask_collective_publication_and_no_comgr_markers() {
        let llvm = canonical_llvm_body().unwrap();
        for marker in [
            "%mask.shifted = lshr i64 %active_mask, %lane64",
            "%contribution = select i1 %active, float %input.value, float 0.000000e+00",
            "%published.reduction = select i1 %active",
            "%published.inclusive = select i1 %active",
            "%published.exclusive = select i1 %active",
        ] {
            assert!(llvm.contains(marker));
        }
        assert!(!llvm.contains("COMGR"));
        assert!(!llvm.contains("comgr"));
        assert!(!llvm.contains(" fast "));
    }
}
