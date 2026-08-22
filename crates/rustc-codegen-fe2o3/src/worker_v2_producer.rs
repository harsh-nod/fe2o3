//! Preparation and attempt-scoped publication of inert Worker V2 compiler modules.

use crate::collected_flash_attention_v1::FlashAttentionFinalizationInputsV1;
use crate::collected_moe_top2_v1::AuthenticatedMoeTop2V1;
use crate::collected_row_softmax_v1::AuthenticatedRowSoftmaxModuleV1;
use crate::collected_scalar_gemm_v1::AuthenticatedScalarGemmModuleV1;
use crate::collected_tiled_gemm_lds_slice1_v1::AuthenticatedLdsSlice1ModuleV1;
use crate::collected_tiled_gemm_v1::AuthenticatedTiledGemmModuleV1;
use crate::compiler_descriptor::{
    CompilerDescriptorError, TypedDescriptorRootV1, construct_compiler_descriptor_source_v1,
    construct_flash_attention_v1_compiler_descriptor_source_v1,
    construct_production_v1_compiler_descriptor_source_v1,
    validate_tiled_gemm_lds_slice1_compiler_module_evidence_v1,
};
use crate::kernel_ir_codegen::{
    CompilerModuleConstructionError, InertCompilerModuleTextV1, bind_compiler_descriptor_source_v1,
    bind_source_debug_metadata_v1, construct_inert_compiler_module_text_for_target_v1,
    retain_production_gfx942_compiler_module_text_v1,
};
use fe2o3_amd_target::{CapabilityDerivationError, WavefrontWidth};
use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV1 as HandoffPublicationErrorV1,
    CompilerModuleHandoffErrorV2 as HandoffPublicationErrorV2, CompilerModuleHandoffReceiptV1,
    CompilerModuleHandoffReceiptV2, ProducerIdentity, publish_compiler_module_handoff_v1,
    publish_compiler_module_handoff_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1, CompilerModuleHandoffErrorV2,
    CompilerModuleHandoffIdentityV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestErrorV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1, decode_row_softmax_compiler_sections_v1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    Module, SCALAR_GEMM_V1_KERNEL_ID, TargetCapability, WaveWidth, WorkgroupSize,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::Path;

const G1_WORKGROUP_X: u32 = 256;
const SCALAR_GEMM_V1_DESCRIPTOR: &str = "scalar_gemm_v1.kd";
const TILED_GEMM_V1_DESCRIPTOR: &str = "tiled_gemm_v1.kd";
const TILED_GEMM_LDS_SLICE1_DESCRIPTOR: &str = "tiled_gemm_lds_v1.kd";
const ROW_SOFTMAX_V1_DESCRIPTOR: &str = "row_softmax_v1.kd";
pub(crate) const ROW_SOFTMAX_OCML_EXP_SYMBOL_V1: &str = "__ocml_exp_f32";
const ROW_SOFTMAX_OCML_EXP_ABI_V1: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";
const ROW_SOFTMAX_OCML_EXP_EFFECTS_V1: &str = "none";
const FLASH_ATTENTION_V1_DESCRIPTOR: &str = fe2o3_kernel_ir::FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL;
const MOE_TOP2_V1_DESCRIPTOR: &str = fe2o3_kernel_ir::MOE_TOP2_V1_DESCRIPTOR_SYMBOL;
pub(crate) const FLASH_ATTENTION_OCML_EXP_SYMBOL_V1: &str = "__ocml_exp_f32";
const FLASH_ATTENTION_OCML_EXP_ABI_V1: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";
const FLASH_ATTENTION_OCML_EXP_EFFECTS_V1: &str = "none";
const FLASH_ATTENTION_OCML_BOUNDARY_V1: &[u8] = b"fe2o3.flash-attention.ocml-exp-boundary.v1;provider-identity-and-closed-link-structure-only;no-exponential-law,approximation-error,IEEE-fp32,or-source-refinement-proof";

/// Inert Worker V2 compiler-module handoff retained with the frontend authority
/// that selected its canonical scalar GEMM Kernel IR.
///
/// This value grants no publication, worker, link, artifact, load, or launch
/// authority. Its fields are private and it is consumed by later stages.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedScalarGemmV1WorkerHandoffV1 {
    frontend_authority_commitment: [u8; 32],
    handoff: CompilerModuleHandoffV2,
}

/// Inert Worker V2 handoff prepared by the single production compiler
/// pipeline. Its LLVM bytes already passed semantic, formal-memory, and exact
/// gfx942 lowering stages; this value grants no publication authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedProductionV1WorkerHandoffV1 {
    llvm_ir_sha256: [u8; 32],
    handoff: CompilerModuleHandoffV2,
}

/// Move-only production worker preparation retaining the exact compiler
/// descriptor source alongside the module handoff that embeds it.
pub(crate) struct PreparedProductionLineageWorkerHandoffV3 {
    worker_handoff: PreparedProductionV1WorkerHandoffV1,
    compiler_descriptor_source: CompilerDescriptorSourceV1,
}

impl PreparedProductionLineageWorkerHandoffV3 {
    pub(crate) fn into_worker_handoff(self) -> PreparedProductionV1WorkerHandoffV1 {
        self.worker_handoff
    }

    pub(crate) fn into_validated_parts(
        self,
    ) -> Result<(CompilerModuleHandoffV2, CompilerDescriptorSourceV1), WorkerV2ProducerError> {
        let Self {
            worker_handoff,
            compiler_descriptor_source,
        } = self;
        let handoff = validate_prepared_production_v1_worker_handoff(worker_handoff)?;
        Ok((handoff, compiler_descriptor_source))
    }
}

impl PreparedScalarGemmV1WorkerHandoffV1 {
    pub(crate) const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

/// Typed, inert Worker V2 handoff for the source-authenticated canonical tiled
/// GEMM. It is distinct from the 32/288-byte fragment-level frontend probe.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedTiledGemmV1WorkerHandoffV1 {
    frontend_authority_commitment: [u8; 32],
    handoff: CompilerModuleHandoffV2,
}

impl PreparedTiledGemmV1WorkerHandoffV1 {
    pub(crate) const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

/// Protected, inert Worker V2 handoff for the exact attributed LDS Slice 1
/// source, canonical Kernel IR, descriptor, and compiler-derived resources.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedTiledGemmLdsSlice1WorkerHandoffV1 {
    source_authority_commitment: [u8; 32],
    canonical_ir_identity: [u8; 32],
    descriptor_source_identity: [u8; 32],
    descriptor_source_bytes: Vec<u8>,
    resource_transcript: Vec<u8>,
    expected_handoff_identity: CompilerModuleHandoffIdentityV2,
    handoff: CompilerModuleHandoffV2,
}

impl PreparedTiledGemmLdsSlice1WorkerHandoffV1 {
    pub(crate) const fn source_authority_commitment(&self) -> &[u8; 32] {
        &self.source_authority_commitment
    }

    pub(crate) const fn canonical_ir_identity(&self) -> &[u8; 32] {
        &self.canonical_ir_identity
    }

    pub(crate) const fn descriptor_source_identity(&self) -> &[u8; 32] {
        &self.descriptor_source_identity
    }

    pub(crate) fn resource_transcript(&self) -> &[u8] {
        &self.resource_transcript
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

/// Inert Worker V2 handoff for the exact row-softmax LLVM and OCML-import
/// closure. This value grants no worker, provider, link, artifact, or runtime
/// authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedRowSoftmaxV1WorkerHandoffV1 {
    authority_transcript: Vec<u8>,
    frontend_authority_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
    handoff: CompilerModuleHandoffV2,
}

/// Inert, exact Flash compiler handoff. Construction consumes the authenticated
/// source/KIR value and grants no worker, link, artifact, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedFlashAttentionV1WorkerHandoffV1 {
    authority_transcript: Vec<u8>,
    frontend_authority_commitment: [u8; 32],
    ocml_boundary_commitment: [u8; 32],
    descriptor_source_bytes: Vec<u8>,
    expected_handoff_identity: CompilerModuleHandoffIdentityV2,
    handoff: CompilerModuleHandoffV2,
}

impl PreparedFlashAttentionV1WorkerHandoffV1 {
    pub(crate) const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub(crate) fn ocml_boundary_hex(&self) -> String {
        crate::encode_hex(&self.ocml_boundary_commitment)
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

/// Linear inert handoff derived only from the consumed exact MoE receipt.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedMoeTop2V1WorkerHandoffV1 {
    source_authority_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    descriptor_profile_identity: [u8; 32],
    expected_handoff_identity: CompilerModuleHandoffIdentityV2,
    handoff: CompilerModuleHandoffV2,
}

impl PreparedMoeTop2V1WorkerHandoffV1 {
    pub(crate) const fn source_authority_identity(&self) -> &[u8; 32] {
        &self.source_authority_identity
    }

    pub(crate) const fn canonical_ir_identity(&self) -> &[u8; 32] {
        &self.canonical_ir_identity
    }

    pub(crate) const fn descriptor_profile_identity(&self) -> &[u8; 32] {
        &self.descriptor_profile_identity
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

impl PreparedRowSoftmaxV1WorkerHandoffV1 {
    #[cfg(test)]
    pub(crate) fn authority_transcript(&self) -> &[u8] {
        &self.authority_transcript
    }

    pub(crate) const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub(crate) const fn exponential_boundary_commitment(&self) -> &[u8; 32] {
        &self.exponential_boundary_commitment
    }

    pub(crate) const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }
}

pub(crate) fn publish_prepared_production_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedProductionV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let handoff = validate_prepared_production_v1_worker_handoff(prepared)?;
    publish_compiler_module_handoff_v1(output_dir, producer, attempt, handoff.canonical_bytes())
        .map_err(WorkerV2ProducerError::Publication)
}

pub(crate) fn publish_prepared_production_v1_worker_handoff_v2(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    compiler_closure: CompilerClosureV2,
    prepared: PreparedProductionV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV2, WorkerV2ProducerError> {
    let handoff = validate_prepared_production_v1_worker_handoff(prepared)?;
    publish_compiler_module_handoff_v2(
        output_dir,
        producer,
        attempt,
        compiler_closure,
        handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::ProtectedPublication)
}

fn validate_prepared_production_v1_worker_handoff(
    prepared: PreparedProductionV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffV2, WorkerV2ProducerError> {
    if Sha256::digest(prepared.handoff.module_bytes()).as_slice() != prepared.llvm_ir_sha256 {
        return Err(WorkerV2ProducerError::MissingProductionBindings);
    }
    Ok(prepared.handoff)
}

/// Consumes the exact scalar frontend handoff into the managed attempt's
/// existing compiler-module publication protocol.
///
/// The frontend commitment remains embedded in the compiler module itself;
/// the returned receipt is coordination evidence and grants no worker, link,
/// publication, load, or launch authority.
pub(crate) fn publish_prepared_scalar_gemm_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedScalarGemmV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let PreparedScalarGemmV1WorkerHandoffV1 {
        frontend_authority_commitment,
        handoff,
    } = prepared;
    let authority_hex = crate::encode_hex(&frontend_authority_commitment);
    let module = std::str::from_utf8(handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingScalarFrontendAuthority)?;
    if !module.contains(".fe2o3.scalar-auth.v1")
        || !frontend_authority_commitment
            .chunks(16)
            .all(|chunk| module.contains(&module_asm_byte_line(chunk)))
    {
        return Err(WorkerV2ProducerError::MissingScalarFrontendAuthority);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published scalar GEMM Worker V2 handoff bound to frontend authority {authority_hex}"
    );
    Ok(receipt)
}

pub(crate) fn publish_prepared_tiled_gemm_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedTiledGemmV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let PreparedTiledGemmV1WorkerHandoffV1 {
        frontend_authority_commitment,
        handoff,
    } = prepared;
    let authority_hex = crate::encode_hex(&frontend_authority_commitment);
    let module = std::str::from_utf8(handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingTiledFrontendAuthority)?;
    if !module.contains(".fe2o3.tiled-auth.v1")
        || !frontend_authority_commitment
            .chunks(16)
            .all(|chunk| module.contains(&module_asm_byte_line(chunk)))
    {
        return Err(WorkerV2ProducerError::MissingTiledFrontendAuthority);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published tiled GEMM Worker V2 handoff bound to frontend authority {authority_hex}"
    );
    Ok(receipt)
}

pub(crate) fn publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedTiledGemmLdsSlice1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let module = std::str::from_utf8(prepared.handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings)?;
    let [descriptor, authority, resources] = decode_tiled_gemm_lds_slice1_sections_v1(module)
        .ok_or(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings)?;
    let descriptor_identity: [u8; 32] = Sha256::digest(&descriptor).into();
    let exact_sections = descriptor == prepared.descriptor_source_bytes
        && descriptor_identity == prepared.descriptor_source_identity
        && authority == prepared.source_authority_commitment
        && resources == prepared.resource_transcript
        && transcript_contains_field(&resources, &prepared.source_authority_commitment)
        && transcript_contains_field(&resources, &prepared.canonical_ir_identity)
        && transcript_contains_field(&resources, &prepared.descriptor_source_identity);
    if !exact_sections
        || prepared.handoff.identity() != prepared.expected_handoff_identity
        || prepared.handoff.target().to_string() != AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        || prepared.handoff.code_object_version() != CodeObjectVersion::V6
    {
        return Err(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        prepared.handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published attributed LDS Slice 1 Worker V2 handoff bound to source authority {}, canonical Kernel IR {}, and compiler descriptor {}",
        crate::encode_hex(&prepared.source_authority_commitment),
        crate::encode_hex(&prepared.canonical_ir_identity),
        crate::encode_hex(&prepared.descriptor_source_identity),
    );
    Ok(receipt)
}

fn decode_tiled_gemm_lds_slice1_sections_v1(module: &str) -> Option<[Vec<u8>; 3]> {
    decode_exact_compiler_sections_v1(
        module,
        &[
            fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            crate::kernel_ir_codegen::TILED_GEMM_LDS_SLICE1_AUTHORITY_SECTION_V1,
            crate::kernel_ir_codegen::TILED_GEMM_LDS_SLICE1_RESOURCE_SECTION_V1,
        ],
    )
}

fn decode_exact_compiler_sections_v1<const N: usize>(
    module: &str,
    expected: &[&str; N],
) -> Option<[Vec<u8>; N]> {
    let lines = module.lines().collect::<Vec<_>>();
    let declarations = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            canonical_module_assembly_section_name(line).map(|name| (index, name))
        })
        .collect::<Vec<_>>();
    if declarations.len() != expected.len()
        || !declarations
            .iter()
            .zip(expected.iter().copied())
            .all(|((_, actual), expected)| *actual == expected)
    {
        return None;
    }
    let mut decoded = std::array::from_fn(|_| Vec::new());
    for (section, (start, _)) in declarations.iter().enumerate() {
        if lines.get(start + 1) != Some(&"module asm \".balign 8\"") {
            return None;
        }
        let end = declarations
            .get(section + 1)
            .map_or(lines.len(), |(index, _)| *index);
        let byte_lines = lines.get(start + 2..end)?;
        if byte_lines.is_empty() {
            return None;
        }
        for line in byte_lines {
            let values = line
                .strip_prefix("module asm \".byte ")?
                .strip_suffix('"')?;
            let chunks = values.split(", ").collect::<Vec<_>>();
            if chunks.is_empty() || chunks.len() > 16 {
                return None;
            }
            for value in chunks {
                let digits = value.strip_prefix("0x")?;
                if digits.len() != 2
                    || !digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return None;
                }
                decoded[section].push(u8::from_str_radix(digits, 16).ok()?);
            }
        }
    }
    Some(decoded)
}

