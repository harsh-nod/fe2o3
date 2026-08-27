//! Exact structural finalization for authenticated FlashAttention Phase A V1.
//!
//! This boundary consumes the compiler's exact closed handoff and a reproducible
//! measured Worker V2 exchange. It authenticates structural finalization only:
//! no compiler refinement, OCML mathematics, runtime, GPU, numerical, or
//! performance authority is created here.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, FLASH_ATTENTION_AUTHORITY_BYTES_V1,
    decode_flash_attention_compiler_sections_v1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion,
    ExplicitValueKind, HiddenValueKind, InspectedHsaco, KernelKind,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, DeviceDescriptorTableV1, OwnershipSemantics, PhysicalAbiComponentKind,
    ScalarTypeV1,
};
use fe2o3_kernel_ir::{
    FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES, FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL,
    FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES, FLASH_ATTENTION_V1_KERNEL_ID,
};
use sha2::{Digest, Sha256};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, FinalizationError,
    FinalizedWorkerV2HsacoIdentityV1, InertDecodedWorkerExchangeV2,
    InertFirstBuildWorkerV2EvidenceV1, PreparedFinalizedWorkerV2HsacoV1,
    WorkerCompilerFfiEnvelopeIdentityV2, WorkerDeviceLibraryProviderEvidenceV1, WorkerInputKindV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerProtocolError, WorkerRequestV2,
    WorkerResponseV2, WorkerStageV1, WorkerV2HsacoFinalizationError,
    WorkerV2RawHsacoInspectionError, WorkerV2RawLaunchContractV1,
    finalize_inspected_worker_v2_hsaco_v1, inspect_unfinalized, verify_finalized,
    worker_v2_hsaco_admission::{
        WorkerV2RawLaunchDiagnosticProfileV1, inspect_worker_v2_raw_hsaco_with_launch_v1,
    },
};

const TARGET: &str = "gfx942:xnack-";
const OCML_EXP_F32: &str = "__ocml_exp_f32";
const OCML_PROVIDER_IDENTITY: &str = "gfx942-ocml-v1";
const OCML_PROVIDER_FILE_COUNT: usize = 4;
const OCML_PROVIDER_BASENAMES: [&str; OCML_PROVIDER_FILE_COUNT] = [
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
];
const EXACT_HANDOFF_SHA256: [u8; 32] = [
    0xc4, 0x00, 0x82, 0x65, 0xf5, 0x89, 0xaa, 0x4a, 0x7f, 0x7f, 0x99, 0xa4, 0xf9, 0x49, 0xf4, 0x20,
    0x40, 0x28, 0x6a, 0xec, 0x95, 0x4c, 0x23, 0x1c, 0x3a, 0x6e, 0x8f, 0x34, 0x66, 0x3b, 0x6e, 0xc2,
];
const EXACT_AUTHORITY_SHA256: [u8; FLASH_ATTENTION_AUTHORITY_BYTES_V1] = [
    0x7e, 0x13, 0x09, 0xa9, 0xcf, 0x8d, 0x8a, 0x83, 0x26, 0xf5, 0xe5, 0xc9, 0xbe, 0x55, 0x0d, 0x97,
    0x6e, 0xbd, 0x99, 0x06, 0x4c, 0x1e, 0x8f, 0x28, 0x02, 0xb9, 0x44, 0x06, 0xca, 0x75, 0x30, 0xb9,
];
const EXACT_OCML_EXP_BOUNDARY_SHA256: [u8; 32] = [
    0xdb, 0x91, 0x96, 0x57, 0x5c, 0xcc, 0xcc, 0xd8, 0x03, 0x53, 0xf5, 0xed, 0x04, 0xbc, 0x42, 0x5b,
    0x64, 0x34, 0x4a, 0x42, 0x07, 0x09, 0x79, 0x3e, 0xe8, 0x37, 0x79, 0xad, 0xd2, 0x1e, 0x47, 0x60,
];
const SUCCESS_DIAGNOSTICS: [&str; 8] = [
    "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4",
    "post_link.check=exports status=ok symbols=[__ocml_exp_f32,flash_attention_causal_f32_b1_h1_n8_d16_v1,flash_attention_causal_f32_b1_h1_n8_d16_v1.kd]",
    "post_link.check=flash_attention_v1_profile status=ok shape=B1,H1,N8,D16 causal=true recurrence=online_strict_f32 workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=64 kernarg_size=320 kernarg_align=8 group_size=0 private_size=0 wavefront_size=64 calls=0 spills=0 dynamic_stack=false descriptor_binding=byte_exact ocml_provider=measured_structural_only rust_descriptor_admission=required",
    "post_link.check=flash_attention_v1_reproducibility status=ok llvm_build_identity=upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 input_ir_sha256=25cc163bc1ee4d5dfbe90b535a2a9913de148f9496762b147ca95e6dda09aa33 linked_bitcode_sha256=f6cfd3083e2e7f539edbffdc4696c16a5af8bc513d5872f0ab2a9b7ee36e8d50 optimized_bitcode_sha256=7fcae92f41d0edb84da73ef65b4d2f148550f6be915411be81347106a75a65bd object_sha256=359d06a95a0483b4363140c8494f54f66acf0c58d6a0fd67b4f432eca0b3dc94 raw_hsaco_sha256=2ca9d787a2bb016da8f01a895b363fdea7eeab032c45ad7ab844e6317923b16c",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=flash_attention_causal_f32_b1_h1_n8_d16_v1 symbol=flash_attention_causal_f32_b1_h1_n8_d16_v1.kd kernarg_size=320 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
];
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/FLASH-ATTENTION-V1/OPAQUE-FINALIZATION/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionV1OcmlProviderPinsV1 {
    files: [[u8; 32]; OCML_PROVIDER_FILE_COUNT],
    manifest: [u8; 32],
}

