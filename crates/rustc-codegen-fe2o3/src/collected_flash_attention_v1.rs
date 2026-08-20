//! Exact-source authentication for the masked FlashAttention V1 profile.
//!
//! This layer authenticates the attributed source bytes (including their
//! Phase A fallback namespace), the distinct wrapper/session-derived
//! registration binding, rustc ABI, and complete reachable portable-MIR
//! closure before selecting the closed semantic profile in `fe2o3-kernel-ir`.
//! The selection is reviewed correspondence, not a compiler-refinement proof.
//! No generic IR is silently substituted and no Worker V2, LLVM, link,
//! artifact, or execution authority is produced here.

use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Dimensions, Endianness, IdentityText, LaunchContract, Mutability as ArtifactMutability,
    PointerWidth, RustScalarElementTypeV1, TargetIdentity,
};
use fe2o3_kernel_ir::{
    FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES, FLASH_ATTENTION_V1_KERNEL_ID,
    FLASH_ATTENTION_V1_NAMESPACE, FLASH_ATTENTION_V1_SOURCE_SHA256, FlashAttentionKernelIrV1,
    FlashAttentionProfileV1, flash_attention_v1_kernel_ir, verify_flash_attention_v1,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{FloatTy, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectionResult, TypedKernelProfile};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem};

pub(crate) const COLLECTED_FLASH_ATTENTION_PIPELINE_V1: &str = "collected-flash-attention-v1";
pub(crate) const EXACT_FLASH_ATTENTION_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const FLASH_ATTENTION_CODE_OBJECT_VERSION_V1: u16 = 6;

const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_NAME: &str = "fe2o3_collected_flash_attention_v1_fixture";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-flash-attention-v1-reviewed";
// The ordinary macro wrapper overrides the source fallback namespace with the
// binding derived from this exact crate name and ordered metadata. Authority
// commits both identities so the override is visible and cannot substitute
// either the public source bytes or the compiler session.
const REVIEWED_COMPILER_CRATE_BINDING: &str =
    "8b7c5dabd2bbc2855b328b84aa387119d8caae550aa6798779461ee3bed0bfc8";
const SOURCE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace/flash-attention-v1.rs";
const WORKSPACE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace";
const REVIEWED_ROOT_INSTANCE_IDENTITY: &str = "kernel::__fe2o3_host_kernel_v1_4cd011e31086168adc65ef2b706d5c0df66642392c149412d11e42edc718e291";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.flash-attention.compiler-semantics.v1";
const TRUSTED_DEFINITIONS_DOMAIN_V4: &[u8] =
    b"fe2o3.flash-attention.trusted-definitions-and-terminals.v4";
const AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.flash-attention.source-authority.v1";
const FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3.flash-attention.rustc-fn-abi.v1";
const ABI_BINDING_V1: &[u8] = b"ptr64;size=64;align=8;q@0:16:8:slice-f32:shared-readonly;k@16:16:8:slice-f32:shared-readonly;v@32:16:8:slice-f32:shared-readonly;output@48:16:8:slice-f32:exclusive-readwrite";
const EFFECT_BINDING_V1: &[u8] = b"q,k,v:read-only-distinct-or-aliasing;output:exclusive-lane-owned-adjacent-pair;lane-l-owns-o[2l],o[2l+1];all-64-physical-lanes;trap-before-owned-writes-on-invalid-shape-or-nonfinite-intermediate";
const SOURCE_LAUNCH_BINDING_V1: &[u8] =
    b"rank=1;block=exact(64,1,1);max-grid=(4294967295,1,1);static-shared=0;dynamic-shared=0";
const PROFILE_LAUNCH_BINDING_V1: &[u8] =
    b"target=gfx942:xnack-;cov=6;wave=64;block=exact(64,1,1);grid=exact(1,1,1)";
const NUMERICAL_BINDING_V1: &[u8] = b"b=1;h=1;n=8;d=16;inputs=f32-finite;dot=strict-sequential-f32-d16;scale-bits=0x3e800000;mask=causal-lower-triangle-diagonal-included;online=max,sum,numerator-pair;ordered-rescale;no-contraction;divide-at-end";
const DESCRIPTOR_BINDING_V1: &[u8] = b"logical=flash_attention_causal_f32_b1_h1_n8_d16_v1;export=flash_attention_causal_f32_b1_h1_n8_d16_v1;descriptor=flash_attention_causal_f32_b1_h1_n8_d16_v1.kd;explicit-kernarg=64;complete-cov6-kernarg=320;wg=64,1,1;wave=64;static-lds=0;dynamic-lds=0";
const CANONICAL_IR_BINDING_V1: &[u8] = b"fe2o3::flash_attention_causal_f32_v1;args=q,k,v-shared-f32x128,output-lane-owned-f32x128;shape=b1,h1,n8,d16;causal=key<=query;ordered-recurrence=dot,scale,init,next-max,previous-exp,current-exp,denominator,numerator-pair,maximum,divide;ownership=adjacent-pair-total-injective-in-bounds";
const CORRESPONDENCE_BINDING_V1: &[u8] = b"exact attributed source plus wrapper/session registration, exact rustc FnAbi, location-independent V4 provider-semantic definitions, identity-bound reviewed semantic terminals, and complete reachable portable-MIR modulo those terminals select a closed FlashAttention semantic sidecar;reviewed correspondence only;not generic lowering, terminal-body refinement, or a compiler-refinement proof";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

// Filled from the pinned compiler fixture after path-independent portable-MIR
// import. Any reachable body, call target, type, or operation drift changes it.
const PORTABLE_MIR_CLOSURE_IDENTITY_V1: [u8; 32] = [
    0x39, 0xdd, 0x09, 0x83, 0x2a, 0x49, 0x72, 0xb4, 0xa3, 0xa1, 0x12, 0xa8, 0x75, 0x4d, 0xb3, 0xbe,
    0x59, 0x5d, 0x11, 0x76, 0xc0, 0x12, 0x2e, 0x1f, 0x0b, 0x66, 0xd0, 0x24, 0xdf, 0x35, 0x35, 0x91,
];
const RUSTC_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0x2c, 0x80, 0x3c, 0x84, 0xc1, 0x7a, 0x11, 0xc8, 0x34, 0xba, 0xe4, 0x53, 0x66, 0x9c, 0x09, 0xe1,
    0xa1, 0x14, 0xe7, 0x8f, 0x25, 0x43, 0x29, 0x4c, 0xaa, 0x7f, 0x15, 0x84, 0xb1, 0xb4, 0x86, 0xf2,
];
const COMPILER_SEMANTICS_IDENTITY_V1: [u8; 32] = [
    0xb9, 0x25, 0x15, 0xfa, 0x53, 0x47, 0xd9, 0x3e, 0xe9, 0x63, 0x88, 0xda, 0x9e, 0x72, 0x76, 0xaa,
    0x96, 0xcd, 0x30, 0x3e, 0x66, 0x4c, 0xa6, 0x75, 0x3b, 0x9b, 0xbd, 0x23, 0xd9, 0x1f, 0x44, 0x3b,
];
const TRUSTED_TERMINAL_IDENTITY_V4: [u8; 32] = [
    0x81, 0x56, 0x90, 0x42, 0xbe, 0x7e, 0x43, 0xc6, 0xa6, 0xbb, 0xbb, 0x12, 0xf5, 0x01, 0xfe, 0x14,
    0x13, 0xf1, 0x6a, 0x75, 0x42, 0x87, 0x53, 0x6d, 0xe9, 0xfa, 0x36, 0x2c, 0xd5, 0x8b, 0x3d, 0x6d,
];

