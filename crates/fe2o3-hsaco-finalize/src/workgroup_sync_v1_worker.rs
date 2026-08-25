//! Exact direct upstream-LLVM Worker V2 admission for workgroup sync V1.
//!
//! The integrated compiler profile currently publishes no public handoff. This
//! module therefore reconstructs two inert, byte-exact handoffs from the
//! reviewed compiler identities. Matching those bytes does not authenticate
//! who selected the pins and does not prove compiler refinement.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind, ExplicitValueType, HiddenValueKind,
    KernelKind,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
use fe2o3_kernel_ir::{
    LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES, LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES,
    LdsReductionProfileV1, SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES,
    SCOPED_ATOMIC_V1_EXPLICIT_KERNARG_BYTES, ScopedAtomicProfileV1, lds_reduction_v1_kernel_ir,
    scoped_atomic_v1_kernel_ir, verify_lds_reduction_v1, verify_scoped_atomic_v1,
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
pub(crate) const EXACT_WORKGROUP_SYNC_GFX942_DATA_LAYOUT_V1: &str = concat!(
    "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-",
    "p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-",
    "v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-",
    "n32:64-S32-A5-G1-ni:7:8:9",
);
const RUSTC_RELEASE: &str = "1.96.0-nightly";
const RUSTC_COMMIT: [u8; 20] = [
    0x55, 0xe8, 0x6c, 0x99, 0x68, 0x09, 0x90, 0x2e, 0x8b, 0xba, 0xd5, 0x12, 0xcf, 0xb4, 0xd2, 0xc1,
    0x8b, 0xe4, 0x46, 0xd9,
];
const EXCHANGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKGROUP-SYNC-V1/DIRECT-WORKER-DUAL-EXCHANGE/V1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkgroupSyncProfileKindV1 {
    LdsReduction,
    ScopedAtomic,
}

impl WorkgroupSyncProfileKindV1 {
    pub const fn kernel(self) -> &'static str {
        self.spec().kernel
    }

    pub const fn descriptor(self) -> &'static str {
        self.spec().descriptor
    }

    pub(crate) const fn spec(self) -> &'static ExactWorkgroupSyncProfileV1 {
        match self {
            Self::LdsReduction => &crate::workgroup_lds_reduction_v1_profile::PROFILE,
            Self::ScopedAtomic => &crate::workgroup_scoped_atomic_v1_profile::PROFILE,
        }
    }
}

pub(crate) struct ExactWorkgroupSyncProfileV1 {
    pub(crate) kind: WorkgroupSyncProfileKindV1,
    pub(crate) kernel: &'static str,
    pub(crate) descriptor: &'static str,
    pub(crate) source_sha256: [u8; 32],
    pub(crate) namespace: [u8; 32],
    pub(crate) source_authority: [u8; 32],
    pub(crate) portable_mir: [u8; 32],
    pub(crate) fn_abi: [u8; 32],
    pub(crate) compiler_semantics: [u8; 32],
    pub(crate) trusted_terminals: [u8; 32],
    pub(crate) compiler_crate_binding: &'static str,
    pub(crate) abi_binding: &'static [u8],
    pub(crate) effect_binding: &'static [u8],
    pub(crate) resource_binding: &'static [u8],
    pub(crate) canonical_ir_binding: &'static [u8],
    pub(crate) producer_version: &'static str,
    pub(crate) llvm_body_tail: &'static str,
}

impl ExactWorkgroupSyncProfileV1 {
    fn abi_identity(&self) -> [u8; 32] {
        sha256(self.abi_binding)
    }

    fn effects_identity(&self) -> [u8; 32] {
        sha256(self.effect_binding)
    }

    fn resources_identity(&self) -> [u8; 32] {
        sha256(self.resource_binding)
    }

    fn kernel_ir_identity(&self) -> [u8; 32] {
        sha256(self.canonical_ir_binding)
    }