impl FlashAttentionV1OcmlProviderPinsV1 {
    pub fn new(
        files: [[u8; 32]; OCML_PROVIDER_FILE_COUNT],
        manifest: [u8; 32],
    ) -> Result<Self, FlashAttentionV1FinalizationErrorV1> {
        if files.iter().any(|digest| digest == &[0; 32]) || manifest == [0; 32] {
            return Err(profile_mismatch("independent OCML provider pins"));
        }
        Ok(Self { files, manifest })
    }

    pub const fn file_sha256(&self) -> &[[u8; 32]; OCML_PROVIDER_FILE_COUNT] {
        &self.files
    }

    pub const fn manifest_identity(&self) -> &[u8; 32] {
        &self.manifest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionV1WorkerPinsV1 {
    executable: ContentIdentityV1,
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
    provider: FlashAttentionV1OcmlProviderPinsV1,
}

impl FlashAttentionV1WorkerPinsV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
        provider: FlashAttentionV1OcmlProviderPinsV1,
    ) -> Result<Self, FlashAttentionV1FinalizationErrorV1> {
        if executable.byte_len() == 0 || executable.sha256() == &[0; 32] {
            return Err(profile_mismatch("worker executable pin"));
        }
        validate_identity_text(worker_build_identity, "worker build identity pin")?;
        validate_identity_text(llvm_build_identity, "LLVM build identity pin")?;
        Ok(Self {
            executable,
            worker_build_identity_sha256: Sha256::digest(worker_build_identity.as_bytes()).into(),
            llvm_build_identity_sha256: Sha256::digest(llvm_build_identity.as_bytes()).into(),
            provider,
        })
    }

    pub const fn executable(self) -> ContentIdentityV1 {
        self.executable
    }