const ARGUMENT_KINDS_V1: [GeneralTypedArgumentKindV3; 4] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];

const REQUIRED_TRUSTED_ITEMS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::DisjointSlice,
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::ContextFromCompiler),
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::F32(
        fe2o3_kernel_ir::F32MathFunction::Exp,
    )),
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];

const REVIEWED_SEMANTIC_TERMINALS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::ContextFromCompiler),
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::F32(
        fe2o3_kernel_ir::F32MathFunction::Exp,
    )),
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum FlashAttentionCompilerIntrinsicV1 {
    FabsF32,
}

impl FlashAttentionCompilerIntrinsicV1 {
    pub(crate) const fn canonical_path(self) -> &'static str {
        match self {
            Self::FabsF32 => "core::intrinsics::fabs::<f32>",
        }
    }
}

pub(crate) fn classify_exact_flash_attention_compiler_intrinsic(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::DefId,
) -> Option<FlashAttentionCompilerIntrinsicV1> {
    if std::env::var("FE2O3_CODEGEN_PIPELINE").as_deref()
        != Ok(COLLECTED_FLASH_ATTENTION_PIPELINE_V1)
        || def_id.is_local()
    {
        return None;
    }
    tcx.def_path_str(def_id)
        .ends_with("::intrinsics::fabs")
        .then_some(FlashAttentionCompilerIntrinsicV1::FabsF32)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerSemanticsV1 {
    rustc_release: &'static str,
    rustc_commit: &'static str,
    llvm_version: &'static str,
    panic_strategy: String,
    overflow_checks: bool,
    optimize: String,
    debug_assertions: bool,
    mir_opt_level: usize,
    mir_enable_passes: Vec<(String, bool)>,
    llvm_args: Vec<String>,
    llvm_passes: Vec<String>,
    target_cpu: Option<String>,
    target_features: String,
    rustc_codegen_opt_level: String,
    crate_name: String,
    crate_metadata: Vec<String>,
    remap_path_destinations: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct FlashAttentionAuthorityV1 {
    source_identity: [u8; 32],
    source_namespace: [u8; 32],
    compiler_crate_binding: [u8; 32],
    target: String,
    code_object_version: u16,
    kernel_export: String,
    root_instance_identity: String,
    portable_mir_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    frontend_contract_identity: [u8; 32],
    abi_identity: [u8; 32],
    effects_identity: [u8; 32],
    source_launch_identity: [u8; 32],
    profile_launch_identity: [u8; 32],
    numerical_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    correspondence_identity: [u8; 32],
    authority_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FlashAttentionFrontendReceiptV1 {
    authority: Option<FlashAttentionAuthorityV1>,
    ir: Option<FlashAttentionKernelIrV1>,
    profile: Option<FlashAttentionProfileV1>,
}

impl FlashAttentionFrontendReceiptV1 {
    fn authority(&self) -> &FlashAttentionAuthorityV1 {
        self.authority
            .as_ref()
            .expect("unconsumed FlashAttention receipt")
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_hex(&self) -> String {
        crate::encode_hex(&self.authority().portable_mir_identity)
    }

    pub(crate) fn authority_hex(&self) -> String {
        crate::encode_hex(&self.authority().authority_identity)
    }

    pub(crate) fn authority_commitment(&self) -> &[u8; 32] {
        &self.authority().authority_identity
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedFlashAttentionV1, CollectedFlashAttentionErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedFlashAttentionErrorV1::ReceiptAlreadyConsumed)?;
        let ir = self
            .ir
            .take()
            .ok_or(CollectedFlashAttentionErrorV1::ReceiptAlreadyConsumed)?;
        let profile = self
            .profile
            .take()
            .ok_or(CollectedFlashAttentionErrorV1::ReceiptAlreadyConsumed)?;
        validate_authority(&authority)?;
        verify_flash_attention_v1(&ir, &profile)
            .map_err(|error| CollectedFlashAttentionErrorV1::CanonicalIr(error.to_string()))?;
        let authority_transcript = authority_transcript(&authority);
        if sha256(&authority_transcript) != authority.authority_identity {
            return Err(CollectedFlashAttentionErrorV1::ReceiptBinding(
                "authority transcript",
            ));
        }
        Ok(AuthenticatedFlashAttentionV1 {
            ir,
            profile,
            authority_transcript,
            source_authority_identity: authority.authority_identity,
            descriptor_identity: authority.descriptor_identity,
            source_identity: authority.source_identity,
            source_namespace: authority.source_namespace,
            compiler_crate_binding: authority.compiler_crate_binding,
            portable_mir_identity: authority.portable_mir_identity,
            compiler_semantics_identity: authority.compiler_semantics_identity,
            fn_abi_identity: authority.fn_abi_identity,
            trusted_definitions_identity: authority.trusted_definitions_identity,
            abi_identity: authority.abi_identity,
            effects_identity: authority.effects_identity,
            numerical_identity: authority.numerical_identity,
            canonical_ir_identity: authority.canonical_ir_identity,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFlashAttentionV1 {
    ir: FlashAttentionKernelIrV1,
    profile: FlashAttentionProfileV1,
    authority_transcript: Vec<u8>,
    source_authority_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    source_identity: [u8; 32],
    source_namespace: [u8; 32],
    compiler_crate_binding: [u8; 32],
    portable_mir_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    abi_identity: [u8; 32],
    effects_identity: [u8; 32],
    numerical_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
}

impl AuthenticatedFlashAttentionV1 {
    pub(crate) fn semantic_summary(&self) -> (u8, u8, u8, u8, usize) {
        (
            self.ir.shape.batches,
            self.ir.shape.heads,
            self.ir.shape.sequence_length,
            self.ir.shape.head_dimension,
            self.ir.recurrence.len(),
        )
    }

    pub(crate) fn profile(&self) -> &FlashAttentionProfileV1 {
        &self.profile
    }

    pub(crate) fn descriptor_hex(&self) -> String {
        crate::encode_hex(&self.descriptor_identity)
    }

    pub(crate) fn into_finalization_inputs(self) -> FlashAttentionFinalizationInputsV1 {
        FlashAttentionFinalizationInputsV1 {
            ir: self.ir,
            profile: self.profile,
            authority_transcript: self.authority_transcript,
            source_authority_identity: self.source_authority_identity,
            descriptor_identity: self.descriptor_identity,
            source_identity: self.source_identity,
            source_namespace: self.source_namespace,
            compiler_crate_binding: self.compiler_crate_binding,
            portable_mir_identity: self.portable_mir_identity,
            compiler_semantics_identity: self.compiler_semantics_identity,
            fn_abi_identity: self.fn_abi_identity,
            trusted_definitions_identity: self.trusted_definitions_identity,
            abi_identity: self.abi_identity,
            effects_identity: self.effects_identity,
            numerical_identity: self.numerical_identity,
            canonical_ir_identity: self.canonical_ir_identity,
        }
    }
}

/// Linear compiler input derived only from the consumed exact-source receipt.
/// It is inert and grants no LLVM, link, artifact, runtime, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FlashAttentionFinalizationInputsV1 {
    pub(crate) ir: FlashAttentionKernelIrV1,
    pub(crate) profile: FlashAttentionProfileV1,
    pub(crate) authority_transcript: Vec<u8>,
    pub(crate) source_authority_identity: [u8; 32],
    pub(crate) descriptor_identity: [u8; 32],
    pub(crate) source_identity: [u8; 32],
    pub(crate) source_namespace: [u8; 32],
    pub(crate) compiler_crate_binding: [u8; 32],
    pub(crate) portable_mir_identity: [u8; 32],
    pub(crate) compiler_semantics_identity: [u8; 32],
    pub(crate) fn_abi_identity: [u8; 32],
    pub(crate) trusted_definitions_identity: [u8; 32],
    pub(crate) abi_identity: [u8; 32],
    pub(crate) effects_identity: [u8; 32],
    pub(crate) numerical_identity: [u8; 32],
    pub(crate) canonical_ir_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CollectedFlashAttentionErrorV1 {
    Admission(String),
    SourceIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    Abi(String),
    Layout(String),
    PortableMir(String),
    PortableMirIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    FnAbiIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    TrustedDefinitions(String),
    CanonicalIr(String),
    ReceiptAlreadyConsumed,
    ReceiptBinding(&'static str),
}

impl fmt::Display for CollectedFlashAttentionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(detail) => {
                write!(formatter, "FlashAttention admission failed: {detail}")
            }
            Self::SourceIdentity { expected, actual } => write!(
                formatter,
                "FlashAttention source bytes mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::Abi(detail) => write!(formatter, "FlashAttention ABI mismatch: {detail}"),
            Self::Layout(detail) => {
                write!(formatter, "FlashAttention layout mismatch: {detail}")
            }
            Self::PortableMir(detail) => {
                write!(formatter, "FlashAttention portable MIR rejected: {detail}")
            }
            Self::PortableMirIdentity { expected, actual } => write!(
                formatter,
                "FlashAttention complete reachable MIR closure mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::FnAbiIdentity { expected, actual } => write!(
                formatter,
                "FlashAttention rustc FnAbi mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::TrustedDefinitions(detail) => write!(
                formatter,
                "FlashAttention trusted definition closure rejected: {detail}"
            ),
            Self::CanonicalIr(detail) => {
                write!(
                    formatter,
                    "FlashAttention canonical semantic IR rejected: {detail}"
                )
            }
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("FlashAttention frontend receipt was already consumed")
            }
            Self::ReceiptBinding(field) => write!(
                formatter,
                "FlashAttention frontend receipt binding mismatch: {field}"
            ),
        }
    }
}

impl Error for CollectedFlashAttentionErrorV1 {}

pub(crate) fn authenticate_collected_flash_attention_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<FlashAttentionFrontendReceiptV1, CollectedFlashAttentionErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics_identity = require_compiler_semantics(&observe_compiler_semantics(tcx))?;
    let root = exact_root(&collection.functions)?;
    require_registration(root)?;
    let source_identity = observe_source_identity(tcx, root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi_identity = require_fn_abi(tcx, root.instance)?;
    let trusted_definitions_identity = trusted_definitions_and_terminals_identity(tcx, collection)?;
    let target_identity = exact_target_identity()?;
    let profile_launch = exact_profile_launch()?;
    let contract = root
        .general_typed_contract
        .as_ref()
        .expect("layout checked contract");
    let imported = crate::mir_import::import_collection(tcx, collection)
        .map_err(|error| CollectedFlashAttentionErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            FLASH_ATTENTION_V1_KERNEL_ID,
            &target_identity,
            contract.abi(),
            &profile_launch,
        ))
        .map_err(|error| CollectedFlashAttentionErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = *portable_mir_identity.as_bytes();
    if portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
        return Err(CollectedFlashAttentionErrorV1::PortableMirIdentity {
            expected: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
            actual: portable_mir_identity,
        });
    }
    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
        return Err(CollectedFlashAttentionErrorV1::Admission(format!(
            "root instance has noncanonical generated identity `{root_instance_identity}`"
        )));
    }
    let ir = flash_attention_v1_kernel_ir();
    let profile = FlashAttentionProfileV1::exact_gfx942_xnack_minus_cov6();
    verify_flash_attention_v1(&ir, &profile)
        .map_err(|error| CollectedFlashAttentionErrorV1::CanonicalIr(error.to_string()))?;
    let frontend_contract_identity = sha256(
        root.frontend_contract
            .as_ref()
            .expect("registration checked frontend contract")
            .canonical_bytes(),
    );
    let mut authority = FlashAttentionAuthorityV1 {
        source_identity,
        source_namespace: FLASH_ATTENTION_V1_NAMESPACE,
        compiler_crate_binding: compiler_crate_binding().as_bytes(),
        target: target.as_str().to_owned(),
        code_object_version: FLASH_ATTENTION_CODE_OBJECT_VERSION_V1,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_identity,
        compiler_semantics_identity,
        fn_abi_identity,
        trusted_definitions_identity,
        frontend_contract_identity,
        abi_identity: sha256(ABI_BINDING_V1),
        effects_identity: sha256(EFFECT_BINDING_V1),
        source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
        profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
        numerical_identity: sha256(NUMERICAL_BINDING_V1),
        descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
        canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
        authority_identity: [0; 32],
    };
    authority.authority_identity = authority_identity(&authority);
    Ok(FlashAttentionFrontendReceiptV1 {
        authority: Some(authority),
        ir: Some(ir),
        profile: Some(profile),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedFlashAttentionErrorV1> {
    if target != EXACT_FLASH_ATTENTION_TARGET_V1 {
        return Err(CollectedFlashAttentionErrorV1::Admission(format!(
            "requires exact target `{EXACT_FLASH_ATTENTION_TARGET_V1}`, found `{target}`"
        )));
    }
    if custom_llvm_pipeline {
        return Err(CollectedFlashAttentionErrorV1::Admission(
            "custom LLVM arguments or passes are forbidden".into(),
        ));
    }
    Ok(())
}

fn exact_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedFlashAttentionErrorV1> {
    let mut roots = functions
        .iter()
        .filter(|function| function.is_kernel_entry());
    let root = roots.next().ok_or_else(|| {
        CollectedFlashAttentionErrorV1::Admission(
            "the exact FlashAttention closure has no kernel root".into(),
        )
    })?;
    if roots.next().is_some() || functions.len() != 9 {
        return Err(CollectedFlashAttentionErrorV1::Admission(format!(
            "the exact FlashAttention closure requires one root plus eight reachable local/core helpers, found {} collected functions",
            functions.len()
        )));
    }
    Ok(root)
}

fn require_registration(
    root: &CollectedFunction<'_>,
) -> Result<(), CollectedFlashAttentionErrorV1> {
    let namespace = compiler_crate_binding();
    let expected_binding = derive_kernel_binding_id_v1(
        namespace,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        FLASH_ATTENTION_V1_KERNEL_ID,
        FLASH_ATTENTION_V1_KERNEL_ID,
    );
    if root.export_name != FLASH_ATTENTION_V1_KERNEL_ID
        || root.logical_name.as_deref() != Some(FLASH_ATTENTION_V1_KERNEL_ID)
        || !matches!(
            root.typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        )
        || root.kernel_binding != Some(expected_binding)
        || root
            .frontend_contract
            .as_ref()
            .map(|value| value.canonical_bytes())
            != Some(EXACT_FRONTEND_CONTRACT_V1)
    {
        return Err(CollectedFlashAttentionErrorV1::Admission(
            "expected the unique ordinary #[kernel(typed)] FlashAttention root with the reviewed wrapper-derived crate binding and required=max=64x1x1 contract".into(),
        ));
    }
    Ok(())
}

fn observe_source_identity(
    tcx: TyCtxt<'_>,
    root: &CollectedFunction<'_>,
) -> Result<[u8; 32], CollectedFlashAttentionErrorV1> {
    let file_name = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(root.instance.def_id()))
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let bytes = std::fs::read(&file_name).map_err(|error| {
        CollectedFlashAttentionErrorV1::Admission(format!(
            "source file `{file_name}` is unavailable for exact-byte authentication: {error}"
        ))
    })?;
    let namespace_declaration = format!(
        "namespace = \"{}\"",
        crate::encode_hex(&FLASH_ATTENTION_V1_NAMESPACE)
    );
    if bytes
        .windows(namespace_declaration.len())
        .filter(|window| *window == namespace_declaration.as_bytes())
        .count()
        != 1
    {
        return Err(CollectedFlashAttentionErrorV1::Admission(
            "exact source must contain the unique reviewed Phase A namespace declaration".into(),
        ));
    }
    let actual = sha256(&bytes);
    if actual != FLASH_ATTENTION_V1_SOURCE_SHA256 {
        return Err(CollectedFlashAttentionErrorV1::SourceIdentity {
            expected: FLASH_ATTENTION_V1_SOURCE_SHA256,
            actual,
        });
    }
    Ok(actual)
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<(), CollectedFlashAttentionErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(CollectedFlashAttentionErrorV1::Abi(
            "kernel must be one nongeneric ordinary function item".into(),
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| {
            CollectedFlashAttentionErrorV1::Abi("signature normalization failed".into())
        })?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 4
        || !is_shared_f32_slice(signature.inputs()[0])
        || !is_shared_f32_slice(signature.inputs()[1])
        || !is_shared_f32_slice(signature.inputs()[2])
        || !is_disjoint_f32_slice(tcx, signature.inputs()[3])
    {
        return Err(CollectedFlashAttentionErrorV1::Abi(format!(
            "expected safe Rust `(&[f32], &[f32], &[f32], DisjointSlice<f32>) -> ()`, found `{signature}`"
        )));
    }
    Ok(())
}

fn is_shared_f32_slice(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Float(FloatTy::F32)))
    )
}

fn is_disjoint_f32_slice(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, args) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::DisjointSlice)
        && args.len() == 2
        && args
            .first()
            .and_then(|value| value.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Float(FloatTy::F32)))
}

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedFlashAttentionErrorV1> {
    let contract = root.general_typed_contract.as_ref().ok_or_else(|| {
        CollectedFlashAttentionErrorV1::Layout("General V3 contract is absent".into())
    })?;
    let actual = contract
        .arguments()
        .iter()
        .map(|value| value.kind())
        .collect::<Vec<_>>();
    if actual != ARGUMENT_KINDS_V1 {
        return Err(CollectedFlashAttentionErrorV1::Layout(format!(
            "expected argument kinds {ARGUMENT_KINDS_V1:?}, found {actual:?}"
        )));
    }
    if root
        .typed_layout_identities
        .as_ref()
        .map(|identities| identities.len())
        != Some(4)
    {
        return Err(CollectedFlashAttentionErrorV1::Layout(
            "four compiler-derived argument identities are required".into(),
        ));
    }
    let abi = contract.abi();
    if abi.size() != u64::from(FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES)
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 4
    {
        return Err(CollectedFlashAttentionErrorV1::Layout(format!(
            "expected ptr64 size-64 align-8 four-field ABI, found {abi:?}"
        )));
    }
    // General V3 canonicalizes physical ABI fields positionally; the semantic
    // role names remain bound by `ABI_BINDING_V1` and the closed Kernel IR.
    let names = ["arg0", "arg1", "arg2", "arg3"];
    let offsets = [0, 16, 32, 48];
    let sizes = [16, 16, 16, 16];
    let alignments = [8, 8, 8, 8];
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != names[index]
            || field.offset() != offsets[index]
            || field.size() != sizes[index]
            || field.alignment() != alignments[index]
            || field.type_identity() != contract.arguments()[index].type_identity()
        {
            return Err(CollectedFlashAttentionErrorV1::Layout(format!(
                "ABI field {index} name, offset, size, alignment, or type identity drifted: field={field:?}, argument={:?}",
                contract.arguments()[index]
            )));
        }
        let exact = match index {
            0..=2 => {
                matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4
                    }
                ) && field.mutability() == ArtifactMutability::Immutable
                    && field.access() == Access::ReadOnly
                    && field.address_space() == AddressSpace::Global
                    && field.ownership() == ArgumentOwnership::SharedBorrow
                    && field.alias_class() == AliasClass::SharedReadOnly
            }
            3 => {
                matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4
                    }
                ) && field.mutability() == ArtifactMutability::Mutable
                    && field.access() == Access::ReadWrite
                    && field.address_space() == AddressSpace::Global
                    && field.ownership() == ArgumentOwnership::UniqueBorrow
                    && field.alias_class() == AliasClass::Exclusive
            }
            _ => unreachable!(),
        };
        if !exact {
            return Err(CollectedFlashAttentionErrorV1::Layout(format!(
                "ABI field {index} access, ownership, address space, or kind drifted"
            )));
        }
    }
    let launch = contract.launch();
    if launch.rank() != 1
        || launch.block_size()
            != BlockSize::Exact(
                Dimensions::new(64, 1, 1)
                    .map_err(|error| CollectedFlashAttentionErrorV1::Layout(error.to_string()))?,
            )
        || launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1)
                .map_err(|error| CollectedFlashAttentionErrorV1::Layout(error.to_string()))?
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedFlashAttentionErrorV1::Layout(
            "source launch must be exact WG64 with one-dimensional grid and no LDS".into(),
        ));
    }
    Ok(())
}