fn canonical_module_assembly_section_name(line: &str) -> Option<&str> {
    let suffix = line.strip_prefix("module asm \".section ")?;
    let name = suffix.strip_suffix(",\\22\\22,@progbits\"")?;
    (!name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
    .then_some(name)
}

fn transcript_contains_field(transcript: &[u8], field: &[u8]) -> bool {
    let mut framed = Vec::with_capacity(8 + field.len());
    framed.extend_from_slice(&(field.len() as u64).to_le_bytes());
    framed.extend_from_slice(field);
    transcript
        .windows(framed.len())
        .filter(|candidate| *candidate == framed)
        .count()
        == 1
}

pub(crate) fn publish_prepared_row_softmax_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedRowSoftmaxV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let validated = validate_prepared_row_softmax_v1_worker_handoff(prepared)?;
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        validated.handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    validated.report_publication();
    Ok(receipt)
}

pub(crate) fn publish_prepared_row_softmax_v1_worker_handoff_v2(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    compiler_closure: CompilerClosureV2,
    prepared: PreparedRowSoftmaxV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV2, WorkerV2ProducerError> {
    let validated = validate_prepared_row_softmax_v1_worker_handoff(prepared)?;
    let receipt = publish_compiler_module_handoff_v2(
        output_dir,
        producer,
        attempt,
        compiler_closure,
        validated.handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::ProtectedPublication)?;
    validated.report_publication();
    Ok(receipt)
}

struct ValidatedRowSoftmaxWorkerHandoffV1 {
    frontend_authority_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
    handoff: CompilerModuleHandoffV2,
}

impl ValidatedRowSoftmaxWorkerHandoffV1 {
    fn report_publication(&self) {
        eprintln!(
            "[rustc-codegen-fe2o3] published row-softmax Worker V2 handoff bound to frontend authority {} and exponential boundary {}",
            crate::encode_hex(&self.frontend_authority_commitment),
            crate::encode_hex(&self.exponential_boundary_commitment),
        );
    }
}

fn validate_prepared_row_softmax_v1_worker_handoff(
    prepared: PreparedRowSoftmaxV1WorkerHandoffV1,
) -> Result<ValidatedRowSoftmaxWorkerHandoffV1, WorkerV2ProducerError> {
    let sections = decode_row_softmax_compiler_sections_v1(prepared.handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingRowSoftmaxBindings)?;
    if sections.authority_transcript() != prepared.authority_transcript
        || sections.authority() != &prepared.frontend_authority_commitment
        || sections.exponential_boundary() != &prepared.exponential_boundary_commitment
    {
        return Err(WorkerV2ProducerError::MissingRowSoftmaxBindings);
    }
    Ok(ValidatedRowSoftmaxWorkerHandoffV1 {
        frontend_authority_commitment: prepared.frontend_authority_commitment,
        exponential_boundary_commitment: prepared.exponential_boundary_commitment,
        handoff: prepared.handoff,
    })
}

pub(crate) fn publish_prepared_flash_attention_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedFlashAttentionV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let module = std::str::from_utf8(prepared.handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingFlashAttentionBindings)?;
    let sections = decode_exact_compiler_sections_v1(
        module,
        &[
            fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            crate::kernel_ir_codegen::FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_V1,
            crate::kernel_ir_codegen::FLASH_ATTENTION_AUTHORITY_SECTION_V1,
            crate::kernel_ir_codegen::FLASH_ATTENTION_OCML_BOUNDARY_SECTION_V1,
        ],
    )
    .ok_or(WorkerV2ProducerError::MissingFlashAttentionBindings)?;
    let exact = sections[0] == prepared.descriptor_source_bytes
        && sections[1] == prepared.authority_transcript
        && sections[2] == prepared.frontend_authority_commitment
        && sections[3] == prepared.ocml_boundary_commitment
        && <[u8; 32]>::from(Sha256::digest(&sections[1])) == prepared.frontend_authority_commitment
        && prepared.handoff.identity() == prepared.expected_handoff_identity
        && prepared.handoff.target().to_string()
            == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        && prepared.handoff.code_object_version() == CodeObjectVersion::V6;
    if !exact {
        return Err(WorkerV2ProducerError::MissingFlashAttentionBindings);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        prepared.handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published exact FlashAttention Worker V2 handoff bound to frontend authority {} and explicit OCML boundary {}",
        crate::encode_hex(&prepared.frontend_authority_commitment),
        crate::encode_hex(&prepared.ocml_boundary_commitment),
    );
    Ok(receipt)
}

pub(crate) fn publish_prepared_moe_top2_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedMoeTop2V1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let sections = decode_moe_top2_v1_sections(prepared.handoff.module_bytes())
        .ok_or(WorkerV2ProducerError::MissingMoeTop2Bindings)?;
    if sections.len() != 17
        || sections[4] != prepared.source_authority_identity
        || sections[13] != prepared.canonical_ir_identity
        || sections[14] != prepared.descriptor_profile_identity
        || prepared.handoff.identity() != prepared.expected_handoff_identity
        || prepared.handoff.target().to_string() != AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        || prepared.handoff.code_object_version() != CodeObjectVersion::V6
    {
        return Err(WorkerV2ProducerError::MissingMoeTop2Bindings);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        prepared.handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published exact MoE Worker V2 handoff bound to source authority {}, canonical Kernel IR {}, and descriptor profile {}",
        crate::encode_hex(&prepared.source_authority_identity),
        crate::encode_hex(&prepared.canonical_ir_identity),
        crate::encode_hex(&prepared.descriptor_profile_identity),
    );
    Ok(receipt)
}

fn decode_moe_top2_v1_sections(module: &[u8]) -> Option<Vec<Vec<u8>>> {
    let module = std::str::from_utf8(module).ok()?;
    let suffixes = [
        "source.v1",
        "namespace.v1",
        "crate.v1",
        "authority.v1",
        "mir.v1",
        "fnabi.v1",
        "compiler.v1",
        "terminals.v3",
        "abi.v1",
        "effects.v1",
        "profile.v1",
        "routing.v1",
        "kir.v1",
        "descriptor.v1",
        "provider.v1",
        "layout.v1",
    ];
    let mut expected = Vec::with_capacity(1 + suffixes.len());
    expected.push(fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V1.to_owned());
    expected.extend(suffixes.map(|suffix| {
        format!(
            "{}.{}",
            crate::kernel_ir_codegen::MOE_TOP2_SECTION_PREFIX_V1,
            suffix
        )
    }));
    let lines = module.lines().collect::<Vec<_>>();
    let declarations = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            canonical_module_assembly_section_name(line).map(|name| (index, name))
        })
        .collect::<Vec<_>>();
    if declarations.len() != expected.len()
        || declarations
            .iter()
            .zip(&expected)
            .any(|((_, actual), expected)| *actual != expected)
    {
        return None;
    }
    let mut decoded = vec![Vec::new(); declarations.len()];
    for (section, (start, _)) in declarations.iter().enumerate() {
        if lines.get(start + 1) != Some(&"module asm \".balign 8\"") {
            return None;
        }
        let end = declarations
            .get(section + 1)
            .map_or(lines.len(), |(index, _)| *index);
        for line in lines.get(start + 2..end)? {
            let values = line
                .strip_prefix("module asm \".byte ")?
                .strip_suffix('"')?;
            for value in values.split(", ") {
                let digits = value.strip_prefix("0x")?;
                if digits.len() != 2
                    || !digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return None;
                }
                decoded[section].push(u8::from_str_radix(digits, 16).ok()?);
            }
        }
        if decoded[section].is_empty() {
            return None;
        }
    }
    Some(decoded)
}

fn module_asm_byte_line(bytes: &[u8]) -> String {
    let mut line = String::from("module asm \".byte ");
    for (index, byte) in bytes.iter().copied().enumerate() {
        if index != 0 {
            line.push_str(", ");
        }
        line.push_str(&format!("0x{byte:02x}"));
    }
    line.push('"');
    line
}

#[cfg(test)]
fn module_asm_commitment_section(section: &str, bytes: &[u8]) -> String {
    let mut text = format!(
        "\nmodule asm \".section {section},\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n"
    );
    for chunk in bytes.chunks(16) {
        text.push_str(&module_asm_byte_line(chunk));
        text.push('\n');
    }
    text
}

/// Prepares the generic production pipeline's exact LLVM text for Worker V2.
///
/// The module has already been target-bound and lowered. This transition only
/// derives the closed symbol manifest, binds the target/COV envelope, and
/// constructs canonical coordination bytes. It performs no LLVM invocation,
/// linking, artifact publication, load, or launch.
pub(crate) fn prepare_production_v1_worker_handoff(
    authenticated: crate::production_pipeline_v1::AuthenticatedProductionGfx942ModuleV1,
) -> Result<PreparedProductionLineageWorkerHandoffV3, WorkerV2ProducerError> {
    let (formal, module, llvm_ir, typed_roots, compiler_ffi_envelope) = authenticated.into_parts();
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed production target is valid");
    validate_exact_target_binding(target, &module)?;
    let compiler_module = retain_production_gfx942_compiler_module_text_v1(&module, llvm_ir)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let envelope = match compiler_ffi_envelope {
        Some(envelope) => envelope,
        None => CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(WorkerV2ProducerError::CompilerEnvelope)?,
    };
    validate_exact_target_binding(envelope.target(), &module)?;
    validate_envelope_module_roles(&envelope, &compiler_module)?;
    let descriptor_source = construct_production_v1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &typed_roots,
        &formal,
    )
    .map_err(WorkerV2ProducerError::CompilerDescriptor)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let symbol_manifest = construct_symbol_manifest(&compiler_module)?;
    let llvm_ir_sha256 = Sha256::digest(compiler_module.llvm_ir().as_bytes()).into();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    Ok(PreparedProductionLineageWorkerHandoffV3 {
        worker_handoff: PreparedProductionV1WorkerHandoffV1 {
            llvm_ir_sha256,
            handoff,
        },
        compiler_descriptor_source: descriptor_source,
    })
}