    pub const fn provider(self) -> FlashAttentionV1OcmlProviderPinsV1 {
        self.provider
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashAttentionV1FinalizationExpectationV1 {
    descriptor: CompilerDescriptorSourceV1,
    worker: FlashAttentionV1WorkerPinsV1,
}

impl FlashAttentionV1FinalizationExpectationV1 {
    pub fn from_authenticated_compiler_handoff(
        handoff: &CompilerModuleHandoffV2,
        worker: FlashAttentionV1WorkerPinsV1,
    ) -> Result<Self, FlashAttentionV1FinalizationErrorV1> {
        let descriptor = validate_exact_handoff(handoff)?;
        Ok(Self { descriptor, worker })
    }

    pub const fn authenticated_compiler_handoff_sha256(&self) -> &[u8; 32] {
        &EXACT_HANDOFF_SHA256
    }

    pub const fn authenticated_frontend_authority_sha256(&self) -> &[u8; 32] {
        &EXACT_AUTHORITY_SHA256
    }

    pub const fn worker_pins(&self) -> FlashAttentionV1WorkerPinsV1 {
        self.worker
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlashAttentionV1FinalizationReceiptIdentityV1([u8; 32]);

impl FlashAttentionV1FinalizationReceiptIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Opaque single-use receipt for exact structural finalization.
///
/// The retained finalized bytes have no public accessor and this type is not
/// `Clone`. It cannot be converted into publication, load, or launch authority.
pub struct FinalizedFlashAttentionV1ReceiptV1 {
    identity: FlashAttentionV1FinalizationReceiptIdentityV1,
    frontend_authority: [u8; 32],
    provider_manifest: [u8; 32],
    finalized: PreparedFinalizedWorkerV2HsacoV1,
}

impl fmt::Debug for FinalizedFlashAttentionV1ReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalizedFlashAttentionV1ReceiptV1")
            .field("identity", &self.identity)
            .field("target", &self.finalized.target())
            .field("code_object_version", &self.finalized.code_object_version())
            .finish_non_exhaustive()
    }
}

impl FinalizedFlashAttentionV1ReceiptV1 {
    pub const fn identity(&self) -> FlashAttentionV1FinalizationReceiptIdentityV1 {
        self.identity
    }

    pub const fn finalized_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized.identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.raw_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.finalized_output_identity()
    }

    pub const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority
    }

    pub const fn measured_ocml_provider_manifest_identity(&self) -> &[u8; 32] {
        &self.provider_manifest
    }

    pub const fn target(&self) -> fe2o3_kernel_descriptor::DeviceTargetV1 {
        self.finalized.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.finalized.code_object_version()
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        true
    }

    pub const fn exact_authenticated_compiler_handoff_was_checked(&self) -> bool {
        true
    }

    pub const fn exact_machine_identity_was_checked(&self) -> bool {
        true
    }