fn require_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<[u8; 32], CollectedFlashAttentionErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        CollectedFlashAttentionErrorV1::Abi(format!("FnAbi query failed: {error:?}"))
    })?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 4
        || abi.args.len() != 4
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(CollectedFlashAttentionErrorV1::Abi(format!(
            "FnAbi header must be Rust(args=4)->unit, found {abi:?}"
        )));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_DOMAIN_V1);
    hash_field(&mut digest, &[u8::from(abi.c_variadic)]);
    hash_field(&mut digest, &abi.fixed_count.to_le_bytes());
    hash_field(&mut digest, &[u8::from(abi.can_unwind)]);
    for (index, argument) in abi.args.iter().enumerate() {
        let expected_size = 16;
        if argument.layout.size.bytes() != expected_size || argument.layout.align.abi.bytes() != 8 {
            return Err(CollectedFlashAttentionErrorV1::Abi(format!(
                "FnAbi argument {index} size or alignment drifted"
            )));
        }
        hash_field(&mut digest, &argument.layout.size.bytes().to_le_bytes());
        hash_field(
            &mut digest,
            &argument.layout.align.abi.bytes().to_le_bytes(),
        );
        match argument.mode {
            PassMode::Pair(first, second) => {
                hash_field(&mut digest, &[2]);
                hash_arg_attributes(&mut digest, first);
                hash_arg_attributes(&mut digest, second);
            }
            _ => {
                return Err(CollectedFlashAttentionErrorV1::Abi(format!(
                    "FnAbi argument {index} pass mode drifted"
                )));
            }
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != RUSTC_FN_ABI_IDENTITY_V1 {
        return Err(CollectedFlashAttentionErrorV1::FnAbiIdentity {
            expected: RUSTC_FN_ABI_IDENTITY_V1,
            actual,
        });
    }
    Ok(actual)
}

fn hash_arg_attributes(digest: &mut Sha256, attributes: ArgAttributes) {
    hash_field(digest, &attributes.regular.bits().to_le_bytes());
    let extension = match attributes.arg_ext {
        ArgExtension::None => 0,
        ArgExtension::Zext => 1,
        ArgExtension::Sext => 2,
    };
    hash_field(digest, &[extension]);
    hash_field(digest, &attributes.pointee_size.bytes().to_le_bytes());
    hash_field(
        digest,
        &attributes
            .pointee_align
            .map_or(0, |value| value.bytes())
            .to_le_bytes(),
    );
}

fn trusted_definitions_and_terminals_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<[u8; 32], CollectedFlashAttentionErrorV1> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, TRUSTED_DEFINITIONS_DOMAIN_V4);
    hash_field(
        &mut digest,
        COLLECTED_FLASH_ATTENTION_PIPELINE_V1.as_bytes(),
    );
    let mut provider = None;
    for item in REQUIRED_TRUSTED_ITEMS_V1 {
        let definition = trusted_device_items::definition(tcx, *item).ok_or_else(|| {
            CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
                "missing exact diagnostic item `{}`",
                item.canonical_path()
            ))
        })?;
        if definition.is_local() || provider.is_some_and(|value| value != definition.krate) {
            return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
                "diagnostic item `{}` did not come from the single external device provider",
                item.canonical_path()
            )));
        }
        provider.get_or_insert(definition.krate);
        let identity = reviewed_device_definition_identity(
            tcx,
            definition,
            trusted_device_items::ProviderSemanticDefinitionRoleV1::TrustedDefinition,
            item.canonical_path(),
        )?;
        hash_field(&mut digest, &identity);
    }
    let provider = provider.ok_or_else(|| {
        CollectedFlashAttentionErrorV1::TrustedDefinitions(
            "exact profile has no reviewed device provider".into(),
        )
    })?;
    let provider_identity = compiler_provider(tcx, provider);

    let mut observed_terminals = Vec::new();
    let mut observed_core_terminals = Vec::new();
    for function in &collection.functions {
        let body = tcx.instance_mir(function.instance.def);
        for block in body.basic_blocks.iter() {
            let Some(terminator) = &block.terminator else {
                return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(
                    "collected MIR block omitted its terminator".into(),
                ));
            };
            let TerminatorKind::Call { func, .. } = &terminator.kind else {
                continue;
            };
            let Operand::Constant(constant) = func else {
                continue;
            };
            let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
                continue;
            };
            let resolved = rustc_middle::ty::Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *def_id,
                args,
            )
            .map_err(|_| {
                CollectedFlashAttentionErrorV1::TrustedDefinitions(
                    "semantic-terminal call resolution failed".into(),
                )
            })?
            .map_or(*def_id, |instance| instance.def_id());
            if let Some(role) = classify_exact_flash_attention_compiler_intrinsic(tcx, resolved) {
                if observed_core_terminals.iter().all(|old| *old != role) {
                    observed_core_terminals.push(role);
                }
                continue;
            }
            let Some(item) = trusted_device_items::classify(tcx, resolved) else {
                continue;
            };
            if !REVIEWED_SEMANTIC_TERMINALS_V1.contains(&item) {
                return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
                    "unreviewed semantic terminal `{}` entered the exact MIR closure",
                    item.canonical_path()
                )));
            }
            if observed_terminals.iter().all(|(old, _)| *old != item) {
                observed_terminals.push((item, resolved));
            }
        }
    }
    if observed_core_terminals != [FlashAttentionCompilerIntrinsicV1::FabsF32] {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
            "core semantic-terminal set drifted: expected fabs::<f32>, found {observed_core_terminals:?}"
        )));
    }
    if observed_terminals.len() != REVIEWED_SEMANTIC_TERMINALS_V1.len()
        || REVIEWED_SEMANTIC_TERMINALS_V1.iter().any(|expected| {
            observed_terminals
                .iter()
                .all(|(actual, _)| actual != expected)
        })
    {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
            "semantic-terminal set drifted: expected {:?}, found {:?}",
            REVIEWED_SEMANTIC_TERMINALS_V1
                .iter()
                .map(|item| item.canonical_path())
                .collect::<Vec<_>>(),
            observed_terminals
                .iter()
                .map(|(item, _)| item.canonical_path())
                .collect::<Vec<_>>()
        )));
    }
    for expected in REVIEWED_SEMANTIC_TERMINALS_V1 {
        let definition = observed_terminals
            .iter()
            .find_map(|(item, definition)| (item == expected).then_some(*definition))
            .expect("terminal set checked above");
        let terminal_provider = compiler_provider(tcx, definition.krate);
        if terminal_provider != provider_identity {
            return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
                "semantic terminal `{}` came from unreviewed provider `{}`",
                expected.canonical_path(),
                terminal_provider.crate_name
            )));
        }
        let identity = reviewed_device_definition_identity(
            tcx,
            definition,
            trusted_device_items::ProviderSemanticDefinitionRoleV1::SemanticTerminal,
            expected.canonical_path(),
        )?;
        hash_field(&mut digest, &identity);
    }
    let core_provider = tcx
        .lang_items()
        .get(rustc_hir::lang_items::LangItem::Sized)
        .ok_or_else(|| {
            CollectedFlashAttentionErrorV1::TrustedDefinitions(
                "pinned compiler omitted the core Sized lang item".into(),
            )
        })?
        .krate;
    let core_provider = compiler_provider(tcx, core_provider);
    if core_provider.crate_name != "core"
        || core_provider.stable_crate_id == 0
        || core_provider.crate_hash_observation == [0; 16]
    {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(
            "pinned core provider identity is incomplete".into(),
        ));
    }
    let fabs_definition = observed_core_terminals
        .iter()
        .find_map(|role| {
            (*role == FlashAttentionCompilerIntrinsicV1::FabsF32).then(|| {
                collection.functions.iter().find_map(|function| {
                    let body = tcx.instance_mir(function.instance.def);
                    body.basic_blocks.iter().find_map(|block| {
                        let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                            return None;
                        };
                        let Operand::Constant(constant) = func else {
                            return None;
                        };
                        let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
                            return None;
                        };
                        rustc_middle::ty::Instance::try_resolve(
                            tcx,
                            TypingEnv::fully_monomorphized(),
                            *def_id,
                            args,
                        )
                        .ok()
                        .flatten()
                        .map(|instance| instance.def_id())
                        .filter(|definition| {
                            classify_exact_flash_attention_compiler_intrinsic(tcx, *definition)
                                == Some(FlashAttentionCompilerIntrinsicV1::FabsF32)
                        })
                    })
                })
            })
        })
        .flatten()
        .ok_or_else(|| {
            CollectedFlashAttentionErrorV1::TrustedDefinitions(
                "fabs semantic terminal definition was not retained".into(),
            )
        })?;
    if compiler_provider(tcx, fabs_definition.krate) != core_provider {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(
            "fabs semantic terminal did not come from pinned core".into(),
        ));
    }
    let core_terminal_identity = trusted_device_items::pinned_core_semantic_terminal_identity_v1(
        &core_provider,
        FlashAttentionCompilerIntrinsicV1::FabsF32.canonical_path(),
        &tcx.def_path(fabs_definition).to_string_no_crate_verbose(),
    )
    .map_err(CollectedFlashAttentionErrorV1::TrustedDefinitions)?;
    hash_field(&mut digest, &core_terminal_identity);
    let actual: [u8; 32] = digest.finalize().into();
    if actual != TRUSTED_TERMINAL_IDENTITY_V4 {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(format!(
            "trusted-definition/semantic-terminal identity drifted: expected {}, found {}",
            crate::encode_hex(&TRUSTED_TERMINAL_IDENTITY_V4),
            crate::encode_hex(&actual)
        )));
    }
    Ok(actual)
}