    fn section_prefix(&self) -> &'static str {
        match self.kind {
            WorkgroupSyncProfileKindV1::LdsReduction => ".fe2o3.wg-lds",
            WorkgroupSyncProfileKindV1::ScopedAtomic => ".fe2o3.wg-atomic",
        }
    }

    fn success_diagnostics(&self) -> Vec<String> {
        let mut diagnostics = vec![
            format!(
                "post_link.check=exports status=ok symbols=[{},{}]",
                self.kernel, self.descriptor
            ),
            "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-"
                .to_owned(),
            "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c"
                .to_owned(),
            "post_link.check=unresolved status=ok symbols=[]".to_owned(),
        ];
        match self.kind {
            WorkgroupSyncProfileKindV1::LdsReduction => diagnostics.push(
                "post_link.check=workgroup_lds_reduction_v1_profile status=ok workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=32 kernarg_size=288 kernarg_align=8 group_size=0 required_dynamic_lds=256 hidden_dynamic_lds_offset=152 hidden_dynamic_lds_value=256 private_size=0 wavefront_size=64 barriers=2 lds_bytes=256 calls=0 spills=0 dynamic_stack=false descriptor_binding=byte_exact rust_descriptor_admission=required".to_owned(),
            ),
            WorkgroupSyncProfileKindV1::ScopedAtomic => diagnostics.push(
                "post_link.check=scoped_atomic_v1_profile status=ok workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=40 kernarg_size=296 kernarg_align=8 group_size=0 private_size=0 wavefront_size=64 atomic=add ordering=relaxed scope=system address_space=global calls=0 spills=0 dynamic_stack=false descriptor_binding=byte_exact rust_descriptor_admission=required".to_owned(),
            ),
        }
        let kernarg_size = match self.kind {
            WorkgroupSyncProfileKindV1::LdsReduction => {
                LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES
            }
            WorkgroupSyncProfileKindV1::ScopedAtomic => {
                SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES
            }
        };
        diagnostics.push(format!(
            "post_link.kernel name={} symbol={} kernarg_size={kernarg_size} group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
            self.kernel, self.descriptor
        ));
        diagnostics.sort();
        diagnostics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncCompilerPinsV1 {
    profile: WorkgroupSyncProfileKindV1,
}

impl WorkgroupSyncCompilerPinsV1 {
    pub const fn exact_lds_reduction_v1() -> Self {
        Self {
            profile: WorkgroupSyncProfileKindV1::LdsReduction,
        }
    }

    pub const fn exact_scoped_atomic_v1() -> Self {
        Self {
            profile: WorkgroupSyncProfileKindV1::ScopedAtomic,
        }
    }

    pub const fn profile(self) -> WorkgroupSyncProfileKindV1 {
        self.profile
    }

    pub const fn source_sha256(self) -> &'static [u8; 32] {
        &self.profile.spec().source_sha256
    }

    pub const fn source_authority(self) -> &'static [u8; 32] {
        &self.profile.spec().source_authority
    }

    pub fn kernel_ir_identity(self) -> [u8; 32] {
        self.profile.spec().kernel_ir_identity()
    }

    pub fn descriptor_profile_identity(self) -> [u8; 32] {
        self.profile.spec().resources_identity()
    }

    pub const fn authenticates_compiler_origin(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncDirectWorkerPinsV1 {
    executable: ContentIdentityV1,
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
}

impl WorkgroupSyncDirectWorkerPinsV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Result<Self, WorkgroupSyncWorkerErrorV1> {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncDirectWorkerExpectationV1 {
    handoff_sha256: [u8; 32],
    compiler: WorkgroupSyncCompilerPinsV1,
    worker: WorkgroupSyncDirectWorkerPinsV1,
}

impl WorkgroupSyncDirectWorkerExpectationV1 {
    pub fn from_pinned_handoff(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        compiler: WorkgroupSyncCompilerPinsV1,
        worker: WorkgroupSyncDirectWorkerPinsV1,
    ) -> Result<Self, WorkgroupSyncWorkerErrorV1> {
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

    pub const fn compiler_pins(self) -> WorkgroupSyncCompilerPinsV1 {
        self.compiler
    }

    pub const fn authenticates_pin_origin(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkgroupSyncWorkerExchangeIdentityV1([u8; 32]);

impl WorkgroupSyncWorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedWorkgroupSyncWorkerExchangeV1 {
    identity: WorkgroupSyncWorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    compiler: WorkgroupSyncCompilerPinsV1,
}

impl ValidatedWorkgroupSyncWorkerExchangeV1 {
    pub const fn identity(self) -> WorkgroupSyncWorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn compiler_pins(self) -> WorkgroupSyncCompilerPinsV1 {
        self.compiler
    }

    pub const fn authenticates_compiler_origin(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct InspectedWorkgroupSyncWorkerV2HsacoV1 {
    exchange: ValidatedWorkgroupSyncWorkerExchangeV1,
    raw: InspectedRawWorkerV2HsacoV1,
}

impl InspectedWorkgroupSyncWorkerV2HsacoV1 {
    pub const fn exchange(&self) -> ValidatedWorkgroupSyncWorkerExchangeV1 {
        self.exchange
    }

    pub const fn profile(&self) -> WorkgroupSyncProfileKindV1 {
        self.exchange.compiler.profile
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
        ValidatedWorkgroupSyncWorkerExchangeV1,
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
pub enum WorkgroupSyncWorkerErrorV1 {
    CompilerHandoff(CompilerModuleHandoffErrorV2),
    WorkerProtocol(WorkerProtocolError),
    RawHsaco(WorkerV2RawHsacoInspectionError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for WorkgroupSyncWorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerHandoff(error) => {
                write!(formatter, "workgroup-sync handoff failed: {error}")
            }
            Self::WorkerProtocol(error) => {
                write!(formatter, "workgroup-sync Worker V2 failed: {error}")
            }
            Self::RawHsaco(error) => write!(formatter, "workgroup-sync raw HSACO failed: {error}"),
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "workgroup synchronization V1 profile mismatch: {field}"
                )
            }
        }
    }
}

impl Error for WorkgroupSyncWorkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerHandoff(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::RawHsaco(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

pub fn construct_inert_workgroup_sync_v1_compiler_handoff_v1(
    pins: WorkgroupSyncCompilerPinsV1,
) -> Result<CompilerModuleHandoffV2, WorkgroupSyncWorkerErrorV1> {
    let spec = pins.profile.spec();
    let body = canonical_llvm_body(spec)?;
    let descriptor = exact_descriptor_source(spec, body.as_bytes())?;
    let mut module = body.into_bytes();
    append_module_assembly_section(
        &mut module,
        COMPILER_DESCRIPTOR_SECTION_NAME_V1,
        descriptor.canonical_bytes(),
    );
    for (suffix, bytes) in [
        ("source.v1", spec.source_sha256.as_slice()),
        ("namespace.v1", spec.namespace.as_slice()),
        ("authority.v1", spec.source_authority.as_slice()),
        ("mir.v1", spec.portable_mir.as_slice()),
        ("fnabi.v1", spec.fn_abi.as_slice()),
        ("semantics.v1", spec.compiler_semantics.as_slice()),
        ("terminals.v3", spec.trusted_terminals.as_slice()),
        ("abi.v1", spec.abi_identity().as_slice()),
        ("effects.v1", spec.effects_identity().as_slice()),
        ("resources.v1", spec.resources_identity().as_slice()),
        ("kir.v1", spec.kernel_ir_identity().as_slice()),
        (
            "layout.v1",
            sha256(EXACT_WORKGROUP_SYNC_GFX942_DATA_LAYOUT_V1.as_bytes()).as_slice(),
        ),
    ] {
        append_module_assembly_section(
            &mut module,
            &format!("{}.{}", spec.section_prefix(), suffix),
            bytes,
        );
    }
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        exact_target(),
        CodeObjectVersion::V6,
        exact_envelope()?,
        exact_manifest(spec)?,
        &module,
    )
    .map_err(WorkgroupSyncWorkerErrorV1::CompilerHandoff)
}

pub fn validate_workgroup_sync_v1_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: WorkgroupSyncDirectWorkerExpectationV1,
) -> Result<ValidatedWorkgroupSyncWorkerExchangeV1, WorkgroupSyncWorkerErrorV1> {
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(WorkgroupSyncWorkerErrorV1::WorkerProtocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(WorkgroupSyncWorkerErrorV1::WorkerProtocol)?;
    let expected_handoff =
        construct_inert_workgroup_sync_v1_compiler_handoff_v1(expected.compiler)?;
    if expected_handoff.identity().sha256() != &expected.handoff_sha256 {
        return Err(profile_mismatch("reconstructed compiler handoff identity"));
    }
    for exchange in [&bootstrap, &replay] {
        validate_exchange(exchange, &expected_handoff, expected)?;
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
    Ok(ValidatedWorkgroupSyncWorkerExchangeV1 {
        identity: calculate_exchange_identity(expected.compiler.profile, &bootstrap, &replay),
        compiler_module: replay.request().compiler_module().identity(),
        linked_output: output.identity(),
        compiler: expected.compiler,
    })
}

pub fn inspect_workgroup_sync_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: WorkgroupSyncDirectWorkerExpectationV1,
) -> Result<InspectedWorkgroupSyncWorkerV2HsacoV1, WorkgroupSyncWorkerErrorV1> {
    let exchange = validate_workgroup_sync_v1_worker_exchange_v1(&source, expected)?;
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::WORKGROUP_SYNC_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1,
    )
    .map_err(WorkgroupSyncWorkerErrorV1::RawHsaco)?;
    if raw.target() != exact_target() || raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("inspected target/code-object version"));
    }
    let spec = expected.compiler.profile.spec();
    if raw.policy().observed_kernels().len() != 1
        || raw.policy().observed_kernels()[0].entry() != spec.kernel
        || raw.policy().observed_kernels()[0].descriptor() != spec.descriptor
    {
        return Err(profile_mismatch("inspected kernel/descriptor closure"));
    }
    validate_hsaco_metadata(raw.exact_bytes(), spec)?;
    Ok(InspectedWorkgroupSyncWorkerV2HsacoV1 { exchange, raw })
}

fn validate_handoff(
    handoff: &CompilerModuleHandoffV2,
    pins: WorkgroupSyncCompilerPinsV1,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let expected = construct_inert_workgroup_sync_v1_compiler_handoff_v1(pins)?;
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
    expected: WorkgroupSyncDirectWorkerExpectationV1,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let spec = expected.compiler.profile.spec();
    let request = exchange.request();
    let response = exchange.response();
    let final_symbols = [spec.kernel, spec.descriptor];
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
        || request.final_symbols() != final_symbols
        || request.worker_executable() != expected.worker.executable
        || sha256(request.worker_build_identity().as_bytes())
            != expected.worker.worker_build_identity_sha256
        || sha256(request.llvm_build_identity().as_bytes())
            != expected.worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("exact Worker V2 request"));
    }
    let diagnostics = spec.success_diagnostics();
    if !response.binds_request(request)
        || response.worker_build_identity() != request.worker_build_identity()
        || response.stage() != WorkerStageV1::Complete
        || response.device_library_provider().is_some()
        || response.diagnostics() != diagnostics
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

fn validate_hsaco_metadata(
    bytes: &[u8],
    spec: &ExactWorkgroupSyncProfileV1,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let inspected =
        fe2o3_hsaco::inspect(bytes).map_err(|_| profile_mismatch("independent COV6 metadata"))?;
    let [kernel] = inspected.kernels() else {
        return Err(profile_mismatch("metadata kernel count"));
    };
    let (explicit_kernarg_bytes, complete_kernarg_bytes) = match spec.kind {
        WorkgroupSyncProfileKindV1::LdsReduction => (
            LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES,
            LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES,
        ),
        WorkgroupSyncProfileKindV1::ScopedAtomic => (
            SCOPED_ATOMIC_V1_EXPLICIT_KERNARG_BYTES,
            SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES,
        ),
    };
    if kernel.name() != spec.kernel
        || kernel.symbol() != spec.descriptor
        || kernel.kernarg_segment_size() != u64::from(complete_kernarg_bytes)
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset() != Some(u64::from(explicit_kernarg_bytes))
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
    match spec.kind {
        WorkgroupSyncProfileKindV1::LdsReduction => validate_lds_arguments(kernel)?,
        WorkgroupSyncProfileKindV1::ScopedAtomic => validate_atomic_arguments(kernel)?,
    }
    Ok(())
}

fn validate_lds_arguments(
    kernel: &fe2o3_hsaco::InspectedKernel,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let arguments = kernel.explicit_arguments();
    if arguments.len() != 4 {
        return Err(profile_mismatch("LDS metadata explicit argument count"));
    }
    validate_global_pointer(
        &arguments[0],
        0,
        ExplicitValueType::I32,
        ArgumentAccess::ReadOnly,
    )?;
    validate_scalar(&arguments[1], 8, 8, ExplicitValueType::U64)?;
    validate_global_pointer(
        &arguments[2],
        16,
        ExplicitValueType::I32,
        ArgumentAccess::ReadWrite,
    )?;
    validate_scalar(&arguments[3], 24, 8, ExplicitValueType::U64)?;
    let dynamic: Vec<_> = kernel
        .hidden_arguments()
        .iter()
        .copied()
        .filter(|argument| argument.value_kind() == HiddenValueKind::DynamicLdsSize)
        .collect();
    if dynamic.len() != 1 || dynamic[0].offset() != 152 || dynamic[0].size() != 4 {
        return Err(profile_mismatch("COV6 hidden dynamic-LDS argument"));
    }
    Ok(())
}

fn validate_atomic_arguments(
    kernel: &fe2o3_hsaco::InspectedKernel,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let arguments = kernel.explicit_arguments();
    if arguments.len() != 5 {
        return Err(profile_mismatch("atomic metadata explicit argument count"));
    }
    validate_global_pointer(
        &arguments[0],
        0,
        ExplicitValueType::U32,
        ArgumentAccess::ReadOnly,
    )?;
    validate_scalar(&arguments[1], 8, 8, ExplicitValueType::U64)?;
    validate_global_pointer(
        &arguments[2],
        16,
        ExplicitValueType::U32,
        ArgumentAccess::ReadOnly,
    )?;
    validate_scalar(&arguments[3], 24, 8, ExplicitValueType::U64)?;
    validate_scalar(&arguments[4], 32, 8, ExplicitValueType::U64)?;
    if kernel
        .hidden_arguments()
        .iter()
        .any(|argument| argument.value_kind() == HiddenValueKind::DynamicLdsSize)
    {
        return Err(profile_mismatch("atomic hidden dynamic-LDS absence"));
    }
    Ok(())
}

fn validate_global_pointer(
    argument: &fe2o3_hsaco::ExplicitArgument,
    offset: u64,
    value_type: ExplicitValueType,
    access: ArgumentAccess,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    if argument.offset() != offset
        || argument.size() != 8
        || argument.value_kind() != ExplicitValueKind::GlobalBuffer
        || (argument.value_type().is_some() && argument.value_type() != Some(value_type))
        || argument.address_space() != Some(ArgumentAddressSpace::Global)
        || argument.access() != Some(access)
        || argument.pointee_alignment().is_some_and(|value| value != 4)
    {
        return Err(profile_mismatch("metadata global pointer ABI"));
    }
    Ok(())
}

fn validate_scalar(
    argument: &fe2o3_hsaco::ExplicitArgument,
    offset: u64,
    size: u64,
    value_type: ExplicitValueType,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    if argument.offset() != offset
        || argument.size() != size
        || argument.value_kind() != ExplicitValueKind::ByValue
        || (argument.value_type().is_some() && argument.value_type() != Some(value_type))
        || argument.address_space().is_some()
        || argument.access().is_some()
    {
        return Err(profile_mismatch("metadata scalar ABI"));
    }
    Ok(())
}

fn canonical_llvm_body(
    spec: &ExactWorkgroupSyncProfileV1,
) -> Result<String, WorkgroupSyncWorkerErrorV1> {
    match spec.kind {
        WorkgroupSyncProfileKindV1::LdsReduction => verify_lds_reduction_v1(
            &lds_reduction_v1_kernel_ir(),
            &LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6(),
        ),
        WorkgroupSyncProfileKindV1::ScopedAtomic => verify_scoped_atomic_v1(
            &scoped_atomic_v1_kernel_ir(),
            &ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6(),
        ),
    }
    .map_err(|_| profile_mismatch("canonical semantic Kernel IR"))?;
    audit_canonical_llvm(spec)?;
    Ok(canonical_llvm_body_unchecked(spec))
}

fn audit_canonical_llvm(
    spec: &ExactWorkgroupSyncProfileV1,
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    let llvm = canonical_llvm_body_unchecked(spec);
    let layout_line =
        format!("target datalayout = \"{EXACT_WORKGROUP_SYNC_GFX942_DATA_LAYOUT_V1}\"");
    let common = [
        "target triple = \"amdgcn-amd-amdhsa\"",
        layout_line.as_str(),
        "call i32 @llvm.amdgcn.workitem.id.x()",
        "call void @llvm.trap()",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "!0 = !{i32 64, i32 1, i32 1}",
    ];
    if common.iter().any(|needle| !llvm.contains(needle))
        || llvm.matches("define amdgpu_kernel").count() != 1
        || ["COMGR", "comgr", " hip", " cuda"]
            .iter()
            .any(|forbidden| llvm.contains(forbidden))
    {
        return Err(profile_mismatch("canonical upstream-LLVM envelope"));
    }
    match spec.kind {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            if !llvm.contains("external addrspace(3) global [0 x i32], align 4")
                || llvm.matches("call void @llvm.amdgcn.s.barrier()").count() != 2
                || llvm
                    .matches("fence syncscope(\"workgroup\") release")
                    .count()
                    != 2
                || llvm
                    .matches("fence syncscope(\"workgroup\") acquire")
                    .count()
                    != 2
                || llvm.matches("addrspace(3)").count() < 4
                || llvm.matches("atomicrmw").count() != 0
            {
                return Err(profile_mismatch("canonical LDS/barrier lowering"));
            }
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            if llvm.matches("atomicrmw add").count() != 1
                || !llvm.contains(", i32 %value monotonic, align 4")
                || llvm.contains("syncscope(")
                || !llvm.contains("inttoptr i64 %target.address to ptr addrspace(1)")
                || llvm.contains("addrspace(3)")
                || llvm.contains("llvm.amdgcn.s.barrier")
            {
                return Err(profile_mismatch("canonical scoped-atomic lowering"));
            }
        }
    }
    Ok(())
}

fn canonical_llvm_body_unchecked(spec: &ExactWorkgroupSyncProfileV1) -> String {
    format!(
        "target triple = \"amdgcn-amd-amdhsa\"\n\
target datalayout = \"{EXACT_WORKGROUP_SYNC_GFX942_DATA_LAYOUT_V1}\"\n\n{}",
        spec.llvm_body_tail
    )
}

fn exact_descriptor_source(
    spec: &ExactWorkgroupSyncProfileV1,
    llvm_body: &[u8],
) -> Result<CompilerDescriptorSourceV1, WorkgroupSyncWorkerErrorV1> {
    let compiler_binding = CrateBindingIdV1::from_hex(spec.compiler_crate_binding)
        .map_err(|_| profile_mismatch("compiler crate binding"))?;
    let kernel_binding = derive_kernel_binding_id_v1(
        compiler_binding,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        spec.kernel,
        spec.kernel,
    );
    let (source_types, device_layouts, arguments, capabilities, max_dynamic_lds) = match spec.kind {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            let shared =
                SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::I32));
            let output =
                SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::I32));
            let shared_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
                ScalarTypeV1::I32,
            ));
            let output_layout = DeviceLayoutRecordV1::new(
                DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::I32),
            );
            let arguments = vec![
                LogicalArgumentV1::shared_slice(0, name("values")?, &shared, &shared_layout, 0)
                    .map_err(|_| profile_mismatch("values descriptor"))?,
                LogicalArgumentV1::disjoint_slice(
                    1,
                    name("output")?,
                    &output,
                    &output_layout,
                    AccessMode::ReadWrite,
                    16,
                )
                .map_err(|_| profile_mismatch("output descriptor"))?,
            ];
            (
                vec![shared, output],
                vec![shared_layout, output_layout],
                arguments,
                vec![CapabilityV1::WorkgroupMemory, CapabilityV1::AmdWave],
                256,
            )
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            let shared =
                SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32));
            let target = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U64));
            let shared_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
                ScalarTypeV1::U32,
            ));
            let target_layout =
                DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64));
            let arguments = vec![
                LogicalArgumentV1::shared_slice(0, name("values")?, &shared, &shared_layout, 0)
                    .map_err(|_| profile_mismatch("values descriptor"))?,
                LogicalArgumentV1::shared_slice(1, name("eligible")?, &shared, &shared_layout, 16)
                    .map_err(|_| profile_mismatch("eligible descriptor"))?,
                LogicalArgumentV1::scalar(2, name("target_address")?, &target, &target_layout, 32)
                    .map_err(|_| profile_mismatch("atomic target descriptor"))?,
            ];
            (
                vec![shared, target],
                vec![shared_layout, target_layout],
                arguments,
                vec![CapabilityV1::Atomics],
                0,
            )
        }
    };
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(kernel_binding.as_bytes()),
        name(spec.kernel)?,
        name(spec.kernel)?,
        name(spec.descriptor)?,
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(spec.source_sha256),
            EvidenceDigest::from_sha256_bytes(spec.source_authority),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(spec.kernel_ir_identity()),
            EvidenceDigest::from_sha256_bytes(sha256(llvm_body)),
        ),
        capabilities,
        KernelAbiLayoutV1::new(
            match spec.kind {
                WorkgroupSyncProfileKindV1::LdsReduction => LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES,
                WorkgroupSyncProfileKindV1::ScopedAtomic => SCOPED_ATOMIC_V1_EXPLICIT_KERNARG_BYTES,
            },
            match spec.kind {
                WorkgroupSyncProfileKindV1::LdsReduction => {
                    LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES
                }
                WorkgroupSyncProfileKindV1::ScopedAtomic => {
                    SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES
                }
            },
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
            max_dynamic_lds,
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
            text(spec.producer_version)?,
        ),
        exact_target(),
        source_types,
        device_layouts,
        vec![kernel],
    )
    .map_err(|_| profile_mismatch("device descriptor table"))?;
    CompilerDescriptorSourceV1::new(table)
        .map_err(|_| profile_mismatch("compiler descriptor source"))
}