    pub const fn measured_ocml_provider_closure_was_checked(&self) -> bool {
        true
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn proves_ocml_or_exponential_semantics(&self) -> bool {
        false
    }

    pub const fn proves_ieee_fp32_refinement(&self) -> bool {
        false
    }

    pub const fn proves_functional_flash_attention(&self) -> bool {
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

    pub const fn proves_gpu_execution_or_numerical_results(&self) -> bool {
        false
    }

    pub const fn proves_performance(&self) -> bool {
        false
    }

    pub const fn proves_no_comgr_linkage(&self) -> bool {
        false
    }

    /// Borrows the exact retained artifact only at the reviewed Flash runtime boundary.
    ///
    /// # Safety
    ///
    /// `consume` must pass the bytes directly to the exact B1/H1/N8/D16
    /// reviewed runtime adapter. It must not copy, persist, publish, return,
    /// reinterpret, or expose the bytes or derive generic load authority from
    /// them. The receipt must remain retained through load validation.
    #[doc(hidden)]
    #[allow(unsafe_code)]
    pub unsafe fn with_exact_finalized_bytes_for_reviewed_flash_runtime_v1<T>(
        &self,
        consume: impl FnOnce(&[u8], ContentIdentityV1) -> T,
    ) -> T {
        consume(
            self.finalized.exact_finalized_bytes(),
            self.finalized.finalized_output_identity(),
        )
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum FlashAttentionV1FinalizationErrorV1 {
    Handoff(CompilerModuleHandoffErrorV2),
    WorkerProtocol(WorkerProtocolError),
    RawInspection(WorkerV2RawHsacoInspectionError),
    DescriptorInspection(FinalizationError),
    CanonicalFinalization(WorkerV2HsacoFinalizationError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for FlashAttentionV1FinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "invalid FlashAttention handoff: {error}"),
            Self::WorkerProtocol(error) => {
                write!(
                    formatter,
                    "invalid FlashAttention Worker V2 exchange: {error}"
                )
            }
            Self::RawInspection(error) => {
                write!(
                    formatter,
                    "FlashAttention raw HSACO inspection failed: {error}"
                )
            }
            Self::DescriptorInspection(error) => {
                write!(
                    formatter,
                    "FlashAttention descriptor inspection failed: {error}"
                )
            }
            Self::CanonicalFinalization(error) => {
                write!(
                    formatter,
                    "FlashAttention canonical finalization failed: {error}"
                )
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "FlashAttention exact finalization profile mismatch: {field}"
                )
            }
        }
    }
}

impl Error for FlashAttentionV1FinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::RawInspection(error) => Some(error),
            Self::DescriptorInspection(error) => Some(error),
            Self::CanonicalFinalization(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

fn validate_exact_handoff(
    handoff: &CompilerModuleHandoffV2,
) -> Result<CompilerDescriptorSourceV1, FlashAttentionV1FinalizationErrorV1> {
    if handoff.identity().sha256() != &EXACT_HANDOFF_SHA256 {
        return Err(profile_mismatch("authenticated compiler handoff identity"));
    }
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr
        || handoff.target().to_string() != TARGET
        || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
    {
        return Err(profile_mismatch(
            "compiler handoff target/module/COV closure",
        ));
    }
    validate_envelope(handoff.envelope())?;
    validate_manifest(handoff.symbol_manifest())?;
    let sections = decode_flash_attention_compiler_sections_v1(handoff.module_bytes())
        .map_err(|error| profile_mismatch(error.profile_field()))?;
    if sections.authority() != &EXACT_AUTHORITY_SHA256
        || <[u8; 32]>::from(Sha256::digest(sections.authority_transcript()))
            != EXACT_AUTHORITY_SHA256
        || sections.ocml_exp_boundary() != &EXACT_OCML_EXP_BOUNDARY_SHA256
    {
        return Err(profile_mismatch(
            "authenticated compiler section commitments",
        ));
    }
    if handoff
        .envelope()
        .directional_symbols()
        .import_semantic_identities()
        .collect::<Vec<_>>()
        != [sections.ocml_exp_boundary()]
    {
        return Err(profile_mismatch("OCML exponential boundary identity"));
    }
    let descriptor = CompilerDescriptorSourceV1::decode(sections.descriptor())
        .map_err(|_| profile_mismatch("canonical compiler descriptor source"))?;
    validate_descriptor_table(descriptor.table())?;
    Ok(descriptor)
}

fn validate_envelope(
    envelope: &CompilerFfiEnvelopeV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    let directional = envelope.directional_symbols();
    if envelope.target().to_string() != TARGET
        || envelope.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
        || directional.imports().collect::<Vec<_>>() != [OCML_EXP_F32]
        || directional.exports().next().is_some()
    {
        return Err(profile_mismatch("compiler FFI envelope closure"));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    let expected = [
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            FLASH_ATTENTION_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL,
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            OCML_EXP_F32,
        ),
    ];
    if manifest.entries().collect::<Vec<_>>() != expected {
        return Err(profile_mismatch("compiler symbol manifest closure"));
    }
    Ok(())
}

fn validate_descriptor_table(
    table: &DeviceDescriptorTableV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if table.canonical_code_object_digest() != CanonicalCodeObjectDigest::from_bytes([0; 32])
        || table.code_object_version() != CodeObjectVersion::V6
        || table.device_target().to_string() != TARGET
        || table.compiler().name().as_str() != "rustc-codegen-fe2o3"
        || table.producer().name().as_str() != "rustc-codegen-fe2o3-worker-v2"
        || table.producer().version().as_str() != "typed-flash-attention-gfx942-cov6-v1"
    {
        return Err(profile_mismatch("compiler descriptor envelope"));
    }
    let [kernel] = table.kernels() else {
        return Err(profile_mismatch("compiler descriptor kernel closure"));
    };
    if kernel.logical_name().as_str() != FLASH_ATTENTION_V1_KERNEL_ID
        || kernel.entry_name().as_str() != FLASH_ATTENTION_V1_KERNEL_ID
        || kernel.descriptor_symbol().as_str() != FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL
        || kernel.capabilities() != [CapabilityV1::Subgroup, CapabilityV1::AmdWave]
    {
        return Err(profile_mismatch(
            "compiler descriptor identity/capabilities",
        ));
    }
    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES
        || abi.kernarg_segment_size() != FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES
        || abi.kernarg_segment_alignment() != 8
    {
        return Err(profile_mismatch("compiler descriptor kernarg ABI"));
    }
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(profile_mismatch("compiler descriptor workgroup"));
    };
    let max_grid = launch.max_grid();
    if launch.rank() != 1
        || [block.x(), block.y(), block.z()] != [64, 1, 1]
        || launch.max_flat_workgroup_size() != 64
        || [max_grid.x(), max_grid.y(), max_grid.z()] != [1, 1, 1]
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(profile_mismatch("compiler descriptor launch/resources"));
    }
    if kernel.arguments().len() != 4 {
        return Err(profile_mismatch("compiler descriptor argument count"));
    }
    for (index, argument) in kernel.arguments().iter().enumerate() {
        let output = index == 3;
        if argument.source_index() != u16::try_from(index).expect("bounded argument")
            || argument.name().as_str() != format!("arg{index}")
            || argument.ownership()
                != if output {
                    OwnershipSemantics::UniqueBorrow
                } else {
                    OwnershipSemantics::SharedBorrow
                }
            || argument.access()
                != if output {
                    AccessMode::ReadWrite
                } else {
                    AccessMode::ReadOnly
                }
            || argument.alias()
                != if output {
                    AliasSemantics::Exclusive
                } else {
                    AliasSemantics::SharedReadOnly
                }
        {
            return Err(profile_mismatch("compiler descriptor argument semantics"));
        }
        let offset = u32::try_from(index).expect("bounded argument") * 16;
        if argument.physical_components().collect::<Vec<_>>()
            != [
                (PhysicalAbiComponentKind::GlobalPointer, offset, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, offset + 8, 8, 8),
            ]
        {
            return Err(profile_mismatch("compiler descriptor physical ABI"));
        }
        let source_scalar = table
            .type_records()
            .iter()
            .find(|record| record.identity() == argument.source_type())
            .map(|record| record.descriptor().scalar_type());
        let layout_scalar = table
            .layout_records()
            .iter()
            .find(|record| record.identity() == argument.device_layout())
            .map(|record| record.descriptor().scalar_type());
        if source_scalar != Some(ScalarTypeV1::F32) || layout_scalar != Some(ScalarTypeV1::F32) {
            return Err(profile_mismatch("compiler descriptor type provenance"));
        }
    }
    Ok(())
}