/// Consumes exact frontend authority and prepares the canonical scalar GEMM V1
/// compiler-module handoff expected by the existing Worker V2 validator.
///
/// Canonical Kernel IR is selected before this function and cannot be supplied
/// by a caller. This path uses no COMGR, subprocess compiler, or command-line
/// linker and performs no publication or worker execution.
pub(crate) fn prepare_scalar_gemm_v1_worker_handoff(
    authenticated: AuthenticatedScalarGemmModuleV1,
) -> Result<PreparedScalarGemmV1WorkerHandoffV1, WorkerV2ProducerError> {
    let frontend_authority_commitment = *authenticated.authority_commitment();
    let (module, descriptor_source) = authenticated.into_parts();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_scalar_gemm_v1_module_text(&module)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = crate::kernel_ir_codegen::bind_scalar_gemm_frontend_authority_v1(
        compiler_module,
        frontend_authority_commitment,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed scalar GEMM target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            SCALAR_GEMM_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            SCALAR_GEMM_V1_DESCRIPTOR,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    Ok(PreparedScalarGemmV1WorkerHandoffV1 {
        frontend_authority_commitment,
        handoff,
    })
}

/// Consumes exact source authority and prepares only the canonical WG64 tiled
/// GEMM LLVM handoff. Lowering uses upstream LLVM-facing text and the Worker V2
/// protocol; neither this path nor its publication invokes COMGR.
pub(crate) fn prepare_tiled_gemm_v1_worker_handoff(
    authenticated: AuthenticatedTiledGemmModuleV1,
) -> Result<PreparedTiledGemmV1WorkerHandoffV1, WorkerV2ProducerError> {
    let frontend_authority_commitment = *authenticated.authority_commitment();
    let (module, descriptor_source) = authenticated.into_parts();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_tiled_gemm_v1_module_text(&module)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = crate::kernel_ir_codegen::bind_tiled_gemm_frontend_authority_v1(
        compiler_module,
        frontend_authority_commitment,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed tiled GEMM target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            fe2o3_kernel_ir::TILED_GEMM_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            TILED_GEMM_V1_DESCRIPTOR,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    Ok(PreparedTiledGemmV1WorkerHandoffV1 {
        frontend_authority_commitment,
        handoff,
    })
}

/// Consumes exact attributed-source authority and prepares the dedicated
/// upstream-LLVM LDS Slice 1 Worker V2 handoff. No worker, linker, code-object
/// builder, runtime, hardware, or COMGR path is entered.
pub(crate) fn prepare_tiled_gemm_lds_slice1_worker_handoff(
    authenticated: AuthenticatedLdsSlice1ModuleV1,
) -> Result<PreparedTiledGemmLdsSlice1WorkerHandoffV1, WorkerV2ProducerError> {
    let source_authority_commitment = *authenticated.source_authority_commitment();
    let canonical_ir_identity = *authenticated.canonical_ir_identity();
    let descriptor_source_identity = *authenticated.descriptor_source().identity().sha256();
    let descriptor_source_bytes = authenticated.descriptor_source().canonical_bytes().to_vec();
    let resource_transcript = authenticated.resource_transcript().to_vec();
    let (
        _module,
        descriptor_source,
        compiler_module,
        retained_authority,
        retained_ir,
        retained_resources,
    ) = authenticated.into_parts();
    if retained_authority != source_authority_commitment
        || retained_ir != canonical_ir_identity
        || retained_resources != resource_transcript
        || descriptor_source.identity().sha256() != &descriptor_source_identity
    {
        return Err(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings);
    }
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed LDS Slice 1 target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    validate_tiled_gemm_lds_slice1_compiler_module_evidence_v1(
        &descriptor_source,
        &envelope,
        &compiler_module,
    )
    .map_err(WorkerV2ProducerError::CompilerDescriptor)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = crate::kernel_ir_codegen::bind_tiled_gemm_lds_slice1_authority_v1(
        compiler_module,
        source_authority_commitment,
        &resource_transcript,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            TILED_GEMM_LDS_SLICE1_DESCRIPTOR,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    let expected_handoff_identity = handoff.identity();
    Ok(PreparedTiledGemmLdsSlice1WorkerHandoffV1 {
        source_authority_commitment,
        canonical_ir_identity,
        descriptor_source_identity,
        descriptor_source_bytes,
        resource_transcript,
        expected_handoff_identity,
        handoff,
    })
}

/// Constructs the exact compiler-owned OCML import observation retained by
/// both descriptor evidence and the Worker V2 handoff.
pub(crate) fn construct_row_softmax_v1_compiler_envelope(
    exponential_boundary_commitment: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed row-softmax target is valid");
    let semantic_text = crate::encode_hex(&exponential_boundary_commitment);
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol: ROW_SOFTMAX_OCML_EXP_SYMBOL_V1,
        calling_convention: "C",
        code_object_version: 6,
        target: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        physical_abi: ROW_SOFTMAX_OCML_EXP_ABI_V1,
        effects: ROW_SOFTMAX_OCML_EXP_EFFECTS_V1,
        semantic_identity: &semantic_text,
    };
    let contract = CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Import,
        CompilerFfiLinkRoleV1::RequiresExternalDefinition,
        target,
        CodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "rustc_codegen_fe2o3",
            "rustc_codegen_fe2o3::row_softmax_v1::__ocml_exp_f32",
            [0x72; 16],
            "__fe2o3_compiler_owned_row_softmax_ocml_exp_f32_v1",
        )?,
        ROW_SOFTMAX_OCML_EXP_SYMBOL_V1,
        ROW_SOFTMAX_OCML_EXP_ABI_V1,
        ROW_SOFTMAX_OCML_EXP_EFFECTS_V1,
        exponential_boundary_commitment,
    )?;
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V6, 1)?;
    builder.push(contract)?;
    builder.finish()
}

/// Consumes exact source authority and prepares the canonical row-softmax LLVM
/// handoff. No OCML provider bytes are selected and no worker is executed.
pub(crate) fn prepare_row_softmax_v1_worker_handoff(
    authenticated: AuthenticatedRowSoftmaxModuleV1,
) -> Result<PreparedRowSoftmaxV1WorkerHandoffV1, WorkerV2ProducerError> {
    let frontend_authority_commitment = *authenticated.authority_commitment();
    let exponential_boundary_commitment = *authenticated.exponential_boundary_commitment();
    let (module, descriptor_source, authority_transcript) = authenticated.into_parts();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_row_softmax_v1_module_text(&module)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module = crate::kernel_ir_codegen::bind_row_softmax_frontend_authority_v1(
        compiler_module,
        &authority_transcript,
        frontend_authority_commitment,
        exponential_boundary_commitment,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    validate_exact_row_softmax_module_closure(&compiler_module)?;
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed row-softmax target is valid");
    let envelope = construct_row_softmax_v1_compiler_envelope(exponential_boundary_commitment)
        .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    validate_envelope_module_roles(&envelope, &compiler_module)?;
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            ROW_SOFTMAX_V1_DESCRIPTOR,
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            ROW_SOFTMAX_OCML_EXP_SYMBOL_V1,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    Ok(PreparedRowSoftmaxV1WorkerHandoffV1 {
        authority_transcript,
        frontend_authority_commitment,
        exponential_boundary_commitment,
        handoff,
    })
}

pub(crate) fn validate_exact_row_softmax_module_closure(
    module: &InertCompilerModuleTextV1,
) -> Result<(), WorkerV2ProducerError> {
    let exact = module.kernel_entries()
        == [crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1]
        && module.device_definitions().is_empty()
        && module.internal_helpers().is_empty()
        && module.device_ffi_exports().is_empty()
        && module.external_declarations() == [ROW_SOFTMAX_OCML_EXP_SYMBOL_V1]
        && module.descriptor_source_identity().is_some();
    if !exact {
        return Err(WorkerV2ProducerError::RowSoftmaxClosureMismatch);
    }
    Ok(())
}

pub(crate) fn construct_flash_attention_v1_compiler_envelope(
    ocml_boundary_commitment: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed FlashAttention target is valid");
    let semantic_text = crate::encode_hex(&ocml_boundary_commitment);
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol: FLASH_ATTENTION_OCML_EXP_SYMBOL_V1,
        calling_convention: "C",
        code_object_version: 6,
        target: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        physical_abi: FLASH_ATTENTION_OCML_EXP_ABI_V1,
        effects: FLASH_ATTENTION_OCML_EXP_EFFECTS_V1,
        semantic_identity: &semantic_text,
    };
    let contract = CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Import,
        CompilerFfiLinkRoleV1::RequiresExternalDefinition,
        target,
        CodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "rustc_codegen_fe2o3",
            "rustc_codegen_fe2o3::flash_attention_v1::__ocml_exp_f32",
            [0x46; 16],
            "__fe2o3_compiler_owned_flash_attention_ocml_exp_f32_v1",
        )?,
        FLASH_ATTENTION_OCML_EXP_SYMBOL_V1,
        FLASH_ATTENTION_OCML_EXP_ABI_V1,
        FLASH_ATTENTION_OCML_EXP_EFFECTS_V1,
        ocml_boundary_commitment,
    )?;
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V6, 1)?;
    builder.push(contract)?;
    builder.finish()
}

/// Consumes exact Flash source/KIR authority into one closed compiler handoff.
/// This performs no provider lookup, LLVM execution, linking, or artifact work.
pub(crate) fn prepare_flash_attention_v1_worker_handoff(
    inputs: FlashAttentionFinalizationInputsV1,
    typed_roots: Vec<TypedDescriptorRootV1>,
) -> Result<PreparedFlashAttentionV1WorkerHandoffV1, WorkerV2ProducerError> {
    let ocml_boundary_commitment: [u8; 32] =
        Sha256::digest(FLASH_ATTENTION_OCML_BOUNDARY_V1).into();
    let required_transcript_fields = [
        inputs.source_identity.as_slice(),
        inputs.source_namespace.as_slice(),
        inputs.compiler_crate_binding.as_slice(),
        inputs.portable_mir_identity.as_slice(),
        inputs.compiler_semantics_identity.as_slice(),
        inputs.fn_abi_identity.as_slice(),
        inputs.trusted_definitions_identity.as_slice(),
        inputs.abi_identity.as_slice(),
        inputs.effects_identity.as_slice(),
        inputs.numerical_identity.as_slice(),
        inputs.descriptor_identity.as_slice(),
        inputs.canonical_ir_identity.as_slice(),
    ];
    if <[u8; 32]>::from(Sha256::digest(&inputs.authority_transcript))
        != inputs.source_authority_identity
        || !required_transcript_fields
            .iter()
            .all(|field| transcript_contains_field(&inputs.authority_transcript, field))
    {
        return Err(WorkerV2ProducerError::MissingFlashAttentionBindings);
    }
    let mut compiler_module =
        crate::kernel_ir_codegen::construct_inert_flash_attention_v1_module_text(
            &inputs.ir,
            &inputs.profile,
        )
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let envelope = construct_flash_attention_v1_compiler_envelope(ocml_boundary_commitment)
        .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    validate_envelope_module_roles(&envelope, &compiler_module)?;
    let descriptor_source = construct_flash_attention_v1_compiler_descriptor_source_v1(
        &envelope,
        &compiler_module,
        &typed_roots,
        &inputs.ir,
        &inputs.profile,
    )
    .map_err(WorkerV2ProducerError::CompilerDescriptor)?;
    let descriptor_source_bytes = descriptor_source.canonical_bytes().to_vec();
    compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    compiler_module = crate::kernel_ir_codegen::bind_flash_attention_v1_authority(
        compiler_module,
        &inputs.authority_transcript,
        inputs.source_authority_identity,
        ocml_boundary_commitment,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    validate_exact_flash_attention_module_closure(&compiler_module)?;
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            FLASH_ATTENTION_V1_DESCRIPTOR,
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            FLASH_ATTENTION_OCML_EXP_SYMBOL_V1,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed FlashAttention target is valid");
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    let expected_handoff_identity = handoff.identity();
    Ok(PreparedFlashAttentionV1WorkerHandoffV1 {
        authority_transcript: inputs.authority_transcript,
        frontend_authority_commitment: inputs.source_authority_identity,
        ocml_boundary_commitment,
        descriptor_source_bytes,
        expected_handoff_identity,
        handoff,
    })
}

/// Consumes the authenticated exact MoE source/KIR receipt and prepares its
/// one-kernel, provider-free upstream-LLVM Worker V2 handoff.
pub(crate) fn prepare_moe_top2_v1_worker_handoff(
    authenticated: AuthenticatedMoeTop2V1,
) -> Result<PreparedMoeTop2V1WorkerHandoffV1, WorkerV2ProducerError> {
    let parts = authenticated.into_worker_parts();
    let source_authority_identity = parts.source_authority_identity;
    let canonical_ir_identity = parts.canonical_ir_identity;
    let descriptor_profile_identity = parts.descriptor_identity;
    let compiler_module = crate::kernel_ir_codegen::construct_inert_moe_top2_v1_module_text(
        &parts.ir,
        &parts.profile,
    )
    .map_err(WorkerV2ProducerError::CompilerModule)?;
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed MoE target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(WorkerV2ProducerError::CompilerEnvelope)?;
    let descriptor =
        crate::compiler_descriptor::construct_moe_top2_v1_compiler_descriptor_source_v1(
            &parts,
            &envelope,
            &compiler_module,
        )
        .map_err(WorkerV2ProducerError::CompilerDescriptor)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    let compiler_module =
        crate::kernel_ir_codegen::bind_moe_top2_v1_identities(compiler_module, &parts)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    if compiler_module.kernel_entries() != [fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID]
        || compiler_module.internal_helpers().len() != 5
        || !compiler_module.device_definitions().is_empty()
        || !compiler_module.device_ffi_exports().is_empty()
        || !compiler_module.external_declarations().is_empty()
        || compiler_module.descriptor_source_identity() != Some(descriptor.identity())
    {
        return Err(WorkerV2ProducerError::MoeTop2ClosureMismatch);
    }
    let symbol_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            MOE_TOP2_V1_DESCRIPTOR,
        ),
    ])
    .map_err(WorkerV2ProducerError::SymbolManifest)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;
    let expected_handoff_identity = handoff.identity();
    Ok(PreparedMoeTop2V1WorkerHandoffV1 {
        source_authority_identity,
        canonical_ir_identity,
        descriptor_profile_identity,
        expected_handoff_identity,
        handoff,
    })
}