fn exact_envelope() -> Result<CompilerFfiEnvelopeV1, WorkgroupSyncWorkerErrorV1> {
    CompilerFfiEnvelopeV1::for_module_without_device_ffi(exact_target(), CodeObjectVersion::V6)
        .map_err(|_| profile_mismatch("empty compiler FFI envelope"))
}

fn exact_manifest(
    spec: &ExactWorkgroupSyncProfileV1,
) -> Result<CompilerModuleSymbolManifestV1, WorkgroupSyncWorkerErrorV1> {
    CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, spec.kernel),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            spec.descriptor,
        ),
    ])
    .map_err(|_| profile_mismatch("compiler symbol manifest"))
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(TARGET).expect("fixed workgroup-sync target is valid")
}

fn calculate_exchange_identity(
    profile: WorkgroupSyncProfileKindV1,
    bootstrap: &InertDecodedWorkerExchangeV2,
    replay: &InertDecodedWorkerExchangeV2,
) -> WorkgroupSyncWorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN);
    digest.update([match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => 1,
        WorkgroupSyncProfileKindV1::ScopedAtomic => 2,
    }]);
    for exchange in [bootstrap, replay] {
        hash_field(&mut digest, exchange.request().canonical_bytes());
        hash_field(&mut digest, exchange.response().canonical_bytes());
    }
    WorkgroupSyncWorkerExchangeIdentityV1(digest.finalize().into())
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
) -> Result<(), WorkgroupSyncWorkerErrorV1> {
    if value.is_empty()
        || value.len() > crate::MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(profile_mismatch(field));
    }
    Ok(())
}