/// Consumes one exact authenticated first-build exchange and returns a sealed receipt.
pub fn finalize_flash_attention_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: FlashAttentionV1FinalizationExpectationV1,
) -> Result<FinalizedFlashAttentionV1ReceiptV1, FlashAttentionV1FinalizationErrorV1> {
    validate_worker_exchange(&source, &expected)?;
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::FLASH_ATTENTION_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1,
    )
    .map_err(FlashAttentionV1FinalizationErrorV1::RawInspection)?;
    if raw.target().to_string() != TARGET
        || raw.code_object_version() != CodeObjectVersion::V6
        || raw.canonical_descriptor_section()
            != CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
    {
        return Err(profile_mismatch("raw target/COV/descriptor closure"));
    }
    let unfinalized = inspect_unfinalized(raw.exact_bytes())
        .map_err(FlashAttentionV1FinalizationErrorV1::DescriptorInspection)?;
    validate_descriptor_equivalence(unfinalized.descriptor_table(), expected.descriptor.table())?;
    validate_exact_hsaco_metadata(unfinalized.hsaco())?;

    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .map_err(FlashAttentionV1FinalizationErrorV1::CanonicalFinalization)?;
    let verified = verify_finalized(finalized.exact_finalized_bytes())
        .map_err(FlashAttentionV1FinalizationErrorV1::DescriptorInspection)?;
    validate_descriptor_equivalence(verified.descriptor_table(), expected.descriptor.table())?;
    validate_exact_hsaco_metadata(verified.hsaco())?;

    let identity = calculate_receipt_identity(&finalized, expected.worker.provider.manifest);
    Ok(FinalizedFlashAttentionV1ReceiptV1 {
        identity,
        frontend_authority: EXACT_AUTHORITY_SHA256,
        provider_manifest: expected.worker.provider.manifest,
        finalized,
    })
}