fn compiler_provider(
    tcx: TyCtxt<'_>,
    crate_num: rustc_hir::def_id::CrateNum,
) -> trusted_device_items::CompilerProviderObservationV1 {
    trusted_device_items::compiler_provider_observation_v1(tcx, crate_num)
}

fn reviewed_device_definition_identity(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
    definition_role: trusted_device_items::ProviderSemanticDefinitionRoleV1,
    canonical_role: &str,
) -> Result<[u8; 32], CollectedFlashAttentionErrorV1> {
    let observed = trusted_device_items::reviewed_provider_semantic_definition_v1(tcx, definition)
        .map_err(CollectedFlashAttentionErrorV1::TrustedDefinitions)?;
    let provider = compiler_provider(tcx, definition.krate);
    if observed.provider != provider {
        return Err(CollectedFlashAttentionErrorV1::TrustedDefinitions(
            "reviewed device provider observation changed within the compiler session".into(),
        ));
    }
    observed
        .durable_semantic_identity(definition_role, canonical_role)
        .map_err(CollectedFlashAttentionErrorV1::TrustedDefinitions)
}

fn observe_compiler_semantics(tcx: TyCtxt<'_>) -> CompilerSemanticsV1 {
    CompilerSemanticsV1 {
        rustc_release: env!("FE2O3_BUILD_RUSTC_RELEASE"),
        rustc_commit: env!("FE2O3_BUILD_RUSTC_COMMIT"),
        llvm_version: env!("FE2O3_BUILD_RUSTC_LLVM"),
        panic_strategy: format!("{:?}", tcx.sess.panic_strategy()),
        overflow_checks: tcx.sess.overflow_checks(),
        optimize: format!("{:?}", tcx.sess.opts.optimize),
        debug_assertions: tcx.sess.opts.debug_assertions,
        mir_opt_level: tcx.sess.mir_opt_level(),
        mir_enable_passes: tcx.sess.opts.unstable_opts.mir_enable_passes.clone(),
        llvm_args: tcx.sess.opts.cg.llvm_args.clone(),
        llvm_passes: tcx.sess.opts.cg.passes.clone(),
        target_cpu: tcx.sess.opts.cg.target_cpu.clone(),
        target_features: tcx.sess.opts.cg.target_feature.clone(),
        rustc_codegen_opt_level: tcx.sess.opts.cg.opt_level.clone(),
        crate_name: tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE).to_string(),
        crate_metadata: tcx.sess.opts.cg.metadata.clone(),
        remap_path_destinations: tcx
            .sess
            .opts
            .remap_path_prefix
            .iter()
            .map(|(_, destination)| destination.display().to_string())
            .collect(),
    }
}