fn validate_exact_flash_attention_module_closure(
    module: &InertCompilerModuleTextV1,
) -> Result<(), WorkerV2ProducerError> {
    let exact = module.kernel_entries() == [fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID]
        && module.device_definitions().is_empty()
        && module.internal_helpers().is_empty()
        && module.device_ffi_exports().is_empty()
        && module.external_declarations() == [FLASH_ATTENTION_OCML_EXP_SYMBOL_V1]
        && module.descriptor_source_identity().is_some();
    if !exact {
        return Err(WorkerV2ProducerError::FlashAttentionClosureMismatch);
    }
    Ok(())
}

/// Constructs and publishes one canonical, inert compiler-module handoff.
///
/// The handoff remains coordination data. Publication proves possession of the cooperative build
/// attempt and exact byte identity; it does not grant artifact, link, load, or launch authority.
#[cfg(test)]
pub(crate) fn publish_worker_v2_compiler_module(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: Option<BuildAttempt>,
    envelope: Option<&CompilerFfiEnvelopeV1>,
    module: &Module,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    publish_worker_v2_compiler_module_with_descriptors(
        output_dir,
        producer,
        attempt,
        envelope,
        module,
        &[],
        None,
    )
}

pub(crate) fn publish_worker_v2_compiler_module_with_descriptors(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: Option<BuildAttempt>,
    envelope: Option<&CompilerFfiEnvelopeV1>,
    module: &Module,
    typed_roots: &[TypedDescriptorRootV1],
    source_debug: Option<&crate::source_debug::AlphaSourceDebugV2>,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let attempt = attempt.ok_or(WorkerV2ProducerError::MissingBuildAttempt)?;
    let envelope = envelope.ok_or(WorkerV2ProducerError::MissingCompilerFfiEnvelope)?;
    validate_exact_target_binding(envelope.target(), module)?;
    let module = bind_g1_launch_contract(module)?;
    let module = bind_exact_target_wave_mode(envelope, &module)?;
    let mut compiler_module =
        construct_inert_compiler_module_text_for_target_v1(&module, Some(envelope.target()))
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    if let Some(source_debug) = source_debug {
        compiler_module = bind_source_debug_metadata_v1(compiler_module, source_debug)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    }
    if let Some(source) =
        construct_compiler_descriptor_source_v1(envelope, &module, &compiler_module, typed_roots)
            .map_err(WorkerV2ProducerError::CompilerDescriptor)?
    {
        compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &source)
            .map_err(WorkerV2ProducerError::CompilerModule)?;
    }
    validate_envelope_module_roles(envelope, &compiler_module)?;

    let symbol_manifest = construct_symbol_manifest(&compiler_module)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        envelope.target(),
        envelope.code_object_version(),
        envelope.clone(),
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;

    if let Some(source_debug) = source_debug {
        let semantic = source_debug.semantic_claim();
        let observation = source_debug.build_claim();
        eprintln!(
            "[rustc-codegen-fe2o3] S09 SemanticIdentityClaimV2: schema=fe2o3-s09-semantic-identity-claim-v2; identity_sha256={}; portable_mir_sha256={}",
            crate::encode_hex(semantic.identity_sha256()),
            crate::encode_hex(semantic.portable_mir_sha256()),
        );
        eprintln!(
            "[rustc-codegen-fe2o3] S09 BuildIdentityClaimV2: schema=fe2o3-s09-build-identity-claim-v2; identity_sha256={}; cargo_metadata_sha256={}; prepared_rustc_command_sha256={}; cargo_fe2o3_executable_sha256={}; declared_cargo_executable_sha256={}; pinned_cargo_image_sha256={}; observed_parent_pid={}; observed_parent_start_time_ticks={}; observed_def_path={}; observed_symbol={}",
            crate::encode_hex(observation.identity_sha256()),
            crate::encode_hex(observation.cargo_metadata_sha256()),
            crate::encode_hex(observation.prepared_rustc_command_sha256()),
            crate::encode_hex(observation.cargo_fe2o3_executable_sha256()),
            crate::encode_hex(observation.declared_cargo_executable_sha256()),
            crate::encode_hex(observation.pinned_cargo_image_sha256()),
            observation.observed_parent_pid(),
            observation.observed_parent_start_time_ticks(),
            observation.observed_def_path(),
            observation.observed_symbol(),
        );
    }

    publish_compiler_module_handoff_v1(output_dir, producer, attempt, handoff.canonical_bytes())
        .map_err(WorkerV2ProducerError::Publication)
}

fn construct_symbol_manifest(
    module: &InertCompilerModuleTextV1,
) -> Result<CompilerModuleSymbolManifestV1, WorkerV2ProducerError> {
    use CompilerModuleSymbolRoleV1 as Role;

    let mut entries = Vec::new();
    entries.extend(
        module
            .kernel_entries()
            .iter()
            .cloned()
            .map(|symbol| (Role::KernelEntry, symbol)),
    );
    entries.extend(
        module
            .kernel_entries()
            .iter()
            .map(|symbol| (Role::KernelDescriptor, format!("{symbol}.kd"))),
    );
    entries.extend(
        module
            .device_ffi_exports()
            .iter()
            .cloned()
            .map(|symbol| (Role::DeviceFfiExport, symbol)),
    );
    entries.extend(
        module
            .internal_helpers()
            .iter()
            .cloned()
            .map(|symbol| (Role::InternalHelper, symbol)),
    );
    entries.extend(
        module
            .external_declarations()
            .iter()
            .cloned()
            .map(|symbol| (Role::UnresolvedExternalImport, symbol)),
    );
    CompilerModuleSymbolManifestV1::new(entries).map_err(WorkerV2ProducerError::SymbolManifest)
}

fn bind_g1_launch_contract(module: &Module) -> Result<Module, WorkerV2ProducerError> {
    let required = WorkgroupSize::new(G1_WORKGROUP_X, 1, 1);
    let mut bound = module.clone();
    for kernel in &mut bound.kernels {
        match kernel.workgroup_size {
            None => kernel.workgroup_size = Some(required),
            Some(declared) if declared == required => {}
            Some(declared) => {
                return Err(WorkerV2ProducerError::ConflictingWorkgroupSize {
                    kernel: kernel.id.as_str().to_owned(),
                    declared,
                    required,
                });
            }
        }
    }
    Ok(bound)
}

fn bind_exact_target_wave_mode(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
) -> Result<Module, WorkerV2ProducerError> {
    let target = envelope.target().as_amd_target_id();
    let capabilities = target
        .capabilities()
        .map_err(WorkerV2ProducerError::TargetCapabilities)?;
    let mut declared = BTreeSet::new();
    for capability in module
        .required_capabilities
        .iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| &function.required_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| &kernel.required_capabilities),
        )
    {
        if let TargetCapability::WaveWidth(width) = capability {
            declared.insert(*width);
        }
    }
    for width in &declared {
        let target_width = match width {
            WaveWidth::Wave32 => WavefrontWidth::Wave32,
            WaveWidth::Wave64 => WavefrontWidth::Wave64,
        };
        if !capabilities.wavefront_widths().contains(target_width) {
            return Err(WorkerV2ProducerError::UnsupportedWaveMode {
                target: envelope.target().to_string(),
                width: *width,
            });
        }
    }

    // A single selected mode can safely govern standalone exports and helper
    // SCCs. Mixed-mode modules retain their per-root claims; an unclaimed
    // standalone SCC then remains an explicit lowering error.
    if declared.len() > 1 {
        return Ok(module.clone());
    }
    let width = declared.into_iter().next().unwrap_or_else(|| {
        match capabilities.default_wavefront_width() {
            WavefrontWidth::Wave32 => WaveWidth::Wave32,
            WavefrontWidth::Wave64 => WaveWidth::Wave64,
        }
    });
    let mut bound = module.clone();
    bound
        .required_capabilities
        .insert(TargetCapability::WaveWidth(width));
    Ok(bound)
}