fn validate_worker_exchange(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: &FlashAttentionV1FinalizationExpectationV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if source.handoff_identity().as_bytes() != &EXACT_HANDOFF_SHA256 {
        return Err(profile_mismatch("consumed compiler handoff identity"));
    }
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(FlashAttentionV1FinalizationErrorV1::WorkerProtocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(FlashAttentionV1FinalizationErrorV1::WorkerProtocol)?;
    validate_matching_requests(bootstrap.request(), replay.request())?;
    validate_request(
        bootstrap.request(),
        source.compiler_envelope(),
        source.symbol_manifest(),
        expected,
    )?;
    validate_request(
        replay.request(),
        source.compiler_envelope(),
        source.symbol_manifest(),
        expected,
    )?;
    validate_response(bootstrap.request(), bootstrap.response(), expected, false)?;
    validate_response(replay.request(), replay.response(), expected, true)?;
    validate_matching_responses(bootstrap.response(), replay.response())?;

    if source.plan().target() != replay.request().target()
        || source.worker_measurement().executable() != replay.request().worker_executable()
        || source.worker_measurement().worker_build_identity()
            != replay.request().worker_build_identity()
        || source.worker_measurement().llvm_build_identity()
            != replay.request().llvm_build_identity()
        || source.output_identity()
            != replay
                .response()
                .output()
                .ok_or_else(|| profile_mismatch("completed replay output"))?
                .identity()
        || !source.output_identity().matches(source.output_bytes())
    {
        return Err(profile_mismatch("retained Worker V2 evidence closure"));
    }
    Ok(())
}

fn validate_matching_requests(
    bootstrap: &WorkerRequestV2,
    replay: &WorkerRequestV2,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if bootstrap.request_id() == replay.request_id()
        || bootstrap.identity() == replay.identity()
        || bootstrap.llvm_build_identity() != replay.llvm_build_identity()
        || bootstrap.worker_build_identity() != replay.worker_build_identity()
        || bootstrap.worker_executable() != replay.worker_executable()
        || bootstrap.target() != replay.target()
        || bootstrap.code_object_version() != replay.code_object_version()
        || bootstrap.options() != replay.options()
        || bootstrap.compiler_envelope_identity() != replay.compiler_envelope_identity()
        || bootstrap.compiler_module() != replay.compiler_module()
        || bootstrap.external_providers() != replay.external_providers()
        || bootstrap.import_symbols() != replay.import_symbols()
        || bootstrap.export_symbols() != replay.export_symbols()
        || bootstrap.final_symbols() != replay.final_symbols()
        || bootstrap.output_constraints().max_bytes() < replay.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("bootstrap/replay request closure"));
    }
    Ok(())
}

fn validate_matching_responses(
    bootstrap: &WorkerResponseV2,
    replay: &WorkerResponseV2,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    let bootstrap_output = bootstrap
        .output()
        .ok_or_else(|| profile_mismatch("bootstrap output"))?;
    let replay_output = replay
        .output()
        .ok_or_else(|| profile_mismatch("replay output"))?;
    if bootstrap.response_identity() == replay.response_identity()
        || bootstrap.compiler_envelope_identity() != replay.compiler_envelope_identity()
        || bootstrap.worker_build_identity() != replay.worker_build_identity()
        || bootstrap.stage() != replay.stage()
        || bootstrap.diagnostics() != replay.diagnostics()
        || bootstrap.device_library_provider() != replay.device_library_provider()
        || bootstrap_output.identity() != replay_output.identity()
        || bootstrap_output.bytes() != replay_output.bytes()
    {
        return Err(profile_mismatch("bootstrap/replay response closure"));
    }
    Ok(())
}

fn validate_request(
    request: &WorkerRequestV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: &FlashAttentionV1FinalizationExpectationV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    let worker_build_identity_sha256: [u8; 32] =
        Sha256::digest(request.worker_build_identity().as_bytes()).into();
    let llvm_build_identity_sha256: [u8; 32] =
        Sha256::digest(request.llvm_build_identity().as_bytes()).into();
    if request.target().to_string() != TARGET
        || request.code_object_version() != CodeObjectVersion::V6
        || request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
        || request.worker_executable() != expected.worker.executable
        || worker_build_identity_sha256 != expected.worker.worker_build_identity_sha256
        || llvm_build_identity_sha256 != expected.worker.llvm_build_identity_sha256
        || request.compiler_envelope_identity()
            != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(envelope.identity())
        || request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr
        || !request.external_providers().is_empty()
        || request.import_symbols() != [OCML_EXP_F32]
        || !request.export_symbols().is_empty()
        || request.final_symbols()
            != [
                OCML_EXP_F32,
                FLASH_ATTENTION_V1_KERNEL_ID,
                FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL,
            ]
    {
        return Err(profile_mismatch("exact Worker V2 request closure"));
    }
    validate_envelope(envelope)?;
    validate_manifest(manifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        envelope.target(),
        envelope.code_object_version(),
        envelope.clone(),
        manifest.clone(),
        request.compiler_module().bytes(),
    )
    .map_err(FlashAttentionV1FinalizationErrorV1::Handoff)?;
    let descriptor = validate_exact_handoff(&handoff)?;
    if descriptor != expected.descriptor {
        return Err(profile_mismatch("compiler descriptor source identity"));
    }
    Ok(())
}