fn require_compiler_semantics(
    observed: &CompilerSemanticsV1,
) -> Result<[u8; 32], CollectedFlashAttentionErrorV1> {
    let expected_passes = [("JumpThreading".to_owned(), false)];
    let mismatch = if observed.rustc_release != REVIEWED_RUSTC_RELEASE {
        Some(format!("rustc release must be {REVIEWED_RUSTC_RELEASE}"))
    } else if observed.rustc_commit != REVIEWED_RUSTC_COMMIT {
        Some(format!("rustc commit must be {REVIEWED_RUSTC_COMMIT}"))
    } else if observed.llvm_version != REVIEWED_RUSTC_LLVM {
        Some(format!("rustc LLVM must be {REVIEWED_RUSTC_LLVM}"))
    } else if observed.panic_strategy != "Unwind"
        || observed.overflow_checks
        || observed.optimize != "No"
        || !observed.debug_assertions
        || observed.mir_opt_level != 1
        || observed.mir_enable_passes != expected_passes
        || observed.rustc_codegen_opt_level != "0"
    {
        Some("panic/overflow/optimization/debug/MIR semantics drifted".into())
    } else if !observed.llvm_args.is_empty()
        || !observed.llvm_passes.is_empty()
        || observed.target_cpu.is_some()
        || !observed.target_features.is_empty()
    {
        Some("custom LLVM or target feature selection is forbidden".into())
    } else if observed.crate_name != REVIEWED_CRATE_NAME {
        Some(format!(
            "crate name must be exactly {REVIEWED_CRATE_NAME:?}"
        ))
    } else if observed.crate_metadata != [REVIEWED_CRATE_METADATA] {
        Some(format!(
            "crate metadata must be exactly {REVIEWED_CRATE_METADATA:?}"
        ))
    } else if derive_crate_binding_id_v1(
        &observed.crate_name,
        observed.crate_metadata.iter().map(String::as_str),
    ) != compiler_crate_binding()
    {
        Some("crate name and ordered metadata do not derive the reviewed compiler binding".into())
    } else if observed.remap_path_destinations
        != [SOURCE_REMAP_DESTINATION, WORKSPACE_REMAP_DESTINATION]
    {
        Some(format!(
            "source remapping must be exactly the reviewed fixture and workspace destinations, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedFlashAttentionErrorV1::Admission(detail));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, COMPILER_SEMANTICS_DOMAIN_V1);
    hash_field(&mut digest, observed.rustc_release.as_bytes());
    hash_field(&mut digest, observed.rustc_commit.as_bytes());
    hash_field(&mut digest, observed.llvm_version.as_bytes());
    hash_field(&mut digest, observed.panic_strategy.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.overflow_checks)]);
    hash_field(&mut digest, observed.optimize.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.debug_assertions)]);
    hash_field(&mut digest, &(observed.mir_opt_level as u64).to_le_bytes());
    for (name, enabled) in &observed.mir_enable_passes {
        hash_field(&mut digest, name.as_bytes());
        hash_field(&mut digest, &[u8::from(*enabled)]);
    }
    hash_field(&mut digest, observed.rustc_codegen_opt_level.as_bytes());
    hash_field(&mut digest, observed.crate_name.as_bytes());
    hash_field(&mut digest, observed.crate_metadata[0].as_bytes());
    for destination in &observed.remap_path_destinations {
        hash_field(&mut digest, destination.as_bytes());
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != COMPILER_SEMANTICS_IDENTITY_V1 {
        return Err(CollectedFlashAttentionErrorV1::Admission(format!(
            "compiler semantics identity drifted: expected {}, found {}",
            crate::encode_hex(&COMPILER_SEMANTICS_IDENTITY_V1),
            crate::encode_hex(&actual)
        )));
    }
    Ok(actual)
}