fn name(value: &str) -> Result<ValidName, WorkgroupSyncWorkerErrorV1> {
    ValidName::new(value).map_err(|_| profile_mismatch("descriptor name"))
}

fn text(value: &str) -> Result<Text, WorkgroupSyncWorkerErrorV1> {
    Text::new(value).map_err(|_| profile_mismatch("descriptor text"))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

const fn profile_mismatch(field: &'static str) -> WorkgroupSyncWorkerErrorV1 {
    WorkgroupSyncWorkerErrorV1::ProfileMismatch(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrated_compiler_bindings_are_duplicated_exactly() {
        let integrated =
            include_str!("../../rustc-codegen-fe2o3/src/collected_workgroup_sync_v1.rs");
        for profile in [
            WorkgroupSyncProfileKindV1::LdsReduction,
            WorkgroupSyncProfileKindV1::ScopedAtomic,
        ] {
            let spec = profile.spec();
            for binding in [
                spec.compiler_crate_binding,
                std::str::from_utf8(spec.abi_binding).unwrap(),
                std::str::from_utf8(spec.effect_binding).unwrap(),
                std::str::from_utf8(spec.resource_binding).unwrap(),
                std::str::from_utf8(spec.canonical_ir_binding).unwrap(),
            ] {
                assert!(integrated.contains(binding), "missing binding {binding}");
            }
            for identity in [
                spec.portable_mir,
                spec.fn_abi,
                spec.compiler_semantics,
                spec.trusted_terminals,
            ] {
                let bytes = identity
                    .iter()
                    .map(|byte| format!("0x{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(",");
                assert!(
                    integrated
                        .split_whitespace()
                        .collect::<String>()
                        .contains(&bytes)
                );
            }
        }
    }

    #[test]
    fn every_compiler_identity_section_mutation_fails_closed() {
        for pins in [
            WorkgroupSyncCompilerPinsV1::exact_lds_reduction_v1(),
            WorkgroupSyncCompilerPinsV1::exact_scoped_atomic_v1(),
        ] {
            let handoff = construct_inert_workgroup_sync_v1_compiler_handoff_v1(pins).unwrap();
            let spec = pins.profile().spec();
            for suffix in [
                "source.v1",
                "namespace.v1",
                "authority.v1",
                "mir.v1",
                "fnabi.v1",
                "semantics.v1",
                "terminals.v3",
                "abi.v1",
                "effects.v1",
                "resources.v1",
                "kir.v1",
                "layout.v1",
            ] {
                let section = format!("{}.{}", spec.section_prefix(), suffix);
                let mut module = handoff.module_bytes().to_vec();
                let section_index = module
                    .windows(section.len())
                    .position(|window| window == section.as_bytes())
                    .unwrap_or_else(|| panic!("missing compiler identity section {section}"));
                let byte_prefix = b"module asm \".byte 0x";
                let payload_offset = module[section_index..]
                    .windows(byte_prefix.len())
                    .position(|window| window == byte_prefix)
                    .unwrap_or_else(|| panic!("missing compiler identity payload {section}"));
                let nibble = section_index + payload_offset + byte_prefix.len();
                module[nibble] = if module[nibble] == b'0' { b'1' } else { b'0' };

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
                        validate_handoff(&mutated, pins).is_err(),
                        "{} admitted mutated compiler identity section {section}",
                        spec.kernel,
                    );
                }
            }
        }
    }

    #[test]
    fn exact_handoffs_are_distinct_and_reproducible() {
        let lds = construct_inert_workgroup_sync_v1_compiler_handoff_v1(
            WorkgroupSyncCompilerPinsV1::exact_lds_reduction_v1(),
        )
        .unwrap();
        let second_lds = construct_inert_workgroup_sync_v1_compiler_handoff_v1(
            WorkgroupSyncCompilerPinsV1::exact_lds_reduction_v1(),
        )
        .unwrap();
        let atomic = construct_inert_workgroup_sync_v1_compiler_handoff_v1(
            WorkgroupSyncCompilerPinsV1::exact_scoped_atomic_v1(),
        )
        .unwrap();
        assert_eq!(lds.canonical_bytes(), second_lds.canonical_bytes());
        assert_ne!(lds.identity(), atomic.identity());
        assert!(
            lds.module_bytes()
                .windows(b"addrspace(3)".len())
                .any(|bytes| bytes == b"addrspace(3)")
        );
        assert!(
            atomic
                .module_bytes()
                .windows(9)
                .any(|bytes| bytes == b"atomicrmw")
        );
    }
}