fn validate_exact_target_binding(
    envelope_target: fe2o3_compiler_ffi::DeviceTargetV1,
    module: &Module,
) -> Result<(), WorkerV2ProducerError> {
    let bindings = module
        .required_capabilities
        .iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| &function.required_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| &kernel.required_capabilities),
        )
        .filter_map(|capability| match capability {
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
            {
                Some(name.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if bindings.is_empty() {
        return Ok(());
    }

    let envelope_target = envelope_target.to_string();
    if bindings.len() == 1
        && bindings.contains(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        && envelope_target == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
    {
        return Ok(());
    }

    Err(WorkerV2ProducerError::TargetBindingMismatch {
        module: bindings.into_iter().collect(),
        envelope: envelope_target,
    })
}

fn validate_envelope_module_roles(
    envelope: &CompilerFfiEnvelopeV1,
    module: &InertCompilerModuleTextV1,
) -> Result<(), WorkerV2ProducerError> {
    let symbols = envelope.directional_symbols();
    for symbol in symbols.imports() {
        if module
            .external_declarations()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(WorkerV2ProducerError::MissingExternalDeclaration(
                symbol.to_owned(),
            ));
        }
    }
    for symbol in symbols.exports() {
        if module
            .device_ffi_exports()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(WorkerV2ProducerError::MissingCompilerDefinition(
                symbol.to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum WorkerV2ProducerError {
    MissingBuildAttempt,
    MissingCompilerFfiEnvelope,
    MissingProductionBindings,
    MissingScalarFrontendAuthority,
    MissingTiledFrontendAuthority,
    MissingTiledGemmLdsSlice1Bindings,
    MissingRowSoftmaxBindings,
    RowSoftmaxClosureMismatch,
    MissingFlashAttentionBindings,
    FlashAttentionClosureMismatch,
    MissingMoeTop2Bindings,
    MoeTop2ClosureMismatch,
    MissingExternalDeclaration(String),
    MissingCompilerDefinition(String),
    TargetCapabilities(CapabilityDerivationError),
    UnsupportedWaveMode {
        target: String,
        width: WaveWidth,
    },
    TargetBindingMismatch {
        module: Vec<String>,
        envelope: String,
    },
    ConflictingWorkgroupSize {
        kernel: String,
        declared: WorkgroupSize,
        required: WorkgroupSize,
    },
    CompilerModule(CompilerModuleConstructionError),
    CompilerEnvelope(CompilerFfiEnvelopeError),
    CompilerDescriptor(CompilerDescriptorError),
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    Handoff(CompilerModuleHandoffErrorV2),
    Publication(HandoffPublicationErrorV1),
    ProtectedPublication(HandoffPublicationErrorV2),
}

impl fmt::Display for WorkerV2ProducerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBuildAttempt => {
                formatter.write_str("kernel-ir-worker-v2 requires a managed FE2O3_BUILD_ATTEMPT_V1")
            }
            Self::MissingCompilerFfiEnvelope => {
                formatter.write_str("kernel-ir-worker-v2 requires a complete compiler FFI envelope")
            }
            Self::MissingProductionBindings => formatter.write_str(
                "production-v1 Worker V2 handoff lost its exact LLVM identity binding",
            ),
            Self::MissingScalarFrontendAuthority => formatter.write_str(
                "scalar GEMM compiler-module handoff lost its embedded frontend authority",
            ),
            Self::MissingTiledFrontendAuthority => formatter.write_str(
                "tiled GEMM compiler-module handoff lost its embedded frontend authority",
            ),
            Self::MissingTiledGemmLdsSlice1Bindings => formatter.write_str(
                "LDS Slice 1 compiler-module handoff lost its source, IR, descriptor, target, or resource binding",
            ),
            Self::MissingRowSoftmaxBindings => formatter.write_str(
                "row-softmax compiler-module handoff lost its frontend or exponential-boundary binding",
            ),
            Self::RowSoftmaxClosureMismatch => formatter.write_str(
                "row-softmax compiler-module symbol closure is not exactly one kernel, one descriptor, and the OCML exp import",
            ),
            Self::MissingFlashAttentionBindings => formatter.write_str(
                "FlashAttention compiler-module handoff lost its exact source, KIR, descriptor, target, or OCML-boundary binding",
            ),
            Self::FlashAttentionClosureMismatch => formatter.write_str(
                "FlashAttention compiler-module symbol closure is not exactly one kernel, one descriptor, and the OCML exp import",
            ),
            Self::MissingMoeTop2Bindings => formatter.write_str(
                "MoE compiler-module handoff lost an authenticated source/KIR/compiler/profile/provider/layout binding",
            ),
            Self::MoeTop2ClosureMismatch => formatter.write_str(
                "MoE compiler-module closure is not exactly one kernel, five private helpers, one descriptor, and no providers",
            ),
            Self::MissingExternalDeclaration(symbol) => write!(
                formatter,
                "compiler FFI import {symbol:?} is absent from the whole kernel IR module's external declarations"
            ),
            Self::MissingCompilerDefinition(symbol) => write!(
                formatter,
                "compiler FFI export {symbol:?} is absent from the whole kernel IR module's device FFI definitions"
            ),
            Self::TargetCapabilities(error) => {
                write!(
                    formatter,
                    "cannot derive exact target capabilities: {error}"
                )
            }
            Self::UnsupportedWaveMode { target, width } => write!(
                formatter,
                "compiler module requires {width:?}, which target {target} does not support"
            ),
            Self::TargetBindingMismatch { module, envelope } => write!(
                formatter,
                "compiler-module exact target bindings {module:?} do not match Worker V2 envelope target {envelope:?}"
            ),
            Self::ConflictingWorkgroupSize {
                kernel,
                declared,
                required,
            } => write!(
                formatter,
                "kernel {kernel:?} declares workgroup size ({}, {}, {}), but the Worker V2 G1 profile requires ({}, {}, {})",
                declared.x, declared.y, declared.z, required.x, required.y, required.z
            ),
            Self::CompilerModule(error) => {
                write!(
                    formatter,
                    "whole compiler-module construction failed: {error}"
                )
            }
            Self::CompilerEnvelope(error) => {
                write!(
                    formatter,
                    "exact FFI-free compiler envelope failed: {error}"
                )
            }
            Self::CompilerDescriptor(error) => {
                write!(
                    formatter,
                    "compiler descriptor construction failed: {error}"
                )
            }
            Self::SymbolManifest(error) => {
                write!(
                    formatter,
                    "compiler symbol manifest construction failed: {error}"
                )
            }
            Self::Handoff(error) => {
                write!(
                    formatter,
                    "compiler-module handoff construction failed: {error}"
                )
            }
            Self::Publication(error) => {
                write!(
                    formatter,
                    "compiler-module handoff publication failed: {error}"
                )
            }
            Self::ProtectedPublication(error) => write!(
                formatter,
                "protected compiler-module V2 handoff publication failed: {error}"
            ),
        }
    }
}

impl Error for WorkerV2ProducerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerModule(error) => Some(error),
            Self::CompilerEnvelope(error) => Some(error),
            Self::CompilerDescriptor(error) => Some(error),
            Self::SymbolManifest(error) => Some(error),
            Self::TargetCapabilities(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::ProtectedPublication(error) => Some(error),
            Self::MissingBuildAttempt
            | Self::MissingCompilerFfiEnvelope
            | Self::MissingProductionBindings
            | Self::MissingScalarFrontendAuthority
            | Self::MissingTiledFrontendAuthority
            | Self::MissingTiledGemmLdsSlice1Bindings
            | Self::MissingRowSoftmaxBindings
            | Self::RowSoftmaxClosureMismatch
            | Self::MissingFlashAttentionBindings
            | Self::FlashAttentionClosureMismatch
            | Self::MissingMoeTop2Bindings
            | Self::MoeTop2ClosureMismatch
            | Self::MissingExternalDeclaration(_)
            | Self::MissingCompilerDefinition(_)
            | Self::UnsupportedWaveMode { .. }
            | Self::TargetBindingMismatch { .. }
            | Self::ConflictingWorkgroupSize { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collected_moe_top2_v1::exact_frontend_receipt_for_test as exact_moe_frontend_receipt_for_test;
    use crate::collected_row_softmax_v1::{
        exact_authority_policy_for_test,
        exact_frontend_receipt_for_test as exact_row_frontend_receipt_for_test,
    };
    use crate::collected_scalar_gemm_v1::exact_frontend_receipt_for_test;
    use crate::collected_tiled_gemm_lds_slice1_v1::exact_lds_slice1_frontend_receipt_for_test;
    use crate::collected_tiled_gemm_v1::exact_frontend_receipt_for_test as exact_tiled_frontend_receipt_for_test;

    #[test]
    fn consumed_moe_receipt_prepares_only_the_exact_closed_handoff() {
        let mut receipt = exact_moe_frontend_receipt_for_test();
        let prepared = prepare_moe_top2_v1_worker_handoff(receipt.consume().unwrap()).unwrap();
        let handoff = prepared.handoff();
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(
            handoff.target().to_string(),
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        );
        assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(handoff.envelope().directional_symbols().total_count(), 0);
        assert_eq!(
            handoff.symbol_manifest().entries().collect::<Vec<_>>(),
            vec![
                (
                    CompilerModuleSymbolRoleV1::KernelEntry,
                    fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID,
                ),
                (
                    CompilerModuleSymbolRoleV1::KernelDescriptor,
                    fe2o3_kernel_ir::MOE_TOP2_V1_DESCRIPTOR_SYMBOL,
                ),
            ]
        );
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(text.contains(crate::moe_top2_v1_codegen::EXACT_MOE_TOP2_GFX942_DATA_LAYOUT_V1));
        assert_eq!(text.matches("define amdgpu_kernel").count(), 1);
        assert_eq!(text.matches("define internal").count(), 5);
        assert!(!text.contains("COMGR"));
        let sections = decode_moe_top2_v1_sections(handoff.module_bytes()).unwrap();
        assert_eq!(sections.len(), 17);
        assert_eq!(sections[4], prepared.source_authority_identity);
        assert_eq!(sections[13], prepared.canonical_ir_identity);
        assert_eq!(sections[14], prepared.descriptor_profile_identity);
        assert_eq!(
            sections[15],
            Sha256::digest(crate::moe_top2_v1_codegen::EMPTY_PROVIDER_CLOSURE_V1).as_slice()
        );
        if let Some(path) = std::env::var_os("FE2O3_TEST_RETAIN_MOE_TOP2_LLVM") {
            std::fs::write(path, handoff.module_bytes()).unwrap();
        }
    }
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, CompilerModuleHandoffErrorV1 as PublicationError,
        CompilerModuleHandoffErrorV2 as ProtectedPublicationError, begin_build_attempt,
        consume_compiler_module_handoff_v1, consume_compiler_module_handoff_v2,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiContractV1,
        CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
        CompilerModuleHandoffV2, DeviceTargetV1, ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1,
        ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1,
        ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1,
    };
    use fe2o3_hsaco_finalize::{
        ContentIdentityV1, LinkOptionV1, PinnedWorkerV1,
        ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
        ROW_SOFTMAX_V1_WORKER_COMPLETE_DIAGNOSTIC_V1, RowSoftmaxV1DirectWorkerExpectationV1,
        RowSoftmaxV1DirectWorkerPinsV1, RowSoftmaxV1OcmlProviderPinsV1, WorkerExecutionLimitsV1,
        WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerStageV1,
        execute_reproducible_first_build_worker_v2,
        finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
        inspect_row_softmax_v1_structural_worker_v2_hsaco_v1,
    };
    use fe2o3_kernel_descriptor::{
        RowSoftmaxV1StructuralDescriptorExpectationV1, decode_device_descriptor_table_v1,
    };
    use fe2o3_kernel_ir::ScalarGemmTargetRequirementsV1;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature,
        TargetCapability, Terminator, WaveWidth, WorkgroupSize,
    };
    use reserved_fe2o3_symbols::{
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };
    use sha2::Sha256;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const IMPORT_ABI: &str =
        "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-worker-v2-producer-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn row_softmax_release_gate_requested() -> bool {
        const RELEASE_GATE_ENV: &str = "FE2O3_ROW_SOFTMAX_RELEASE_GATE";
        match std::env::var(RELEASE_GATE_ENV) {
            Ok(value) => {
                assert_eq!(
                    value, "1",
                    "{RELEASE_GATE_ENV} must be exactly 1 when present",
                );
                true
            }
            Err(std::env::VarError::NotPresent) => false,
            Err(error) => panic!("read {RELEASE_GATE_ENV}: {error}"),
        }
    }

    fn require_row_softmax_release_configuration(
        release_gate: bool,
        configured: usize,
        required: &[&str],
    ) -> bool {
        if configured == 0 && !release_gate {
            return false;
        }
        assert_eq!(
            configured,
            required.len(),
            "row-softmax release gate requires all configuration variables: {}",
            required.join(", "),
        );
        true
    }

    fn parse_manifest_sha256(value: &str) -> [u8; 32] {
        assert_eq!(value.len(), 64, "manifest SHA-256 must contain 64 digits");
        let mut digest = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("manifest SHA-256 must use lowercase hexadecimal"),
            };
            digest[index] = (digit(pair[0]) << 4) | digit(pair[1]);
        }
        digest
    }

    fn parse_manifest_length(value: &str) -> u64 {
        assert!(
            value == "0" || !value.starts_with('0'),
            "manifest byte length is not canonical",
        );
        value.parse().expect("manifest byte length is not u64")
    }

    #[test]
    fn row_softmax_release_gate_fails_closed_on_missing_or_partial_worker_configuration() {
        let required = [
            "worker",
            "worker-build-id",
            "llvm-build-id",
            "worker-sha256",
            "worker-length",
        ];
        assert!(!require_row_softmax_release_configuration(
            false, 0, &required
        ));
        assert!(
            std::panic::catch_unwind(|| {
                require_row_softmax_release_configuration(true, 0, &required)
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                require_row_softmax_release_configuration(true, 2, &required)
            })
            .is_err()
        );
    }

    #[test]
    fn row_softmax_release_manifest_measurement_is_canonical() {
        assert_eq!(parse_manifest_sha256(&"ab".repeat(32)), [0xab; 32]);
        assert_eq!(parse_manifest_length("42375992"), 42_375_992);
        for malformed in [
            "AB".repeat(32),
            "ab".repeat(31),
            format!("g{}", "0".repeat(63)),
        ] {
            assert!(
                std::panic::catch_unwind(|| parse_manifest_sha256(&malformed)).is_err(),
                "accepted malformed manifest SHA-256 {malformed}",
            );
        }
        assert!(std::panic::catch_unwind(|| parse_manifest_length("01")).is_err());
    }

    #[test]
    fn consumed_frontend_receipt_prepares_exact_scalar_gemm_worker_v2_handoff() {
        let mut receipt = exact_frontend_receipt_for_test();
        let authenticated = receipt.consume().expect("consume exact frontend receipt");
        let expected_authority = *authenticated.authority_commitment();
        let prepared = prepare_scalar_gemm_v1_worker_handoff(authenticated)
            .expect("prepare exact scalar GEMM handoff");
        let handoff = prepared.handoff();
        let canonical = dialect_amdgcn::lower_scalar_gemm_v1_to_gfx942_llvm_ir(
            &fe2o3_kernel_ir::scalar_gemm_v1_module(),
            ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
        )
        .unwrap();

        assert_eq!(
            prepared.frontend_authority_commitment(),
            &expected_authority
        );
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(handoff.target(), target());
        assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
        assert!(
            handoff
                .module_bytes()
                .starts_with(canonical.as_str().as_bytes())
        );
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(module_text.contains("module asm \".section .fe2o3.kd.v1"));
        assert!(module_text.contains("module asm \".section .fe2o3.scalar-auth.v1"));
        assert!(
            handoff
                .envelope()
                .directional_symbols()
                .imports()
                .next()
                .is_none()
        );
        assert!(
            handoff
                .envelope()
                .directional_symbols()
                .exports()
                .next()
                .is_none()
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
                .collect::<Vec<_>>(),
            [SCALAR_GEMM_V1_KERNEL_ID]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .collect::<Vec<_>>(),
            [SCALAR_GEMM_V1_DESCRIPTOR]
        );
        assert!(!handoff.authenticates_compiler_origin());
        assert!(!handoff.grants_worker_authority());
        assert!(!handoff.grants_link_authority());
        assert!(!handoff.grants_load_authority());
        assert!(!handoff.grants_launch_authority());
    }

    #[test]
    fn consumed_tiled_receipt_prepares_only_the_exact_wg64_gfx942_handoff() {
        let mut receipt = exact_tiled_frontend_receipt_for_test();
        let authenticated = receipt.consume().expect("consume exact tiled receipt");
        let expected_authority = *authenticated.authority_commitment();
        let prepared = prepare_tiled_gemm_v1_worker_handoff(authenticated)
            .expect("prepare exact tiled GEMM handoff");
        let handoff = prepared.handoff();
        let canonical = dialect_amdgcn::lower_tiled_gemm_v1_to_gfx942_llvm_ir(
            &fe2o3_kernel_ir::tiled_gemm_v1_module(),
            fe2o3_kernel_ir::TiledGemmV1Profile::exact_gfx942_xnack_minus_cov6(),
        )
        .unwrap();

        assert_eq!(
            prepared.frontend_authority_commitment(),
            &expected_authority
        );
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(handoff.target(), target());
        assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
        assert!(
            handoff
                .module_bytes()
                .starts_with(canonical.as_str().as_bytes())
        );
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(module_text.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
        assert!(module_text.contains("!0 = !{i32 64, i32 1, i32 1}"));
        assert_eq!(
            module_text.matches(" = load i16, ptr addrspace(1)").count(),
            8
        );
        assert_eq!(
            module_text
                .matches(" = load float, ptr addrspace(1)")
                .count(),
            4
        );
        assert_eq!(module_text.matches("store float ").count(), 4);
        assert_eq!(
            module_text
                .matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
                .count(),
            1
        );
        assert!(module_text.contains("module asm \".section .fe2o3.kd.v1"));
        assert!(module_text.contains("module asm \".section .fe2o3.tiled-auth.v1"));
        assert!(!module_text.contains(".fe2o3.scalar-auth.v1"));
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
                .collect::<Vec<_>>(),
            [fe2o3_kernel_ir::TILED_GEMM_V1_KERNEL_ID]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .collect::<Vec<_>>(),
            [TILED_GEMM_V1_DESCRIPTOR]
        );
        assert!(!handoff.authenticates_compiler_origin());
        assert!(!handoff.grants_worker_authority());
        assert!(!handoff.grants_link_authority());
        assert!(!handoff.grants_load_authority());
        assert!(!handoff.grants_launch_authority());
    }

    #[test]
    fn consumed_attributed_lds_receipt_prepares_exact_bound_worker_v2_handoff() {
        let mut receipt = exact_lds_slice1_frontend_receipt_for_test();
        let expected_authority = *receipt.authority_commitment();
        let authenticated = receipt.consume().expect("consume exact LDS receipt");
        let expected_ir = *authenticated.canonical_ir_identity();
        let expected_descriptor = *authenticated.descriptor_source().identity().sha256();
        let expected_pre_section_llvm = authenticated
            .compiler_module()
            .llvm_ir()
            .as_bytes()
            .to_vec();
        let prepared = prepare_tiled_gemm_lds_slice1_worker_handoff(authenticated)
            .expect("prepare exact attributed LDS handoff");
        let handoff = prepared.handoff();
        let canonical = dialect_amdgcn::lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
            &fe2o3_kernel_ir::tiled_gemm_lds_v1_module(),
            fe2o3_kernel_ir::TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
        )
        .unwrap();

        assert_eq!(prepared.source_authority_commitment(), &expected_authority);
        assert_eq!(prepared.canonical_ir_identity(), &expected_ir);
        assert_eq!(prepared.descriptor_source_identity(), &expected_descriptor);
        assert_eq!(handoff.target(), target());
        assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
        assert!(
            handoff
                .module_bytes()
                .starts_with(canonical.as_str().as_bytes())
        );
        assert!(
            handoff
                .module_bytes()
                .starts_with(&expected_pre_section_llvm)
        );
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        let [descriptor_bytes, authority, resources] =
            decode_tiled_gemm_lds_slice1_sections_v1(module_text).unwrap();
        assert_eq!(descriptor_bytes, prepared.descriptor_source_bytes);
        assert_eq!(authority, expected_authority);
        assert_eq!(resources, prepared.resource_transcript());
        let descriptor = CompilerDescriptorSourceV1::decode(&descriptor_bytes).unwrap();
        let kernel = &descriptor.table().kernels()[0];
        assert_eq!(kernel.logical_name().as_str(), "tiled_gemm_lds_slice1");
        assert_eq!(
            kernel.entry_name().as_str(),
            fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID
        );
        assert_eq!(kernel.launch().static_shared_memory_bytes(), 1024);
        assert_eq!(
            module_text
                .matches("internal addrspace(3) global [256 x i16]")
                .count(),
            2
        );
        assert_eq!(module_text.matches("s_barrier").count(), 1);
        assert_eq!(
            module_text
                .matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
                .count(),
            1
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
                .collect::<Vec<_>>(),
            [fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .collect::<Vec<_>>(),
            [TILED_GEMM_LDS_SLICE1_DESCRIPTOR]
        );
        assert!(!handoff.authenticates_compiler_origin());
        assert!(!handoff.grants_worker_authority());
        assert!(!handoff.grants_link_authority());
        assert!(!handoff.grants_load_authority());
        assert!(!handoff.grants_launch_authority());
    }

    #[test]
    fn attributed_lds_publication_is_single_use_and_rejects_binding_mutations() {
        let producer = producer();
        let directory = TestDirectory::new();
        let attempt = begin_attempt(&directory.0, &producer);
        let mut first_receipt = exact_lds_slice1_frontend_receipt_for_test();
        let first =
            prepare_tiled_gemm_lds_slice1_worker_handoff(first_receipt.consume().unwrap()).unwrap();
        let expected_bytes = first.handoff().canonical_bytes().to_vec();
        publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
            &directory.0,
            &producer,
            attempt,
            first,
        )
        .unwrap();

        let mut replay_receipt = exact_lds_slice1_frontend_receipt_for_test();
        let replay =
            prepare_tiled_gemm_lds_slice1_worker_handoff(replay_receipt.consume().unwrap())
                .unwrap();
        assert!(matches!(
            publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
                &directory.0,
                &producer,
                attempt,
                replay,
            ),
            Err(WorkerV2ProducerError::Publication(
                PublicationError::AlreadyPublished
            ))
        ));
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        assert_eq!(consumed.bytes(), expected_bytes);
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::AlreadyConsumed)
        ));

        let hostile_directory = TestDirectory::new();
        let hostile_attempt = begin_attempt(&hostile_directory.0, &producer);
        let mut hostile_receipt = exact_lds_slice1_frontend_receipt_for_test();
        let mut hostile =
            prepare_tiled_gemm_lds_slice1_worker_handoff(hostile_receipt.consume().unwrap())
                .unwrap();
        hostile.descriptor_source_bytes[0] ^= 1;
        assert!(matches!(
            publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
                &hostile_directory.0,
                &producer,
                hostile_attempt,
                hostile,
            ),
            Err(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&hostile_directory.0, &producer, hostile_attempt,),
            Err(PublicationError::NotPublished)
        ));

        let substituted_directory = TestDirectory::new();
        let substituted_attempt = begin_attempt(&substituted_directory.0, &producer);
        let mut substituted_receipt = exact_lds_slice1_frontend_receipt_for_test();
        let mut substituted =
            prepare_tiled_gemm_lds_slice1_worker_handoff(substituted_receipt.consume().unwrap())
                .unwrap();
        let original = std::str::from_utf8(substituted.handoff.module_bytes()).unwrap();
        let mutated_module = original.replacen(" = add i64 ", " = sub i64 ", 1);
        assert_ne!(mutated_module, original);
        substituted.handoff = CompilerModuleHandoffV2::new(
            substituted.handoff.kind(),
            substituted.handoff.target(),
            substituted.handoff.code_object_version(),
            substituted.handoff.envelope().clone(),
            substituted.handoff.symbol_manifest().clone(),
            mutated_module.as_bytes(),
        )
        .unwrap();
        assert_ne!(
            substituted.handoff.identity(),
            substituted.expected_handoff_identity
        );
        assert!(matches!(
            publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
                &substituted_directory.0,
                &producer,
                substituted_attempt,
                substituted,
            ),
            Err(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(
                &substituted_directory.0,
                &producer,
                substituted_attempt,
            ),
            Err(PublicationError::NotPublished)
        ));

        let manifest_directory = TestDirectory::new();
        let manifest_attempt = begin_attempt(&manifest_directory.0, &producer);
        let mut manifest_receipt = exact_lds_slice1_frontend_receipt_for_test();
        let mut substituted_manifest =
            prepare_tiled_gemm_lds_slice1_worker_handoff(manifest_receipt.consume().unwrap())
                .unwrap();
        let manifest = CompilerModuleSymbolManifestV1::new([
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                TILED_GEMM_LDS_SLICE1_DESCRIPTOR,
            ),
            (
                CompilerModuleSymbolRoleV1::InternalHelper,
                "__fe2o3_substituted_helper",
            ),
        ])
        .unwrap();
        substituted_manifest.handoff = CompilerModuleHandoffV2::new(
            substituted_manifest.handoff.kind(),
            substituted_manifest.handoff.target(),
            substituted_manifest.handoff.code_object_version(),
            substituted_manifest.handoff.envelope().clone(),
            manifest,
            substituted_manifest.handoff.module_bytes(),
        )
        .unwrap();
        assert_ne!(
            substituted_manifest.handoff.identity(),
            substituted_manifest.expected_handoff_identity
        );
        assert!(matches!(
            publish_prepared_tiled_gemm_lds_slice1_worker_handoff(
                &manifest_directory.0,
                &producer,
                manifest_attempt,
                substituted_manifest,
            ),
            Err(WorkerV2ProducerError::MissingTiledGemmLdsSlice1Bindings)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&manifest_directory.0, &producer, manifest_attempt,),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn consumed_row_receipt_prepares_exact_llvm_descriptor_and_ocml_import_closure() {
        let mut receipt = exact_row_frontend_receipt_for_test();
        let authenticated = receipt.consume().expect("consume exact row receipt");
        let expected_authority = *authenticated.authority_commitment();
        let expected_exponential = *authenticated.exponential_boundary_commitment();
        let prepared = prepare_row_softmax_v1_worker_handoff(authenticated)
            .expect("prepare exact row-softmax handoff");
        let handoff = prepared.handoff();
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(module_text.starts_with(&format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\n",
            crate::kernel_ir_codegen::ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1,
        )));

        assert_eq!(
            prepared.frontend_authority_commitment(),
            &expected_authority
        );
        assert_eq!(
            prepared.exponential_boundary_commitment(),
            &expected_exponential
        );
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(handoff.target(), target());
        assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(
            handoff.module_identity().sha256(),
            &<[u8; 32]>::from(Sha256::digest(handoff.module_bytes()))
        );
        assert_eq!(
            module_text
                .matches("declare float @__ocml_exp_f32(float)")
                .count(),
            1
        );
        assert_eq!(
            module_text
                .matches("call float @__ocml_exp_f32(float ")
                .count(),
            2
        );
        assert!(!module_text.contains("__fe2o3_ir_float_v1_exp_f32"));
        assert!(module_text.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
        assert!(module_text.contains("module asm \".section .fe2o3.kd.v1"));
        assert!(module_text.contains(&module_asm_commitment_section(
            ".fe2o3.row-softmax-auth.v1",
            &expected_authority,
        )));
        assert!(!module_text.contains(".fe2o3.row-auth.v1"));
        assert!(module_text.contains(&module_asm_commitment_section(
            ".fe2o3.row-exp.v1",
            &expected_exponential,
        )));
        assert_eq!(
            handoff
                .envelope()
                .directional_symbols()
                .imports()
                .collect::<Vec<_>>(),
            [ROW_SOFTMAX_OCML_EXP_SYMBOL_V1]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
                .collect::<Vec<_>>(),
            [crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .collect::<Vec<_>>(),
            [ROW_SOFTMAX_V1_DESCRIPTOR]
        );
        assert_eq!(
            handoff
                .symbol_manifest()
                .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
                .collect::<Vec<_>>(),
            [ROW_SOFTMAX_OCML_EXP_SYMBOL_V1]
        );
        assert!(!handoff.authenticates_compiler_origin());
        assert!(!handoff.grants_worker_authority());
        assert!(!handoff.grants_link_authority());
        assert!(!handoff.grants_load_authority());
        assert!(!handoff.grants_launch_authority());

        let provider = RowSoftmaxV1OcmlProviderPinsV1::new(
            [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]],
            [0x45; 32],
        )
        .unwrap();
        let worker = RowSoftmaxV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x46; 32], 1),
            "row-softmax-production-compatibility-test-worker",
            ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
            provider,
        )
        .unwrap();
        RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
            handoff,
            *handoff.identity().sha256(),
            expected_authority,
            exact_authority_policy_for_test(),
            worker,
        )
        .expect("production rustc row handoff is admitted by the direct worker profile");
        if let Some(path) = std::env::var_os("FE2O3_TEST_RETAIN_ROW_SOFTMAX_LLVM") {
            fs::write(path, handoff.module_bytes()).expect("retain exact row-softmax LLVM module");
        }
    }

    #[test]
    fn row_handoff_rejects_import_manifest_and_envelope_substitutions() {
        let mut receipt = exact_row_frontend_receipt_for_test();
        let authenticated = receipt.consume().unwrap();
        let exponential = *authenticated.exponential_boundary_commitment();
        let prepared = prepare_row_softmax_v1_worker_handoff(authenticated).unwrap();
        let handoff = prepared.handoff();
        let without_import = CompilerModuleSymbolManifestV1::new([
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                ROW_SOFTMAX_V1_DESCRIPTOR,
            ),
        ])
        .unwrap();
        assert!(matches!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V6,
                construct_row_softmax_v1_compiler_envelope(exponential).unwrap(),
                without_import,
                handoff.module_bytes(),
            ),
            Err(CompilerModuleHandoffErrorV2::FfiImportRoleMismatch)
        ));

        let substituted_manifest = CompilerModuleSymbolManifestV1::new([
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                ROW_SOFTMAX_V1_DESCRIPTOR,
            ),
            (
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                "__ocml_exp2_f32",
            ),
        ])
        .unwrap();
        assert!(matches!(
            CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V6,
                construct_row_softmax_v1_compiler_envelope(exponential).unwrap(),
                substituted_manifest,
                handoff.module_bytes(),
            ),
            Err(CompilerModuleHandoffErrorV2::FfiImportRoleMismatch)
        ));
    }

    #[test]
    fn row_handoff_publication_is_attempt_scoped_and_bound_sections_are_mandatory() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let mut receipt = exact_row_frontend_receipt_for_test();
        let prepared = prepare_row_softmax_v1_worker_handoff(receipt.consume().unwrap()).unwrap();
        let expected_bytes = prepared.handoff().canonical_bytes().to_vec();

        let publication = publish_prepared_row_softmax_v1_worker_handoff(
            &directory.0,
            &producer,
            attempt,
            prepared,
        )
        .unwrap();
        assert_eq!(publication.attempt(), attempt);

        let mut replay_receipt = exact_row_frontend_receipt_for_test();
        let replay =
            prepare_row_softmax_v1_worker_handoff(replay_receipt.consume().unwrap()).unwrap();
        assert!(matches!(
            publish_prepared_row_softmax_v1_worker_handoff(
                &directory.0,
                &producer,
                attempt,
                replay,
            ),
            Err(WorkerV2ProducerError::Publication(
                PublicationError::AlreadyPublished
            ))
        ));
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        assert_eq!(consumed.bytes(), expected_bytes);
        let decoded = CompilerModuleHandoffV2::decode(consumed.bytes()).unwrap();
        assert_eq!(decoded.symbol_manifest().symbol_count(), 3);

        let rejected_directory = TestDirectory::new();
        let rejected_attempt = begin_attempt(&rejected_directory.0, &producer);
        let mut rejected_receipt = exact_row_frontend_receipt_for_test();
        let rejected =
            prepare_row_softmax_v1_worker_handoff(rejected_receipt.consume().unwrap()).unwrap();
        assert!(!rejected.authority_transcript().is_empty());
        let PreparedRowSoftmaxV1WorkerHandoffV1 {
            authority_transcript,
            frontend_authority_commitment,
            exponential_boundary_commitment,
            handoff,
        } = rejected;
        let mutated_module = std::str::from_utf8(handoff.module_bytes())
            .unwrap()
            .replacen(".fe2o3.row-exp.v1", ".fe2o3.row-exp.v2", 1);
        let mutated_handoff = CompilerModuleHandoffV2::new(
            handoff.kind(),
            handoff.target(),
            handoff.code_object_version(),
            handoff.envelope().clone(),
            handoff.symbol_manifest().clone(),
            mutated_module.as_bytes(),
        )
        .unwrap();
        let rejected = PreparedRowSoftmaxV1WorkerHandoffV1 {
            authority_transcript,
            frontend_authority_commitment,
            exponential_boundary_commitment,
            handoff: mutated_handoff,
        };
        assert!(matches!(
            publish_prepared_row_softmax_v1_worker_handoff(
                &rejected_directory.0,
                &producer,
                rejected_attempt,
                rejected,
            ),
            Err(WorkerV2ProducerError::MissingRowSoftmaxBindings)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&rejected_directory.0, &producer, rejected_attempt,),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn protected_row_publication_binds_every_closure_role_and_exact_canonical_bytes() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let expected_closure = compiler_closure(compiler_closure_pins());
        let mut frontend_receipt = exact_row_frontend_receipt_for_test();
        let prepared =
            prepare_row_softmax_v1_worker_handoff(frontend_receipt.consume().unwrap()).unwrap();
        let expected_bytes = prepared.handoff().canonical_bytes().to_vec();

        let receipt = publish_prepared_row_softmax_v1_worker_handoff_v2(
            &directory.0,
            &producer,
            attempt,
            expected_closure,
            prepared,
        )
        .unwrap();
        assert_eq!(receipt.compiler_closure(), expected_closure);
        assert_eq!(receipt.length(), expected_bytes.len());
        let _distinct_v2_identity: fe2o3_artifact_transaction::CompilerModuleHandoffIdentityV2 =
            receipt.identity();
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));

        for role in 0..compiler_closure_pins().len() {
            let mut substituted = compiler_closure_pins();
            substituted[role][role] ^= 0x80;
            assert!(matches!(
                consume_compiler_module_handoff_v2(
                    &directory.0,
                    &producer,
                    attempt,
                    compiler_closure(substituted),
                ),
                Err(ProtectedPublicationError::WrongCompilerClosure)
            ));
        }

        let consumed =
            consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, expected_closure)
                .unwrap();
        assert_eq!(consumed.compiler_closure(), expected_closure);
        assert_eq!(consumed.bytes(), expected_bytes);
    }

    #[test]
    fn protected_production_publication_returns_the_native_v2_receipt() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let closure = compiler_closure(compiler_closure_pins());
        let mut frontend_receipt = exact_row_frontend_receipt_for_test();
        let row =
            prepare_row_softmax_v1_worker_handoff(frontend_receipt.consume().unwrap()).unwrap();
        let PreparedRowSoftmaxV1WorkerHandoffV1 { handoff, .. } = row;
        let expected_bytes = handoff.canonical_bytes().to_vec();
        let production = PreparedProductionV1WorkerHandoffV1 {
            llvm_ir_sha256: Sha256::digest(handoff.module_bytes()).into(),
            handoff,
        };

        let receipt = publish_prepared_production_v1_worker_handoff_v2(
            &directory.0,
            &producer,
            attempt,
            closure,
            production,
        )
        .unwrap();
        let _identity: fe2o3_artifact_transaction::CompilerModuleHandoffIdentityV2 =
            receipt.identity();
        assert_eq!(receipt.compiler_closure(), closure);
        assert_eq!(receipt.length(), expected_bytes.len());
        let consumed =
            consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure).unwrap();
        assert_eq!(consumed.compiler_closure(), closure);
        assert_eq!(consumed.bytes(), expected_bytes);
    }

    #[test]
    fn ordinary_v1_cannot_cross_into_v2_and_protected_failure_has_no_v1_fallback() {
        let producer = producer();
        let closure = compiler_closure(compiler_closure_pins());

        let ordinary_directory = TestDirectory::new();
        let ordinary_attempt = begin_attempt(&ordinary_directory.0, &producer);
        let mut ordinary_receipt = exact_row_frontend_receipt_for_test();
        let ordinary =
            prepare_row_softmax_v1_worker_handoff(ordinary_receipt.consume().unwrap()).unwrap();
        let ordinary_bytes = ordinary.handoff().canonical_bytes().to_vec();
        let receipt = publish_prepared_row_softmax_v1_worker_handoff(
            &ordinary_directory.0,
            &producer,
            ordinary_attempt,
            ordinary,
        )
        .unwrap();
        let _distinct_v1_identity: fe2o3_artifact_transaction::CompilerModuleHandoffIdentityV1 =
            receipt.identity();
        assert!(matches!(
            consume_compiler_module_handoff_v2(
                &ordinary_directory.0,
                &producer,
                ordinary_attempt,
                closure,
            ),
            Err(ProtectedPublicationError::NotPublished)
        ));
        assert_eq!(
            consume_compiler_module_handoff_v1(&ordinary_directory.0, &producer, ordinary_attempt,)
                .unwrap()
                .bytes(),
            ordinary_bytes,
        );

        let protected_directory = TestDirectory::new();
        let protected_attempt = begin_attempt(&protected_directory.0, &producer);
        let mut protected_receipt = exact_row_frontend_receipt_for_test();
        let mut invalid =
            prepare_row_softmax_v1_worker_handoff(protected_receipt.consume().unwrap()).unwrap();
        invalid.frontend_authority_commitment[0] ^= 1;
        assert!(matches!(
            publish_prepared_row_softmax_v1_worker_handoff_v2(
                &protected_directory.0,
                &producer,
                protected_attempt,
                closure,
                invalid,
            ),
            Err(WorkerV2ProducerError::MissingRowSoftmaxBindings)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(
                &protected_directory.0,
                &producer,
                protected_attempt,
            ),
            Err(PublicationError::NotPublished)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v2(
                &protected_directory.0,
                &producer,
                protected_attempt,
                closure,
            ),
            Err(ProtectedPublicationError::NotPublished)
        ));
    }

    #[test]
    fn row_publication_rejects_nonclosed_section_suffixes_before_publishing_attempt() {
        let producer = producer();
        let cases = [
            ("leading", vec![0, 1, 2, 3], true, false, false, 16),
            ("duplicate", vec![0, 1, 1, 2, 3], false, false, false, 16),
            ("reordered", vec![1, 0, 2, 3], false, false, false, 16),
            ("trailing", vec![0, 1, 2, 3], false, true, false, 16),
            ("truncated", vec![0, 1, 2, 3], false, false, true, 16),
            (
                "noncanonical chunks",
                vec![0, 1, 2, 3],
                false,
                false,
                false,
                8,
            ),
        ];

        for (name, order, leading, trailing, truncate, descriptor_width) in cases {
            let directory = TestDirectory::new();
            let attempt = begin_attempt(&directory.0, &producer);
            let mut receipt = exact_row_frontend_receipt_for_test();
            let prepared =
                prepare_row_softmax_v1_worker_handoff(receipt.consume().unwrap()).unwrap();
            let malformed = rebuild_prepared_row_sections(
                prepared,
                &order,
                leading,
                trailing,
                truncate,
                descriptor_width,
            );

            assert!(
                matches!(
                    publish_prepared_row_softmax_v1_worker_handoff(
                        &directory.0,
                        &producer,
                        attempt,
                        malformed,
                    ),
                    Err(WorkerV2ProducerError::MissingRowSoftmaxBindings)
                ),
                "publication accepted {name} row section suffix"
            );
            assert!(
                matches!(
                    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
                    Err(PublicationError::NotPublished)
                ),
                "publication consumed the {name} build attempt"
            );
        }
    }

    fn rebuild_prepared_row_sections(
        prepared: PreparedRowSoftmaxV1WorkerHandoffV1,
        order: &[usize],
        leading: bool,
        trailing: bool,
        truncate: bool,
        descriptor_width: usize,
    ) -> PreparedRowSoftmaxV1WorkerHandoffV1 {
        let PreparedRowSoftmaxV1WorkerHandoffV1 {
            authority_transcript,
            frontend_authority_commitment,
            exponential_boundary_commitment,
            handoff,
        } = prepared;
        let decoded = decode_row_softmax_compiler_sections_v1(handoff.module_bytes()).unwrap();
        let names = [
            fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1,
            ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1,
            ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1,
        ];
        let bytes = [
            decoded.descriptor(),
            decoded.authority_transcript(),
            decoded.authority().as_slice(),
            decoded.exponential_boundary().as_slice(),
        ];
        let descriptor_header = test_module_assembly_section_header(names[0]);
        let descriptor_position = handoff
            .module_bytes()
            .windows(descriptor_header.len())
            .position(|window| window == descriptor_header.as_bytes())
            .unwrap();
        let mut module = handoff.module_bytes()[..descriptor_position].to_vec();
        if leading {
            append_test_module_assembly_section(
                &mut module,
                ".fe2o3.unreviewed-leading.v1",
                &[0x44],
                16,
            );
        }
        for index in order.iter().copied() {
            let width = if index == 0 { descriptor_width } else { 16 };
            append_test_module_assembly_section(&mut module, names[index], bytes[index], width);
        }
        if trailing {
            append_test_module_assembly_section(
                &mut module,
                ".fe2o3.unreviewed-trailing.v1",
                &[0x55],
                16,
            );
        }
        if truncate {
            assert_eq!(module.pop(), Some(b'\n'));
        }

        let handoff = CompilerModuleHandoffV2::new(
            handoff.kind(),
            handoff.target(),
            handoff.code_object_version(),
            handoff.envelope().clone(),
            handoff.symbol_manifest().clone(),
            &module,
        )
        .unwrap();
        PreparedRowSoftmaxV1WorkerHandoffV1 {
            authority_transcript,
            frontend_authority_commitment,
            exponential_boundary_commitment,
            handoff,
        }
    }

    fn append_test_module_assembly_section(
        module: &mut Vec<u8>,
        section: &str,
        bytes: &[u8],
        chunk_width: usize,
    ) {
        module.extend_from_slice(test_module_assembly_section_header(section).as_bytes());
        module.extend_from_slice(b"module asm \".balign 8\"\n");
        for chunk in bytes.chunks(chunk_width) {
            module.extend_from_slice(b"module asm \".byte ");
            for (index, byte) in chunk.iter().enumerate() {
                if index != 0 {
                    module.extend_from_slice(b", ");
                }
                module.extend_from_slice(format!("0x{byte:02x}").as_bytes());
            }
            module.extend_from_slice(b"\"\n");
        }
    }

    fn test_module_assembly_section_header(section: &str) -> String {
        format!("module asm \".section {section},\\22\\22,@progbits\"\n")
    }

    fn producer() -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            "worker_v2_fixture",
            Some(Path::new("/workspace/worker-v2-fixture/src/lib.rs")),
        )
        .unwrap()
    }

    fn begin_attempt(directory: &Path, producer: &ProducerIdentity) -> BuildAttempt {
        begin_build_attempt(
            directory,
            producer,
            BuildInvocation::from_bytes([0x42; 32]),
            BuildSession::from_bytes([0x31; 16]),
        )
        .unwrap()
    }

    fn compiler_closure(pins: [[u8; 32]; 6]) -> CompilerClosureV2 {
        CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
    }

    fn compiler_closure_pins() -> [[u8; 32]; 6] {
        [
            [0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32], [0x55; 32], [0x66; 32],
        ]
    }

    fn owner(byte: u8, item: &str) -> CompilerFfiSourceOwnerV1 {
        CompilerFfiSourceOwnerV1::new(
            "worker_v2_fixture",
            &format!("worker_v2_fixture::{item}"),
            [byte; 16],
            &format!("_RINvNtCs1234_worker_v2_fixture{item}"),
        )
        .unwrap()
    }

    fn contract(
        direction: DeviceFfiDirectionV1,
        symbol: &str,
        abi: &str,
        effects: &str,
        semantic_byte: u8,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [semantic_byte; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: direction.tag(),
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: abi,
            effects,
            semantic_identity: &semantic_text,
        });
        CompilerFfiContractV1::new(
            id,
            direction,
            match direction {
                DeviceFfiDirectionV1::Import => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                DeviceFfiDirectionV1::Export => {
                    CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition
                }
            },
            target(),
            CodeObjectVersion::V5,
            owner(semantic_byte, symbol),
            symbol,
            abi,
            effects,
            semantic_identity,
        )
        .unwrap()
    }

    fn envelope() -> CompilerFfiEnvelopeV1 {
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 2).unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Import,
                "external_add",
                IMPORT_ABI,
                "read_global",
                0x11,
            ))
            .unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    fn returning_block() -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        block
    }

    fn complete_module() -> Module {
        let entry = Function::kernel_entry(
            "entry_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning_block()],
        );
        let mut export = Function::device_ffi_export(
            "rust_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning_block()],
        );
        export
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        let import = Function::declaration("external_add", Signature::new(vec![], vec![]));
        let mut kernel = Kernel::new(
            "entry",
            "entry_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(G1_WORKGROUP_X, 1, 1));

        let mut module = Module::new("tests::worker_v2_producer");
        module.functions = vec![entry, export, import];
        module.kernels.push(kernel);
        module
    }

    fn target_bound_module(binding: &str) -> Module {
        let mut module = complete_module();
        let capability = TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: binding.to_owned(),
        };
        module.required_capabilities.insert(capability.clone());
        module.functions[0].required_capabilities.insert(capability);
        module
    }

    #[test]
    fn exact_target_binding_matches_only_the_full_worker_envelope_target() {
        let exact = target_bound_module(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME);
        validate_exact_target_binding(target(), &exact).unwrap();

        for wrong in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx942:sramecc-:xnack-",
            "gfx950:xnack-",
        ] {
            let error =
                validate_exact_target_binding(DeviceTargetV1::parse(wrong).unwrap(), &exact)
                    .unwrap_err();
            assert!(matches!(
                error,
                WorkerV2ProducerError::TargetBindingMismatch { .. }
            ));
        }

        let unknown = target_bound_module("gfx942:xnack-:future+");
        assert!(matches!(
            validate_exact_target_binding(target(), &unknown),
            Err(WorkerV2ProducerError::TargetBindingMismatch { .. })
        ));

        let mut conflicting = exact;
        conflicting
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
                name: "gfx942:xnack+".to_owned(),
            });
        assert!(matches!(
            validate_exact_target_binding(target(), &conflicting),
            Err(WorkerV2ProducerError::TargetBindingMismatch { .. })
        ));
    }

    #[test]
    fn exact_target_binding_survives_worker_v2_handoff_authority() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let module = target_bound_module(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME);

        publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).unwrap();
        assert_eq!(
            handoff.target().to_string(),
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        );
        assert!(
            std::str::from_utf8(handoff.module_bytes())
                .unwrap()
                .contains("\"target-cpu\"=\"gfx942\"")
        );
    }

    #[test]
    fn configured_upstream_worker_v2_accepts_layout_bound_row_softmax_through_native_link() {
        const WORKER_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_WORKER";
        const WORKER_BUILD_ID_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_WORKER_BUILD_ID";
        const LLVM_BUILD_ID_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_LLVM_BUILD_ID";
        const WORKER_SHA256_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_WORKER_SHA256";
        const WORKER_LENGTH_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_WORKER_LENGTH";
        const CONFIGURATION: [&str; 5] = [
            WORKER_ENV,
            WORKER_BUILD_ID_ENV,
            LLVM_BUILD_ID_ENV,
            WORKER_SHA256_ENV,
            WORKER_LENGTH_ENV,
        ];
        let configured = CONFIGURATION
            .iter()
            .filter(|name| std::env::var_os(name).is_some())
            .count();
        let release_gate = row_softmax_release_gate_requested();
        if !require_row_softmax_release_configuration(release_gate, configured, &CONFIGURATION) {
            eprintln!(
                "skipping configured row-softmax Worker V2 execution: {} are absent",
                CONFIGURATION.join(", "),
            );
            return;
        }

        let worker_path = PathBuf::from(std::env::var_os(WORKER_ENV).unwrap());
        let worker_digest = parse_manifest_sha256(&std::env::var(WORKER_SHA256_ENV).unwrap());
        let worker_length = parse_manifest_length(&std::env::var(WORKER_LENGTH_ENV).unwrap());
        let measurement = WorkerMeasurementV1::new(
            ContentIdentityV1::from_parts(worker_digest, worker_length),
            std::env::var(WORKER_BUILD_ID_ENV).unwrap(),
            std::env::var(LLVM_BUILD_ID_ENV).unwrap(),
        )
        .expect("construct configured upstream worker measurement");
        assert_eq!(
            measurement.llvm_build_identity(),
            ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
        );
        let worker = PinnedWorkerV1::open(&worker_path, measurement)
            .expect("open measured upstream Worker V2 executable");

        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let mut receipt = exact_row_frontend_receipt_for_test();
        let prepared = prepare_row_softmax_v1_worker_handoff(receipt.consume().unwrap()).unwrap();
        let compiler_sections =
            decode_row_softmax_compiler_sections_v1(prepared.handoff().module_bytes()).unwrap();
        let descriptor_table =
            decode_device_descriptor_table_v1(compiler_sections.descriptor()).unwrap();
        let [descriptor_kernel] = descriptor_table.kernels() else {
            panic!("exact row-softmax compiler descriptor is not singular");
        };
        let descriptor_expectation = RowSoftmaxV1StructuralDescriptorExpectationV1::new(
            descriptor_kernel.kernel_id(),
            descriptor_kernel.source_evidence(),
            descriptor_kernel.executable_ir_evidence(),
        )
        .unwrap();
        publish_prepared_row_softmax_v1_worker_handoff(&directory.0, &producer, attempt, prepared)
            .expect("publish exact row-softmax producer handoff");
        let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
            .expect("consume exact row-softmax producer handoff");
        let options = [
            ("code-object-version", "6"),
            ("opt-level", "0"),
            ("strip-debug", "true"),
            ("verify-each", "true"),
        ]
        .into_iter()
        .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
        .collect();
        let evidence = execute_reproducible_first_build_worker_v2(
            consumed,
            &worker,
            Vec::new(),
            options,
            WorkerOutputConstraintsV1::new(fe2o3_hsaco::MAX_HSACO_BYTES as u64).unwrap(),
            WorkerExecutionLimitsV1::default(),
        )
        .expect("real upstream Worker V2 row-softmax OCML/native-link execution");

        const OCML_DIAGNOSTIC: &str = "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4";
        for execution in [evidence.bootstrap(), evidence.exact_replay()] {
            assert_eq!(execution.response().stage(), WorkerStageV1::Complete);
            assert!(
                execution
                    .response()
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic == OCML_DIAGNOSTIC),
                "completed Worker V2 execution omitted measured OCML evidence: {:?}",
                execution.response().diagnostics(),
            );
            assert!(
                execution.response().diagnostics().iter().any(|diagnostic| {
                    diagnostic == ROW_SOFTMAX_V1_WORKER_COMPLETE_DIAGNOSTIC_V1
                }),
                "completed Worker V2 execution omitted the exact row structural diagnostic: {:?}",
                execution.response().diagnostics(),
            );
        }
        let inspected = fe2o3_hsaco::inspect(evidence.output_bytes())
            .expect("inspect real row-softmax Worker V2 output");
        assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
        assert_eq!(
            inspected.code_object_version(),
            fe2o3_hsaco::CodeObjectVersion::V6,
        );
        if let Some(path) = std::env::var_os("FE2O3_TEST_RETAIN_ROW_SOFTMAX_HSACO") {
            fs::write(path, evidence.output_bytes()).expect("retain real row-softmax HSACO");
        }
        let structural =
            inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(evidence, descriptor_expectation)
                .expect("structurally admit exact real row-softmax Worker output");
        finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(structural)
            .expect("finalize exact real row-softmax Worker output");
    }

    #[test]
    fn publishes_exact_text_handoff_without_artifact_authority() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();

        let receipt = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &complete_module(),
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).unwrap();

        assert_eq!(receipt.attempt(), attempt);
        assert_eq!(receipt.identity(), consumed.identity());
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(handoff.target(), envelope.target());
        assert_eq!(
            handoff.code_object_version(),
            envelope.code_object_version()
        );
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(module_text.contains("define amdgpu_kernel void @entry"));
        assert!(module_text.contains("define void @rust_helper"));
        assert!(module_text.contains("declare void @external_add"));
        let manifest = handoff.symbol_manifest();
        assert_eq!(
            manifest
                .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
                .collect::<Vec<_>>(),
            ["entry"]
        );
        assert_eq!(
            manifest
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .collect::<Vec<_>>(),
            ["entry.kd"]
        );
        assert_eq!(
            manifest
                .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
                .collect::<Vec<_>>(),
            ["rust_helper"]
        );
        assert_eq!(
            manifest
                .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
                .collect::<Vec<_>>(),
            ["external_add"]
        );
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_compiler_authority());
        assert!(!consumed.grants_link_authority());
        assert!(!consumed.grants_load_authority());
        assert!(!consumed.grants_launch_authority());
    }

    #[test]
    fn rejects_missing_attempt_or_envelope_before_publication() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();

        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                None,
                Some(&envelope),
                &complete_module(),
            ),
            Err(WorkerV2ProducerError::MissingBuildAttempt)
        ));
        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                None,
                &complete_module(),
            ),
            Err(WorkerV2ProducerError::MissingCompilerFfiEnvelope)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn rejects_envelope_roles_missing_from_the_compiler_module() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .functions
            .retain(|function| function.id.as_str() != "rust_helper");

        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                Some(&envelope),
                &module,
            ),
            Err(WorkerV2ProducerError::MissingCompilerDefinition(symbol))
                if symbol == "rust_helper"
        ));
        let mut module = complete_module();
        module
            .functions
            .retain(|function| function.id.as_str() != "external_add");
        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                Some(&envelope),
                &module,
            ),
            Err(WorkerV2ProducerError::MissingExternalDeclaration(symbol))
                if symbol == "external_add"
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn binds_missing_and_accepts_exact_g1_workgroup_sizes() {
        let explicit = complete_module();
        assert_eq!(
            bind_g1_launch_contract(&explicit).unwrap().kernels[0].workgroup_size,
            Some(WorkgroupSize::new(G1_WORKGROUP_X, 1, 1))
        );

        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module.kernels[0].workgroup_size = None;

        publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).unwrap();
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(text.contains("\"amdgpu-flat-work-group-size\"=\"256,256\""));
        assert!(text.contains("!reqd_work_group_size"));
    }

    #[test]
    fn rejects_a_conflicting_workgroup_size_without_publishing() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

        let error = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2ProducerError::ConflictingWorkgroupSize {
                kernel,
                declared: WorkgroupSize { x: 64, y: 1, z: 1 },
                required: WorkgroupSize { x: 256, y: 1, z: 1 },
            } if kernel == "entry"
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn binds_the_target_default_wave_mode_for_a_standalone_export() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "rust_helper")
            .unwrap()
            .required_capabilities
            .clear();

        publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).unwrap();
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(text.contains("-wavefrontsize32,+wavefrontsize64"));
    }

    #[test]
    fn rejects_a_wave_mode_unsupported_by_the_exact_target() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave32,
            ));

        let error = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2ProducerError::UnsupportedWaveMode {
                width: WaveWidth::Wave32,
                ..
            }
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }
}