fn exact_target_identity() -> Result<TargetIdentity, CollectedFlashAttentionErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| CollectedFlashAttentionErrorV1::Admission(error.to_string()))?,
        IdentityText::new(EXACT_FLASH_ATTENTION_TARGET_V1)
            .map_err(|error| CollectedFlashAttentionErrorV1::Admission(error.to_string()))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Subgroup, Capability::AmdWave],
    )
    .map_err(|error| CollectedFlashAttentionErrorV1::Admission(error.to_string()))
}

fn exact_profile_launch() -> Result<LaunchContract, CollectedFlashAttentionErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedFlashAttentionErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| CollectedFlashAttentionErrorV1::Layout(error.to_string()))?,
        0,
        0,
    )
    .map_err(|error| CollectedFlashAttentionErrorV1::Layout(error.to_string()))
}

fn validate_authority(
    authority: &FlashAttentionAuthorityV1,
) -> Result<(), CollectedFlashAttentionErrorV1> {
    let field = if authority.source_identity != FLASH_ATTENTION_V1_SOURCE_SHA256 {
        Some("source bytes")
    } else if authority.source_namespace != FLASH_ATTENTION_V1_NAMESPACE {
        Some("source namespace")
    } else if authority.compiler_crate_binding != compiler_crate_binding().as_bytes() {
        Some("wrapper-derived compiler crate binding")
    } else if authority.target != EXACT_FLASH_ATTENTION_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != FLASH_ATTENTION_CODE_OBJECT_VERSION_V1 {
        Some("code object version")
    } else if authority.kernel_export != FLASH_ATTENTION_V1_KERNEL_ID {
        Some("kernel export")
    } else if authority.root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
        Some("root instance identity")
    } else if authority.portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
        Some("complete reachable MIR closure")
    } else if authority.fn_abi_identity != RUSTC_FN_ABI_IDENTITY_V1 {
        Some("rustc FnAbi")
    } else if authority.compiler_semantics_identity != COMPILER_SEMANTICS_IDENTITY_V1
        || authority.trusted_definitions_identity != TRUSTED_TERMINAL_IDENTITY_V4
    {
        Some("compiler/trusted definition closure")
    } else if authority.frontend_contract_identity != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.abi_identity != sha256(ABI_BINDING_V1) {
        Some("ABI")
    } else if authority.effects_identity != sha256(EFFECT_BINDING_V1) {
        Some("effects")
    } else if authority.source_launch_identity != sha256(SOURCE_LAUNCH_BINDING_V1)
        || authority.profile_launch_identity != sha256(PROFILE_LAUNCH_BINDING_V1)
    {
        Some("source/profile launch")
    } else if authority.numerical_identity != sha256(NUMERICAL_BINDING_V1) {
        Some("strict finite integral-f32 policy")
    } else if authority.descriptor_identity != sha256(DESCRIPTOR_BINDING_V1) {
        Some("descriptor")
    } else if authority.canonical_ir_identity != sha256(CANONICAL_IR_BINDING_V1) {
        Some("canonical semantic IR")
    } else if authority.correspondence_identity != sha256(CORRESPONDENCE_BINDING_V1) {
        Some("reviewed correspondence boundary")
    } else if authority.authority_identity != authority_identity(authority) {
        Some("authority commitment")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedFlashAttentionErrorV1::ReceiptBinding(field));
    }
    Ok(())
}