fn validate_response(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
    expected: &FlashAttentionV1FinalizationExpectationV1,
    require_exact_output_bound: bool,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if !response.binds_request(request)
        || response.worker_build_identity() != request.worker_build_identity()
        || response.stage() != WorkerStageV1::Complete
        || response.response_identity().is_none()
        || response.diagnostics() != SUCCESS_DIAGNOSTICS
    {
        return Err(profile_mismatch("authenticated Worker V2 response closure"));
    }
    let provider = response
        .device_library_provider()
        .ok_or_else(|| profile_mismatch("structured OCML provider evidence"))?;
    validate_provider(provider, expected.worker.provider)?;
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || output.identity().byte_len() > request.output_constraints().max_bytes()
        || (require_exact_output_bound
            && output.identity().byte_len() != request.output_constraints().max_bytes())
    {
        return Err(profile_mismatch("Worker V2 output binding"));
    }
    Ok(())
}

fn validate_provider(
    actual: &WorkerDeviceLibraryProviderEvidenceV1,
    expected: FlashAttentionV1OcmlProviderPinsV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if actual.provider_identity() != OCML_PROVIDER_IDENTITY
        || actual.target().to_string() != TARGET
        || actual.code_object_version() != CodeObjectVersion::V6
        || actual.import_symbols() != [OCML_EXP_F32]
        || actual.manifest_identity() != expected.manifest_identity()
        || actual.files().len() != OCML_PROVIDER_FILE_COUNT
    {
        return Err(profile_mismatch("measured OCML provider closure"));
    }
    for (index, file) in actual.files().iter().enumerate() {
        if file.basename() != OCML_PROVIDER_BASENAMES[index]
            || file.sha256() != &expected.files[index]
        {
            return Err(profile_mismatch("ordered OCML provider file identity"));
        }
    }
    Ok(())
}

fn validate_descriptor_equivalence(
    actual: &DeviceDescriptorTableV1,
    expected: &DeviceDescriptorTableV1,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if actual.code_object_version() != expected.code_object_version()
        || actual.compiler() != expected.compiler()
        || actual.producer() != expected.producer()
        || actual.device_target() != expected.device_target()
        || actual.type_records() != expected.type_records()
        || actual.layout_records() != expected.layout_records()
        || actual.kernels() != expected.kernels()
    {
        return Err(profile_mismatch("linked compiler descriptor equivalence"));
    }
    Ok(())
}

fn validate_exact_hsaco_metadata(
    hsaco: &InspectedHsaco,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if hsaco.code_object_version() != InspectedCodeObjectVersion::V6
        || hsaco.target().to_string() != TARGET
    {
        return Err(profile_mismatch("HSACO target/COV"));
    }
    let [kernel] = hsaco.kernels() else {
        return Err(profile_mismatch("HSACO kernel closure"));
    };
    if kernel.name() != FLASH_ATTENTION_V1_KERNEL_ID
        || kernel.symbol() != FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL
        || kernel.kind() != KernelKind::Normal
        || kernel.required_workgroup_size() != Some([64, 1, 1])
        || kernel.max_flat_workgroup_size() != 64
        || kernel.wavefront_size() != 64
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.sgpr_spill_count() != Some(0)
        || kernel.vgpr_spill_count() != Some(0)
        || kernel.uses_dynamic_stack()
        || kernel.uniform_work_group_size()
        || kernel.workgroup_processor_mode().is_some()
        || kernel.cluster_dims().is_some()
        || kernel.max_workgroups() != [None; 3]
        || kernel.device_enqueue_symbol().is_some()
        || kernel.kernarg_segment_size()
            != u64::from(FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES)
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset()
            != Some(u64::from(FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES))
        || kernel.implicit_argument_size()
            != u64::from(
                FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES
                    - FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES,
            )
    {
        return Err(profile_mismatch("exact HSACO kernel metadata/resources"));
    }
    validate_explicit_arguments(kernel.explicit_arguments())?;
    validate_hidden_arguments(kernel.hidden_arguments())?;
    Ok(())
}

