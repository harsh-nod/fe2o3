//! Preparation and attempt-scoped publication of inert Worker V2 compiler modules.

use crate::collected_row_softmax_v1::AuthenticatedRowSoftmaxModuleV1;
use crate::collected_scalar_gemm_v1::AuthenticatedScalarGemmModuleV1;
use crate::collected_tiled_gemm_v1::AuthenticatedTiledGemmModuleV1;
use crate::compiler_descriptor::{
    CompilerDescriptorError, TypedDescriptorRootV1, construct_compiler_descriptor_source_v1,
};
use crate::kernel_ir_codegen::{
    CompilerModuleConstructionError, InertCompilerModuleTextV1, bind_compiler_descriptor_source_v1,
    bind_source_debug_metadata_v1, construct_inert_compiler_module_text_for_target_v1,
};
use fe2o3_amd_target::{CapabilityDerivationError, WavefrontWidth};
use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV1 as HandoffPublicationErrorV1,
    CompilerModuleHandoffReceiptV1, ProducerIdentity, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
    CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestErrorV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    Module, SCALAR_GEMM_V1_KERNEL_ID, TargetCapability, WaveWidth, WorkgroupSize,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::Path;

const G1_WORKGROUP_X: u32 = 256;
const SCALAR_GEMM_V1_DESCRIPTOR: &str = "scalar_gemm_v1.kd";
const TILED_GEMM_V1_DESCRIPTOR: &str = "tiled_gemm_v1.kd";
const ROW_SOFTMAX_V1_DESCRIPTOR: &str = "row_softmax_v1.kd";
pub(crate) const ROW_SOFTMAX_OCML_EXP_SYMBOL_V1: &str = "__ocml_exp_f32";
const ROW_SOFTMAX_OCML_EXP_ABI_V1: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";
const ROW_SOFTMAX_OCML_EXP_EFFECTS_V1: &str = "none";

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
    let authority_hex = hex(&frontend_authority_commitment);
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
    let authority_hex = hex(&frontend_authority_commitment);
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