fn authority_identity(authority: &FlashAttentionAuthorityV1) -> [u8; 32] {
    sha256(&authority_transcript(authority))
}

fn authority_transcript(authority: &FlashAttentionAuthorityV1) -> Vec<u8> {
    let mut transcript = Vec::with_capacity(1024);
    push_transcript_field(&mut transcript, AUTHORITY_DOMAIN_V1);
    push_transcript_field(&mut transcript, &authority.source_identity);
    push_transcript_field(&mut transcript, &authority.source_namespace);
    push_transcript_field(&mut transcript, &authority.compiler_crate_binding);
    push_transcript_field(&mut transcript, authority.target.as_bytes());
    push_transcript_field(
        &mut transcript,
        &authority.code_object_version.to_le_bytes(),
    );
    push_transcript_field(&mut transcript, authority.kernel_export.as_bytes());
    push_transcript_field(&mut transcript, authority.root_instance_identity.as_bytes());
    push_transcript_field(&mut transcript, &authority.portable_mir_identity);
    push_transcript_field(&mut transcript, &authority.compiler_semantics_identity);
    push_transcript_field(&mut transcript, &authority.fn_abi_identity);
    push_transcript_field(&mut transcript, &authority.trusted_definitions_identity);
    push_transcript_field(&mut transcript, &authority.frontend_contract_identity);
    push_transcript_field(&mut transcript, &authority.abi_identity);
    push_transcript_field(&mut transcript, &authority.effects_identity);
    push_transcript_field(&mut transcript, &authority.source_launch_identity);
    push_transcript_field(&mut transcript, &authority.profile_launch_identity);
    push_transcript_field(&mut transcript, &authority.numerical_identity);
    push_transcript_field(&mut transcript, &authority.descriptor_identity);
    push_transcript_field(&mut transcript, &authority.canonical_ir_identity);
    push_transcript_field(&mut transcript, &authority.correspondence_identity);
    transcript
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn compiler_crate_binding() -> CrateBindingIdV1 {
    CrateBindingIdV1::from_hex(REVIEWED_COMPILER_CRATE_BINDING)
        .expect("reviewed FlashAttention compiler crate binding is canonical")
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn push_transcript_field(transcript: &mut Vec<u8>, bytes: &[u8]) {
    transcript.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    transcript.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> FlashAttentionFrontendReceiptV1 {
        let mut authority = FlashAttentionAuthorityV1 {
            source_identity: FLASH_ATTENTION_V1_SOURCE_SHA256,
            source_namespace: FLASH_ATTENTION_V1_NAMESPACE,
            compiler_crate_binding: compiler_crate_binding().as_bytes(),
            target: EXACT_FLASH_ATTENTION_TARGET_V1.into(),
            code_object_version: 6,
            kernel_export: FLASH_ATTENTION_V1_KERNEL_ID.into(),
            root_instance_identity: REVIEWED_ROOT_INSTANCE_IDENTITY.into(),
            portable_mir_identity: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
            compiler_semantics_identity: COMPILER_SEMANTICS_IDENTITY_V1,
            fn_abi_identity: RUSTC_FN_ABI_IDENTITY_V1,
            trusted_definitions_identity: TRUSTED_TERMINAL_IDENTITY_V4,
            frontend_contract_identity: sha256(EXACT_FRONTEND_CONTRACT_V1),
            abi_identity: sha256(ABI_BINDING_V1),
            effects_identity: sha256(EFFECT_BINDING_V1),
            source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
            profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
            numerical_identity: sha256(NUMERICAL_BINDING_V1),
            descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
            canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
            correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
            authority_identity: [0; 32],
        };
        authority.authority_identity = authority_identity(&authority);
        FlashAttentionFrontendReceiptV1 {
            authority: Some(authority),
            ir: Some(flash_attention_v1_kernel_ir()),
            profile: Some(FlashAttentionProfileV1::exact_gfx942_xnack_minus_cov6()),
        }
    }

    #[test]
    fn receipt_selects_only_the_exact_semantic_profile_once() {
        let mut value = receipt();
        let admitted = value.consume().unwrap();
        assert_eq!(admitted.semantic_summary(), (1, 1, 8, 16, 10));
        assert_eq!(admitted.profile().grid, [1, 1, 1]);
        assert_eq!(
            value.consume(),
            Err(CollectedFlashAttentionErrorV1::ReceiptAlreadyConsumed)
        );
    }

    #[test]
    fn authority_mutations_fail_closed() {
        let mutations: Vec<fn(&mut FlashAttentionAuthorityV1)> = vec![
            |value| value.source_identity[0] ^= 1,
            |value| value.source_namespace[0] ^= 1,
            |value| value.compiler_crate_binding[0] ^= 1,
            |value| value.target.push('+'),
            |value| value.code_object_version = 5,
            |value| value.root_instance_identity.push('_'),
            |value| value.portable_mir_identity[0] ^= 1,
            |value| value.compiler_semantics_identity[0] ^= 1,
            |value| value.fn_abi_identity[0] ^= 1,
            |value| value.trusted_definitions_identity[0] ^= 1,
            |value| value.abi_identity[0] ^= 1,
            |value| value.effects_identity[0] ^= 1,
            |value| value.profile_launch_identity[0] ^= 1,
            |value| value.numerical_identity[0] ^= 1,
            |value| value.descriptor_identity[0] ^= 1,
            |value| value.canonical_ir_identity[0] ^= 1,
            |value| value.correspondence_identity[0] ^= 1,
        ];
        for mutate in mutations {
            let mut value = receipt();
            mutate(value.authority.as_mut().unwrap());
            value.authority.as_mut().unwrap().authority_identity =
                authority_identity(value.authority.as_ref().unwrap());
            assert!(matches!(
                value.consume(),
                Err(CollectedFlashAttentionErrorV1::ReceiptBinding(_))
            ));
        }
    }

    #[test]
    fn canonical_ir_and_profile_substitutions_fail_after_source_authentication() {
        let mut ir = receipt();
        ir.ir.as_mut().unwrap().recurrence.swap(3, 4);
        assert!(matches!(
            ir.consume(),
            Err(CollectedFlashAttentionErrorV1::CanonicalIr(_))
        ));

        let mut profile = receipt();
        profile.profile.as_mut().unwrap().code_object_version = 5;
        assert!(matches!(
            profile.consume(),
            Err(CollectedFlashAttentionErrorV1::CanonicalIr(_))
        ));
    }
}