fn validate_explicit_arguments(
    arguments: &[fe2o3_hsaco::ExplicitArgument],
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if arguments.len() != 8 {
        return Err(profile_mismatch("HSACO explicit argument count"));
    }
    let names = [
        "q.data",
        "q.len",
        "k.data",
        "k.len",
        "v.data",
        "v.len",
        "output.data",
        "output.len",
    ];
    for (index, argument) in arguments.iter().enumerate() {
        if argument.name() != Some(names[index])
            || argument.offset() != u64::try_from(index).expect("bounded argument") * 8
            || argument.size() != 8
            || argument.alignment().is_some()
            || argument.is_volatile().is_some()
            || argument.is_pipe().is_some()
        {
            return Err(profile_mismatch("HSACO explicit argument layout"));
        }
        if index % 2 == 1 {
            if argument.type_name() != Some("ulong")
                || argument.value_kind() != ExplicitValueKind::ByValue
                || argument.value_type().is_some()
                || argument.address_space().is_some()
                || argument.access().is_some()
                || argument.actual_access().is_some()
                || argument.pointee_alignment().is_some()
                || argument.is_const().is_some()
                || argument.is_restrict().is_some()
            {
                return Err(profile_mismatch("HSACO slice-length ABI"));
            }
            continue;
        }
        let output = index == 6;
        if argument.type_name() != Some("float*")
            || argument.value_kind() != ExplicitValueKind::GlobalBuffer
            || argument.value_type().is_some()
            || argument.address_space() != Some(ArgumentAddressSpace::Global)
            || argument.access()
                != Some(if output {
                    ArgumentAccess::ReadWrite
                } else {
                    ArgumentAccess::ReadOnly
                })
            || argument.actual_access()
                != if output {
                    Some(ArgumentAccess::WriteOnly)
                } else {
                    None
                }
            || argument.pointee_alignment().is_some()
            || argument.is_const() != if output { None } else { Some(true) }
            || argument.is_restrict() != if output { Some(true) } else { None }
        {
            return Err(profile_mismatch("HSACO pointer access/alias contract"));
        }
    }
    Ok(())
}

fn validate_hidden_arguments(
    arguments: &[fe2o3_hsaco::HiddenArgument],
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    const REQUIRED: [(u64, u64, HiddenValueKind); 13] = [
        (0, 4, HiddenValueKind::BlockCountX),
        (4, 4, HiddenValueKind::BlockCountY),
        (8, 4, HiddenValueKind::BlockCountZ),
        (12, 2, HiddenValueKind::GroupSizeX),
        (14, 2, HiddenValueKind::GroupSizeY),
        (16, 2, HiddenValueKind::GroupSizeZ),
        (18, 2, HiddenValueKind::RemainderX),
        (20, 2, HiddenValueKind::RemainderY),
        (22, 2, HiddenValueKind::RemainderZ),
        (40, 8, HiddenValueKind::GlobalOffsetX),
        (48, 8, HiddenValueKind::GlobalOffsetY),
        (56, 8, HiddenValueKind::GlobalOffsetZ),
        (64, 2, HiddenValueKind::GridDimensions),
    ];
    if arguments.len() != REQUIRED.len() {
        return Err(profile_mismatch("COV6 hidden argument closure"));
    }
    for (argument, (offset, size, kind)) in arguments.iter().copied().zip(REQUIRED) {
        if argument.offset() != u64::from(FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES) + offset
            || argument.size() != size
            || argument.value_kind() != kind
        {
            return Err(profile_mismatch("COV6 hidden argument ABI"));
        }
    }
    Ok(())
}

fn calculate_receipt_identity(
    finalized: &PreparedFinalizedWorkerV2HsacoV1,
    provider_manifest: [u8; 32],
) -> FlashAttentionV1FinalizationReceiptIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(RECEIPT_IDENTITY_DOMAIN_V1);
    digest.update(EXACT_HANDOFF_SHA256);
    digest.update(EXACT_AUTHORITY_SHA256);
    digest.update(provider_manifest);
    digest.update(finalized.identity().as_bytes());
    digest.update(finalized.finalized_output_identity().sha256());
    FlashAttentionV1FinalizationReceiptIdentityV1(digest.finalize().into())
}

fn validate_identity_text(
    value: &str,
    field: &'static str,
) -> Result<(), FlashAttentionV1FinalizationErrorV1> {
    if value.is_empty()
        || value.len() > crate::MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(profile_mismatch(field));
    }
    Ok(())
}

const fn profile_mismatch(field: &'static str) -> FlashAttentionV1FinalizationErrorV1 {
    FlashAttentionV1FinalizationErrorV1::ProfileMismatch(field)
}