pub(crate) fn publish_prepared_row_softmax_v1_worker_handoff(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    prepared: PreparedRowSoftmaxV1WorkerHandoffV1,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let PreparedRowSoftmaxV1WorkerHandoffV1 {
        authority_transcript,
        frontend_authority_commitment,
        exponential_boundary_commitment,
        handoff,
    } = prepared;
    let module = std::str::from_utf8(handoff.module_bytes())
        .map_err(|_| WorkerV2ProducerError::MissingRowSoftmaxBindings)?;
    let authority_section =
        module_asm_commitment_section(".fe2o3.row-softmax-auth.v1", &frontend_authority_commitment);
    let transcript_section = module_asm_commitment_section(
        ".fe2o3.row-softmax-authority-transcript.v1",
        &authority_transcript,
    );
    let exponential_section =
        module_asm_commitment_section(".fe2o3.row-exp.v1", &exponential_boundary_commitment);
    if !module.contains(&transcript_section)
        || !module.contains(&authority_section)
        || !module.contains(&exponential_section)
    {
        return Err(WorkerV2ProducerError::MissingRowSoftmaxBindings);
    }
    let receipt = publish_compiler_module_handoff_v1(
        output_dir,
        producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .map_err(WorkerV2ProducerError::Publication)?;
    eprintln!(
        "[rustc-codegen-fe2o3] published row-softmax Worker V2 handoff bound to frontend authority {} and exponential boundary {}",
        hex(&frontend_authority_commitment),
        hex(&exponential_boundary_commitment),
    );
    Ok(receipt)
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

/// Constructs the exact compiler-owned OCML import observation retained by
/// both descriptor evidence and the Worker V2 handoff.
pub(crate) fn construct_row_softmax_v1_compiler_envelope(
    exponential_boundary_commitment: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed row-softmax target is valid");
    let semantic_text = hex(&exponential_boundary_commitment);
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
            hex(semantic.identity_sha256()),
            hex(semantic.portable_mir_sha256()),
        );
        eprintln!(
            "[rustc-codegen-fe2o3] S09 BuildIdentityClaimV2: schema=fe2o3-s09-build-identity-claim-v2; identity_sha256={}; cargo_metadata_sha256={}; prepared_rustc_command_sha256={}; cargo_fe2o3_executable_sha256={}; declared_cargo_executable_sha256={}; pinned_cargo_image_sha256={}; observed_parent_pid={}; observed_parent_start_time_ticks={}; observed_def_path={}; observed_symbol={}",
            hex(observation.identity_sha256()),
            hex(observation.cargo_metadata_sha256()),
            hex(observation.prepared_rustc_command_sha256()),
            hex(observation.cargo_fe2o3_executable_sha256()),
            hex(observation.declared_cargo_executable_sha256()),
            hex(observation.pinned_cargo_image_sha256()),
            observation.observed_parent_pid(),
            observation.observed_parent_start_time_ticks(),
            observation.observed_def_path(),
            observation.observed_symbol(),
        );
    }

    publish_compiler_module_handoff_v1(output_dir, producer, attempt, handoff.canonical_bytes())
        .map_err(WorkerV2ProducerError::Publication)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
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
    MissingScalarFrontendAuthority,
    MissingTiledFrontendAuthority,
    MissingRowSoftmaxBindings,
    RowSoftmaxClosureMismatch,
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
            Self::MissingScalarFrontendAuthority => formatter.write_str(
                "scalar GEMM compiler-module handoff lost its embedded frontend authority",
            ),
            Self::MissingTiledFrontendAuthority => formatter.write_str(
                "tiled GEMM compiler-module handoff lost its embedded frontend authority",
            ),
            Self::MissingRowSoftmaxBindings => formatter.write_str(
                "row-softmax compiler-module handoff lost its frontend or exponential-boundary binding",
            ),
            Self::RowSoftmaxClosureMismatch => formatter.write_str(
                "row-softmax compiler-module symbol closure is not exactly one kernel, one descriptor, and the OCML exp import",
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
            Self::MissingBuildAttempt
            | Self::MissingCompilerFfiEnvelope
            | Self::MissingScalarFrontendAuthority
            | Self::MissingTiledFrontendAuthority
            | Self::MissingRowSoftmaxBindings
            | Self::RowSoftmaxClosureMismatch
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
    use crate::collected_row_softmax_v1::{
        exact_authority_policy_for_test,
        exact_frontend_receipt_for_test as exact_row_frontend_receipt_for_test,
    };
    use crate::collected_scalar_gemm_v1::exact_frontend_receipt_for_test;
    use crate::collected_tiled_gemm_v1::exact_frontend_receipt_for_test as exact_tiled_frontend_receipt_for_test;
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, CompilerModuleHandoffErrorV1 as PublicationError,
        begin_build_attempt, consume_compiler_module_handoff_v1,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
        CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1, CompilerModuleHandoffV2, DeviceTargetV1,
    };
    use fe2o3_hsaco_finalize::{
        ContentIdentityV1, RowSoftmaxV1DirectWorkerExpectationV1, RowSoftmaxV1DirectWorkerPinsV1,
        RowSoftmaxV1OcmlProviderPinsV1,
    };
    use fe2o3_kernel_ir::ScalarGemmTargetRequirementsV1;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature,
        TargetCapability, Terminator, WaveWidth, WorkgroupSize,
    };
    use reserved_fe2o3_symbols::{
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };
    use sha2::{Digest as _, Sha256};
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
    fn consumed_row_receipt_prepares_exact_llvm_descriptor_and_ocml_import_closure() {
        let mut receipt = exact_row_frontend_receipt_for_test();
        let authenticated = receipt.consume().expect("consume exact row receipt");
        let expected_authority = *authenticated.authority_commitment();
        let expected_exponential = *authenticated.exponential_boundary_commitment();
        let prepared = prepare_row_softmax_v1_worker_handoff(authenticated)
            .expect("prepare exact row-softmax handoff");
        let handoff = prepared.handoff();
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();

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
            "upstream-llvm-production-compatibility-test",
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
