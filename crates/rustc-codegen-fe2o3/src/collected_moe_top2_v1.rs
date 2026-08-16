//! Exact-source authentication for deterministic MoE top-2 routing V1.
//!
//! This layer authenticates the attributed source bytes (including their
//! Phase A fallback namespace), the distinct wrapper/session-derived
//! registration binding, rustc ABI, and complete reachable portable-MIR
//! closure before selecting the closed semantic profile in `fe2o3-kernel-ir`.
//! The selection is reviewed correspondence, not compiler, IEEE-754, or
//! source-to-Verus/model refinement proof.
//! No generic IR is silently substituted and no Worker V2, LLVM, link,
//! artifact, or execution authority is produced here.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[path = "moe_top2_source_kir_correspondence.rs"]
mod moe_top2_source_kir_correspondence;

use self::moe_top2_source_kir_correspondence::{
    CheckedMoeSourceKirStructuralRecordV2, MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2,
    MoeFnAbiArgumentStructuralProjectionV2, MoeFnAbiStructuralProjectionV2,
    canonical_kernel_profile_identities_v2, produce_checked_moe_source_kir_structural_record_v2,
    seal_authenticated_live_inputs_v2,
};
use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Dimensions, Endianness, IdentityText, LaunchContract, Mutability as ArtifactMutability,
    PointerWidth, RustScalarElementTypeV1, TargetIdentity,
};
use fe2o3_kernel_ir::{
    MOE_TOP2_V1_EXPLICIT_KERNARG_BYTES, MOE_TOP2_V1_KERNEL_ID, MOE_TOP2_V1_NAMESPACE,
    MOE_TOP2_V1_SOURCE_SHA256, MoeTop2KernelIrV1, MoeTop2ProfileV1, moe_top2_v1_kernel_ir,
    verify_moe_top2_v1,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{FloatTy, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_span::SourceFile;
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectionResult, TypedKernelProfile};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem};

pub(crate) const COLLECTED_MOE_TOP2_PIPELINE_V1: &str = "collected-moe-top2-v1";
pub(crate) const EXACT_MOE_TOP2_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const MOE_TOP2_CODE_OBJECT_VERSION_V1: u16 = 6;

const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_NAME: &str = "fe2o3_collected_moe_top2_v1_fixture";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-moe-top2-v1-reviewed";
// The ordinary macro wrapper overrides the source fallback namespace with the
// binding derived from this exact crate name and ordered metadata. Authority
// commits both identities so the override is visible and cannot substitute
// either the public source bytes or the compiler session.
const REVIEWED_COMPILER_CRATE_BINDING: &str =
    "fce826d20b8f2e4eca29180a2d9fc34949b51a07841dd7f79258625fc6a9f296";
const SOURCE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace/moe-top2-v1.rs";
const WORKSPACE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace";
const REVIEWED_ROOT_INSTANCE_IDENTITY: &str = "kernel::__fe2o3_host_kernel_v1_0d0504325353eb74b0c9ace47560290e2278a7cd7c20e3b1c6c70f4a7e37b1ab";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.moe-top2.compiler-semantics.v1";
const TRUSTED_DEFINITIONS_DOMAIN_V3: &[u8] = b"fe2o3.moe-top2.trusted-definitions-and-terminals.v3";
const AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.moe-top2.source-authority.v1";
const FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3.moe-top2.rustc-fn-abi.v1";
const FN_ABI_CALLING_CONVENTION_RUST_V2: u8 = 0;
const FN_ABI_RETURN_MODE_IGNORE_V2: u8 = 0;
const ABI_BINDING_V1: &[u8] = b"ptr64;size=128;align=8;logits@0:16:8:slice-f32:shared-readonly;top2@16:16:8:disjoint-u32:exclusive-readwrite;requested@32:16:8:disjoint-u32:exclusive-readwrite;admitted@48:16:8:disjoint-u32:exclusive-readwrite;offsets@64:16:8:disjoint-u32:exclusive-readwrite;slots@80:16:8:disjoint-u32:exclusive-readwrite;permutation@96:16:8:disjoint-u32:exclusive-readwrite;inverse@112:16:8:disjoint-u32:exclusive-readwrite";
const EFFECT_BINDING_V1: &[u8] = b"logits:shared-readonly-f32x32;seven-disjoint-u32-outputs:exclusive;lane0-owns-every-output-element-once;lanes1..63-write-none;all-shape-and-finite-input-failures-trap-before-any-output-write";
const SOURCE_LAUNCH_BINDING_V1: &[u8] =
    b"rank=1;block=exact(64,1,1);max-grid=(4294967295,1,1);static-shared=0;dynamic-shared=0";
const PROFILE_LAUNCH_BINDING_V1: &[u8] =
    b"target=gfx942:xnack-;cov=6;wave=64;block=exact(64,1,1);grid=exact(1,1,1)";
const ROUTING_BINDING_V1: &[u8] = b"t=8;e=4;k=2;capacity=4;logits=finite-f32-token-major;top2=descending-score-lower-expert-tie;requested=exact-route-count;admitted=min(requested,4);offsets=exclusive-expert-scan;drop=stable-route-prefix;slot=offset+stable-rank-unique-bounded;permutation-inverse=round-trip;sentinel=u32-max-for-dropped-and-tail";
const DESCRIPTOR_BINDING_V1: &[u8] = b"logical=moe_top2_route_f32_t8_e4_k2_c4_v1;export=moe_top2_route_f32_t8_e4_k2_c4_v1;descriptor=moe_top2_route_f32_t8_e4_k2_c4_v1.kd;explicit-kernarg=128;complete-cov6-kernarg=384;wg=64,1,1;wave=64;static-lds=0;dynamic-lds=0";
const CANONICAL_IR_BINDING_V1: &[u8] = b"fe2o3::moe_top2_route_f32_t8_e4_k2_c4_v1;args=logits-shared-f32x32,seven-lane0-owned-u32-outputs;ordered-routing=validate,select,count,clamp,scan,initialize,stable-rank,slot,permutation-inverse,commit;ownership=lane0-total-exclusive-in-bounds;lanes1..63-inactive";
const CORRESPONDENCE_BINDING_V1: &[u8] = b"exact attributed source plus wrapper/session registration, exact rustc FnAbi, location-independent V3 trusted definitions, identity-bound reviewed semantic terminals, and complete reachable portable-MIR modulo those terminals select a closed deterministic MoE top-2 semantic sidecar;reviewed correspondence only;not generic lowering, IEEE-754 refinement, terminal-body refinement, compiler refinement, or source-to-Verus/model refinement";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

// Filled from the pinned compiler fixture after path-independent portable-MIR
// import. Any reachable body, call target, type, or operation drift changes it.
const PORTABLE_MIR_CLOSURE_IDENTITY_V1: [u8; 32] = [
    0x93, 0x4c, 0x22, 0x05, 0x97, 0x3e, 0x24, 0x21, 0x6d, 0x53, 0x7c, 0x5f, 0x89, 0xbc, 0x65, 0xd8,
    0xe1, 0x5d, 0xd6, 0x83, 0x76, 0xdc, 0xe4, 0x77, 0xd1, 0x76, 0x8e, 0x29, 0x36, 0xb4, 0xfc, 0x13,
];
const RUSTC_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0xdd, 0xc0, 0x17, 0x2c, 0xfc, 0x37, 0x01, 0x6c, 0x86, 0xbe, 0x2b, 0x57, 0x9c, 0x4c, 0x98, 0xb1,
    0x4f, 0x82, 0x3d, 0xd9, 0x37, 0x18, 0x16, 0xb6, 0x64, 0x8f, 0x1b, 0x8b, 0xd0, 0x61, 0xbd, 0x88,
];
const COMPILER_SEMANTICS_IDENTITY_V1: [u8; 32] = [
    0x49, 0x50, 0xc2, 0x25, 0xe0, 0xcd, 0xbd, 0xce, 0x4e, 0x12, 0x30, 0x16, 0x69, 0x84, 0x94, 0x99,
    0x70, 0x29, 0x0d, 0xed, 0xc1, 0x9e, 0x8d, 0xc4, 0xcd, 0x31, 0xf8, 0x65, 0xf1, 0x62, 0x5a, 0x4a,
];
const TRUSTED_TERMINAL_IDENTITY_V3: [u8; 32] = [
    0x3d, 0xbb, 0xe3, 0xec, 0x9d, 0x58, 0xa7, 0xc2, 0x85, 0xa1, 0x41, 0x59, 0x29, 0x40, 0x51, 0x49,
    0x83, 0x78, 0xf2, 0x91, 0x52, 0x5d, 0x84, 0x45, 0x11, 0x3b, 0x17, 0xaa, 0xb9, 0xb0, 0xe0, 0x8b,
];

const ARGUMENT_KINDS_V1: [GeneralTypedArgumentKindV3; 8] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::U32),
];

const REQUIRED_TRUSTED_ITEMS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::DisjointSlice,
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];

const REVIEWED_SEMANTIC_TERMINALS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum MoeTop2CompilerIntrinsicV1 {
    FabsF32,
}

impl MoeTop2CompilerIntrinsicV1 {
    pub(crate) const fn canonical_path(self) -> &'static str {
        match self {
            Self::FabsF32 => "core::intrinsics::fabs::<f32>",
        }
    }
}

pub(crate) fn classify_exact_moe_top2_compiler_intrinsic(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::DefId,
) -> Option<MoeTop2CompilerIntrinsicV1> {
    if std::env::var("FE2O3_CODEGEN_PIPELINE").as_deref() != Ok(COLLECTED_MOE_TOP2_PIPELINE_V1)
        || def_id.is_local()
    {
        return None;
    }
    tcx.def_path_str(def_id)
        .ends_with("::intrinsics::fabs")
        .then_some(MoeTop2CompilerIntrinsicV1::FabsF32)
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
struct MoeTop2AuthorityV1 {
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
    routing_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    correspondence_identity: [u8; 32],
    authority_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
struct RustcLoadedMoeTop2SourceV2 {
    contents: Arc<String>,
    identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservedMoeTop2FnAbiV1 {
    identity: [u8; 32],
    structural_projection: MoeFnAbiStructuralProjectionV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedMoeTop2FnAbiHeaderV2 {
    calling_convention: u8,
    c_variadic: bool,
    fixed_count: u64,
    argument_count: u64,
    can_unwind: bool,
    return_mode: u8,
    return_size: u64,
    return_alignment: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MoeTop2FrontendReceiptV1 {
    authority: Option<MoeTop2AuthorityV1>,
    ir: Option<MoeTop2KernelIrV1>,
    profile: Option<MoeTop2ProfileV1>,
    structural_record: Option<CheckedMoeSourceKirStructuralRecordV2>,
}

impl MoeTop2FrontendReceiptV1 {
    fn authority(&self) -> &MoeTop2AuthorityV1 {
        self.authority.as_ref().expect("unconsumed MoeTop2 receipt")
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_hex(&self) -> String {
        encode_hex(&self.authority().portable_mir_identity)
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(&self.authority().authority_identity)
    }

    pub(crate) fn structural_record_hex(&self) -> String {
        encode_hex(
            &self
                .structural_record
                .as_ref()
                .expect("unconsumed MoeTop2 structural record")
                .identity(),
        )
    }

    pub(crate) fn consume(&mut self) -> Result<AuthenticatedMoeTop2V1, CollectedMoeTop2ErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedMoeTop2ErrorV1::ReceiptAlreadyConsumed)?;
        let ir = self
            .ir
            .take()
            .ok_or(CollectedMoeTop2ErrorV1::ReceiptAlreadyConsumed)?;
        let profile = self
            .profile
            .take()
            .ok_or(CollectedMoeTop2ErrorV1::ReceiptAlreadyConsumed)?;
        let structural_record = self
            .structural_record
            .take()
            .ok_or(CollectedMoeTop2ErrorV1::ReceiptAlreadyConsumed)?;
        let _validated_authority = validated_authority::validate_authority(&authority)?;
        verify_moe_top2_v1(&ir, &profile)
            .map_err(|error| CollectedMoeTop2ErrorV1::CanonicalIr(error.to_string()))?;
        let (kernel_ir_identity, profile_identity) =
            canonical_kernel_profile_identities_v2(&ir, &profile);
        if structural_record.source_identity() != authority.source_identity
            || structural_record.fn_abi_identity() != authority.fn_abi_identity
            || structural_record.portable_mir_identity() != authority.portable_mir_identity
            || structural_record.compiler_semantics_identity()
                != authority.compiler_semantics_identity
            || structural_record.trusted_definitions_identity()
                != authority.trusted_definitions_identity
            || structural_record.root_instance_identity() != authority.root_instance_identity
            || structural_record.source_authority_identity() != authority.authority_identity
            || structural_record.kernel_ir_identity() != kernel_ir_identity
            || structural_record.profile_identity() != profile_identity
            || structural_record.snapshot() != MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2
            || structural_record.proves_source_to_kir_semantic_refinement()
            || structural_record.proves_llvm_or_isa_refinement()
            || structural_record.proves_logical_to_machine_address_refinement()
            || structural_record.proves_ieee_fp32_or_ocml_semantics()
            || structural_record.proves_generalized_memory_safety_or_race_freedom()
            || structural_record.proves_gpu_execution()
            || structural_record.grants_artifact_authority()
            || structural_record.grants_load_authority()
            || structural_record.grants_launch_authority()
        {
            return Err(CollectedMoeTop2ErrorV1::ReceiptBinding(
                "inert private source/FnAbi/MIR/KIR structural record",
            ));
        }
        Ok(AuthenticatedMoeTop2V1 {
            ir,
            profile,
            source_identity: authority.source_identity,
            source_namespace: authority.source_namespace,
            compiler_crate_binding: authority.compiler_crate_binding,
            source_authority_identity: authority.authority_identity,
            portable_mir_identity: authority.portable_mir_identity,
            compiler_semantics_identity: authority.compiler_semantics_identity,
            fn_abi_identity: authority.fn_abi_identity,
            trusted_definitions_identity: authority.trusted_definitions_identity,
            abi_identity: authority.abi_identity,
            effects_identity: authority.effects_identity,
            profile_launch_identity: authority.profile_launch_identity,
            routing_identity: authority.routing_identity,
            descriptor_identity: authority.descriptor_identity,
            canonical_ir_identity: authority.canonical_ir_identity,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMoeTop2V1 {
    ir: MoeTop2KernelIrV1,
    profile: MoeTop2ProfileV1,
    source_identity: [u8; 32],
    source_namespace: [u8; 32],
    compiler_crate_binding: [u8; 32],
    source_authority_identity: [u8; 32],
    portable_mir_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    abi_identity: [u8; 32],
    effects_identity: [u8; 32],
    profile_launch_identity: [u8; 32],
    routing_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
}

impl AuthenticatedMoeTop2V1 {
    pub(crate) fn semantic_summary(&self) -> (u8, u8, u8, u8, usize) {
        (
            self.ir.shape.tokens,
            self.ir.shape.experts,
            self.ir.shape.experts_per_token,
            self.ir.shape.expert_capacity,
            self.ir.routing.len(),
        )
    }

    pub(crate) fn profile(&self) -> &MoeTop2ProfileV1 {
        &self.profile
    }

    pub(crate) fn into_worker_parts(self) -> AuthenticatedMoeTop2WorkerPartsV1 {
        AuthenticatedMoeTop2WorkerPartsV1 {
            ir: self.ir,
            profile: self.profile,
            source_identity: self.source_identity,
            source_namespace: self.source_namespace,
            compiler_crate_binding: self.compiler_crate_binding,
            source_authority_identity: self.source_authority_identity,
            portable_mir_identity: self.portable_mir_identity,
            compiler_semantics_identity: self.compiler_semantics_identity,
            fn_abi_identity: self.fn_abi_identity,
            trusted_definitions_identity: self.trusted_definitions_identity,
            abi_identity: self.abi_identity,
            effects_identity: self.effects_identity,
            profile_launch_identity: self.profile_launch_identity,
            routing_identity: self.routing_identity,
            descriptor_identity: self.descriptor_identity,
            canonical_ir_identity: self.canonical_ir_identity,
        }
    }
}

/// Private linear bridge from the authenticated source/KIR receipt to the
/// exact MoE Worker V2 producer. It is intentionally not cloneable.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMoeTop2WorkerPartsV1 {
    pub(crate) ir: MoeTop2KernelIrV1,
    pub(crate) profile: MoeTop2ProfileV1,
    pub(crate) source_identity: [u8; 32],
    pub(crate) source_namespace: [u8; 32],
    pub(crate) compiler_crate_binding: [u8; 32],
    pub(crate) source_authority_identity: [u8; 32],
    pub(crate) portable_mir_identity: [u8; 32],
    pub(crate) compiler_semantics_identity: [u8; 32],
    pub(crate) fn_abi_identity: [u8; 32],
    pub(crate) trusted_definitions_identity: [u8; 32],
    pub(crate) abi_identity: [u8; 32],
    pub(crate) effects_identity: [u8; 32],
    pub(crate) profile_launch_identity: [u8; 32],
    pub(crate) routing_identity: [u8; 32],
    pub(crate) descriptor_identity: [u8; 32],
    pub(crate) canonical_ir_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CollectedMoeTop2ErrorV1 {
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
    SourceKirStructuralRecord(String),
    ReceiptAlreadyConsumed,
    ReceiptBinding(&'static str),
}

impl fmt::Display for CollectedMoeTop2ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(detail) => {
                write!(formatter, "MoeTop2 admission failed: {detail}")
            }
            Self::SourceIdentity { expected, actual } => write!(
                formatter,
                "MoeTop2 source bytes mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::Abi(detail) => write!(formatter, "MoeTop2 ABI mismatch: {detail}"),
            Self::Layout(detail) => {
                write!(formatter, "MoeTop2 layout mismatch: {detail}")
            }
            Self::PortableMir(detail) => {
                write!(formatter, "MoeTop2 portable MIR rejected: {detail}")
            }
            Self::PortableMirIdentity { expected, actual } => write!(
                formatter,
                "MoeTop2 complete reachable MIR closure mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::FnAbiIdentity { expected, actual } => write!(
                formatter,
                "MoeTop2 rustc FnAbi mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::TrustedDefinitions(detail) => write!(
                formatter,
                "MoeTop2 trusted definition closure rejected: {detail}"
            ),
            Self::CanonicalIr(detail) => {
                write!(
                    formatter,
                    "MoeTop2 canonical semantic IR rejected: {detail}"
                )
            }
            Self::SourceKirStructuralRecord(detail) => write!(
                formatter,
                "MoeTop2 source/FnAbi/MIR/KIR structural record failed: {detail}"
            ),
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("MoeTop2 frontend receipt was already consumed")
            }
            Self::ReceiptBinding(field) => write!(
                formatter,
                "MoeTop2 frontend receipt binding mismatch: {field}"
            ),
        }
    }
}

impl Error for CollectedMoeTop2ErrorV1 {}

pub(crate) fn authenticate_collected_moe_top2_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<MoeTop2FrontendReceiptV1, CollectedMoeTop2ErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics_identity = require_compiler_semantics(&observe_compiler_semantics(tcx))?;
    let root = exact_root(&collection.functions)?;
    require_registration(root)?;
    let source = observe_source_identity(tcx, root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi = require_fn_abi(tcx, root.instance)?;
    let trusted_definitions_identity = trusted_definitions_and_terminals_identity(tcx, collection)?;
    let target_identity = exact_target_identity()?;
    let profile_launch = exact_profile_launch()?;
    let contract = root
        .general_typed_contract
        .as_ref()
        .expect("layout checked contract");
    let imported = crate::mir_import::import_collection(tcx, collection)
        .map_err(|error| CollectedMoeTop2ErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            MOE_TOP2_V1_KERNEL_ID,
            &target_identity,
            contract.abi(),
            &profile_launch,
        ))
        .map_err(|error| CollectedMoeTop2ErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = *portable_mir_identity.as_bytes();
    if portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
        return Err(CollectedMoeTop2ErrorV1::PortableMirIdentity {
            expected: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
            actual: portable_mir_identity,
        });
    }
    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
        return Err(CollectedMoeTop2ErrorV1::Admission(format!(
            "root instance has noncanonical generated identity `{root_instance_identity}`"
        )));
    }
    let ir = moe_top2_v1_kernel_ir();
    let profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
    verify_moe_top2_v1(&ir, &profile)
        .map_err(|error| CollectedMoeTop2ErrorV1::CanonicalIr(error.to_string()))?;
    let frontend_contract_identity = sha256(
        root.frontend_contract
            .as_ref()
            .expect("registration checked frontend contract")
            .canonical_bytes(),
    );
    let mut authority = MoeTop2AuthorityV1 {
        source_identity: source.identity,
        source_namespace: MOE_TOP2_V1_NAMESPACE,
        compiler_crate_binding: compiler_crate_binding().as_bytes(),
        target: target.as_str().to_owned(),
        code_object_version: MOE_TOP2_CODE_OBJECT_VERSION_V1,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_identity,
        compiler_semantics_identity,
        fn_abi_identity: fn_abi.identity,
        trusted_definitions_identity,
        frontend_contract_identity,
        abi_identity: sha256(ABI_BINDING_V1),
        effects_identity: sha256(EFFECT_BINDING_V1),
        source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
        profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
        routing_identity: sha256(ROUTING_BINDING_V1),
        descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
        canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
        authority_identity: [0; 32],
    };
    authority.authority_identity = authority_identity(&authority);
    let validated_authority = validated_authority::validate_authority(&authority)?;
    let structural_inputs = seal_authenticated_live_inputs_v2(
        &source,
        &fn_abi,
        &imported,
        portable_mir_identity,
        &ir,
        &profile,
        validated_authority,
    )
    .map_err(|error| CollectedMoeTop2ErrorV1::SourceKirStructuralRecord(error.to_string()))?;
    let structural_record = produce_checked_moe_source_kir_structural_record_v2(structural_inputs)
        .map_err(|error| CollectedMoeTop2ErrorV1::SourceKirStructuralRecord(error.to_string()))?;
    Ok(MoeTop2FrontendReceiptV1 {
        authority: Some(authority),
        ir: Some(ir),
        profile: Some(profile),
        structural_record: Some(structural_record),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedMoeTop2ErrorV1> {
    if target != EXACT_MOE_TOP2_TARGET_V1 {
        return Err(CollectedMoeTop2ErrorV1::Admission(format!(
            "requires exact target `{EXACT_MOE_TOP2_TARGET_V1}`, found `{target}`"
        )));
    }
    if custom_llvm_pipeline {
        return Err(CollectedMoeTop2ErrorV1::Admission(
            "custom LLVM arguments or passes are forbidden".into(),
        ));
    }
    Ok(())
}

fn exact_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedMoeTop2ErrorV1> {
    let mut roots = functions
        .iter()
        .filter(|function| function.is_kernel_entry());
    let root = roots.next().ok_or_else(|| {
        CollectedMoeTop2ErrorV1::Admission("the exact MoeTop2 closure has no kernel root".into())
    })?;
    if roots.next().is_some() || functions.len() != 6 {
        return Err(CollectedMoeTop2ErrorV1::Admission(format!(
            "the exact MoE top-2 closure requires one root plus five reachable local/core helpers, found {} collected functions",
            functions.len()
        )));
    }
    Ok(root)
}

fn require_registration(root: &CollectedFunction<'_>) -> Result<(), CollectedMoeTop2ErrorV1> {
    let namespace = compiler_crate_binding();
    let expected_binding = derive_kernel_binding_id_v1(
        namespace,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        MOE_TOP2_V1_KERNEL_ID,
        MOE_TOP2_V1_KERNEL_ID,
    );
    if root.export_name != MOE_TOP2_V1_KERNEL_ID
        || root.logical_name.as_deref() != Some(MOE_TOP2_V1_KERNEL_ID)
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
        let observed_contract = root
            .frontend_contract
            .as_ref()
            .map(|value| encode_hex(value.canonical_bytes()))
            .unwrap_or_else(|| "absent".to_owned());
        return Err(CollectedMoeTop2ErrorV1::Admission(format!(
            "expected the unique ordinary #[kernel(typed)] MoE top-2 root with the reviewed wrapper-derived crate binding and required=max=64x1x1 contract; observed frontend contract {observed_contract}"
        )));
    }
    Ok(())
}

fn observe_source_identity(
    tcx: TyCtxt<'_>,
    root: &CollectedFunction<'_>,
) -> Result<RustcLoadedMoeTop2SourceV2, CollectedMoeTop2ErrorV1> {
    let source_file = tcx
        .sess
        .source_map()
        .lookup_source_file(tcx.def_span(root.instance.def_id()).lo());
    rustc_loaded_source_witness(&source_file)
}

fn rustc_loaded_source_witness(
    source_file: &SourceFile,
) -> Result<RustcLoadedMoeTop2SourceV2, CollectedMoeTop2ErrorV1> {
    let contents = source_file.src.as_ref().cloned().ok_or_else(|| {
        CollectedMoeTop2ErrorV1::Admission(
            "kernel root source was not retained in rustc's loaded SourceFile".into(),
        )
    })?;
    let namespace_declaration = format!("namespace = \"{}\"", encode_hex(&MOE_TOP2_V1_NAMESPACE));
    if contents
        .as_bytes()
        .windows(namespace_declaration.len())
        .filter(|window| *window == namespace_declaration.as_bytes())
        .count()
        != 1
    {
        return Err(CollectedMoeTop2ErrorV1::Admission(
            "exact source must contain the unique reviewed Phase A namespace declaration".into(),
        ));
    }
    let actual = sha256(contents.as_bytes());
    if actual != MOE_TOP2_V1_SOURCE_SHA256 {
        return Err(CollectedMoeTop2ErrorV1::SourceIdentity {
            expected: MOE_TOP2_V1_SOURCE_SHA256,
            actual,
        });
    }
    Ok(RustcLoadedMoeTop2SourceV2 {
        contents,
        identity: actual,
    })
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<(), CollectedMoeTop2ErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(CollectedMoeTop2ErrorV1::Abi(
            "kernel must be one nongeneric ordinary function item".into(),
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| CollectedMoeTop2ErrorV1::Abi("signature normalization failed".into()))?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 8
        || !is_shared_f32_slice(signature.inputs()[0])
        || signature.inputs()[1..]
            .iter()
            .any(|input| !is_disjoint_u32_slice(tcx, *input))
    {
        return Err(CollectedMoeTop2ErrorV1::Abi(format!(
            "expected safe Rust `(&[f32], DisjointSlice<u32> x 7) -> ()`, found `{signature}`"
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

fn is_disjoint_u32_slice(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, args) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::DisjointSlice)
        && args.len() == 2
        && args
            .first()
            .and_then(|value| value.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Uint(UintTy::U32)))
}

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedMoeTop2ErrorV1> {
    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| CollectedMoeTop2ErrorV1::Layout("General V3 contract is absent".into()))?;
    let actual = contract
        .arguments()
        .iter()
        .map(|value| value.kind())
        .collect::<Vec<_>>();
    if actual != ARGUMENT_KINDS_V1 {
        return Err(CollectedMoeTop2ErrorV1::Layout(format!(
            "expected argument kinds {ARGUMENT_KINDS_V1:?}, found {actual:?}"
        )));
    }
    if root
        .typed_layout_identities
        .as_ref()
        .map(|identities| identities.len())
        != Some(8)
    {
        return Err(CollectedMoeTop2ErrorV1::Layout(
            "eight compiler-derived argument identities are required".into(),
        ));
    }
    let abi = contract.abi();
    if abi.size() != u64::from(MOE_TOP2_V1_EXPLICIT_KERNARG_BYTES)
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 8
    {
        return Err(CollectedMoeTop2ErrorV1::Layout(format!(
            "expected ptr64 size-128 align-8 eight-field ABI, found {abi:?}"
        )));
    }
    // General V3 canonicalizes physical ABI fields positionally; the semantic
    // role names remain bound by `ABI_BINDING_V1` and the closed Kernel IR.
    let names = [
        "arg0", "arg1", "arg2", "arg3", "arg4", "arg5", "arg6", "arg7",
    ];
    let offsets = [0, 16, 32, 48, 64, 80, 96, 112];
    let sizes = [16; 8];
    let alignments = [8; 8];
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != names[index]
            || field.offset() != offsets[index]
            || field.size() != sizes[index]
            || field.alignment() != alignments[index]
            || field.type_identity() != contract.arguments()[index].type_identity()
        {
            return Err(CollectedMoeTop2ErrorV1::Layout(format!(
                "ABI field {index} name, offset, size, alignment, or type identity drifted: field={field:?}, argument={:?}",
                contract.arguments()[index]
            )));
        }
        let exact = match index {
            0 => {
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
            1..=7 => {
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
            return Err(CollectedMoeTop2ErrorV1::Layout(format!(
                "ABI field {index} access, ownership, address space, or kind drifted"
            )));
        }
    }
    let launch = contract.launch();
    if launch.rank() != 1
        || launch.block_size()
            != BlockSize::Exact(
                Dimensions::new(64, 1, 1)
                    .map_err(|error| CollectedMoeTop2ErrorV1::Layout(error.to_string()))?,
            )
        || launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1)
                .map_err(|error| CollectedMoeTop2ErrorV1::Layout(error.to_string()))?
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedMoeTop2ErrorV1::Layout(
            "source launch must be exact WG64 with one-dimensional grid and no LDS".into(),
        ));
    }
    Ok(())
}

fn require_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<ObservedMoeTop2FnAbiV1, CollectedMoeTop2ErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx
        .fn_abi_of_instance(query)
        .map_err(|error| CollectedMoeTop2ErrorV1::Abi(format!("FnAbi query failed: {error:?}")))?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 8
        || abi.args.len() != 8
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
        || abi.ret.layout.align.abi.bytes() != 1
    {
        return Err(CollectedMoeTop2ErrorV1::Abi(format!(
            "FnAbi header must be Rust(args=8)->unit, found {abi:?}"
        )));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_DOMAIN_V1);
    hash_checked_fn_abi_header(
        &mut digest,
        CheckedMoeTop2FnAbiHeaderV2 {
            calling_convention: FN_ABI_CALLING_CONVENTION_RUST_V2,
            c_variadic: abi.c_variadic,
            fixed_count: u64::from(abi.fixed_count),
            argument_count: u64::try_from(abi.args.len()).map_err(|_| {
                CollectedMoeTop2ErrorV1::Abi("FnAbi argument count overflowed u64".into())
            })?,
            can_unwind: abi.can_unwind,
            return_mode: FN_ABI_RETURN_MODE_IGNORE_V2,
            return_size: abi.ret.layout.size.bytes(),
            return_alignment: abi.ret.layout.align.abi.bytes(),
        },
    );
    let mut arguments = [MoeFnAbiArgumentStructuralProjectionV2 {
        size: 0,
        alignment: 0,
        pair_mode: false,
        first_pointee_bytes: 0,
        second_pointee_bytes: 0,
    }; 8];
    for (index, argument) in abi.args.iter().enumerate() {
        let expected_size = 16;
        if argument.layout.size.bytes() != expected_size || argument.layout.align.abi.bytes() != 8 {
            return Err(CollectedMoeTop2ErrorV1::Abi(format!(
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
                arguments[index] = MoeFnAbiArgumentStructuralProjectionV2 {
                    size: u16::try_from(argument.layout.size.bytes()).map_err(|_| {
                        CollectedMoeTop2ErrorV1::Abi(format!(
                            "FnAbi argument {index} size overflowed u16"
                        ))
                    })?,
                    alignment: u16::try_from(argument.layout.align.abi.bytes()).map_err(|_| {
                        CollectedMoeTop2ErrorV1::Abi(format!(
                            "FnAbi argument {index} alignment overflowed u16"
                        ))
                    })?,
                    pair_mode: true,
                    first_pointee_bytes: u32::try_from(first.pointee_size.bytes()).map_err(
                        |_| {
                            CollectedMoeTop2ErrorV1::Abi(format!(
                                "FnAbi argument {index} first pointee size overflowed u32"
                            ))
                        },
                    )?,
                    second_pointee_bytes: u32::try_from(second.pointee_size.bytes()).map_err(
                        |_| {
                            CollectedMoeTop2ErrorV1::Abi(format!(
                                "FnAbi argument {index} second pointee size overflowed u32"
                            ))
                        },
                    )?,
                };
            }
            _ => {
                return Err(CollectedMoeTop2ErrorV1::Abi(format!(
                    "FnAbi argument {index} pass mode drifted"
                )));
            }
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != RUSTC_FN_ABI_IDENTITY_V1 {
        return Err(CollectedMoeTop2ErrorV1::FnAbiIdentity {
            expected: RUSTC_FN_ABI_IDENTITY_V1,
            actual,
        });
    }
    Ok(ObservedMoeTop2FnAbiV1 {
        identity: actual,
        // The digest commits calling convention, variadic/fixed/actual counts,
        // unwind, ignored-return mode, return size/alignment, and each argument's
        // size/alignment, pair mode, and both components' regular bits,
        // extension, pointee size, and optional pointee alignment. This value
        // remains a bounded projection, not a complete FnAbi model.
        structural_projection: MoeFnAbiStructuralProjectionV2 {
            identity: actual,
            rust_calling_convention: abi.conv == CanonAbi::Rust,
            c_variadic: abi.c_variadic,
            fixed_count: u8::try_from(abi.fixed_count).map_err(|_| {
                CollectedMoeTop2ErrorV1::Abi("FnAbi fixed count overflowed u8".into())
            })?,
            can_unwind: abi.can_unwind,
            result_ignored: matches!(abi.ret.mode, PassMode::Ignore),
            result_size: u16::try_from(abi.ret.layout.size.bytes()).map_err(|_| {
                CollectedMoeTop2ErrorV1::Abi("FnAbi result size overflowed u16".into())
            })?,
            arguments,
        },
    })
}

fn hash_checked_fn_abi_header(digest: &mut Sha256, header: CheckedMoeTop2FnAbiHeaderV2) {
    hash_field(digest, &[header.calling_convention]);
    hash_field(digest, &[u8::from(header.c_variadic)]);
    hash_field(digest, &header.fixed_count.to_le_bytes());
    hash_field(digest, &header.argument_count.to_le_bytes());
    hash_field(digest, &[u8::from(header.can_unwind)]);
    hash_field(digest, &[header.return_mode]);
    hash_field(digest, &header.return_size.to_le_bytes());
    hash_field(digest, &header.return_alignment.to_le_bytes());
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerProviderIdentityV1 {
    crate_name: String,
    stable_crate_id: u64,
    // Used only to require one internally consistent rustc session. Device
    // definition identities never hash this path-sensitive observation.
    crate_hash_observation: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedDeviceDefinitionIdentityV3 {
    provider: CompilerProviderIdentityV1,
    cargo_metadata_build_observation: [u8; 32],
    source_closure_identity: [u8; 32],
    definition_source_identity: [u8; 32],
}

fn trusted_definitions_and_terminals_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<[u8; 32], CollectedMoeTop2ErrorV1> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, TRUSTED_DEFINITIONS_DOMAIN_V3);
    hash_field(&mut digest, COLLECTED_MOE_TOP2_PIPELINE_V1.as_bytes());
    let mut provider = None;
    for item in REQUIRED_TRUSTED_ITEMS_V1 {
        let definition = trusted_device_items::definition(tcx, *item).ok_or_else(|| {
            CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
                "missing exact diagnostic item `{}`",
                item.canonical_path()
            ))
        })?;
        if definition.is_local() || provider.is_some_and(|value| value != definition.krate) {
            return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
                "diagnostic item `{}` did not come from the single external device provider",
                item.canonical_path()
            )));
        }
        provider.get_or_insert(definition.krate);
        let identity = reviewed_device_definition_identity(tcx, definition)?;
        hash_device_definition(
            &mut digest,
            item.canonical_path(),
            &tcx.def_path_str(definition),
            tcx.def_path_hash(definition).local_hash().as_u64(),
            &identity,
        );
    }
    let provider = provider.ok_or_else(|| {
        CollectedMoeTop2ErrorV1::TrustedDefinitions(
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
                return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(
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
                CollectedMoeTop2ErrorV1::TrustedDefinitions(
                    "semantic-terminal call resolution failed".into(),
                )
            })?
            .map_or(*def_id, |instance| instance.def_id());
            if let Some(role) = classify_exact_moe_top2_compiler_intrinsic(tcx, resolved) {
                if observed_core_terminals.iter().all(|old| *old != role) {
                    observed_core_terminals.push(role);
                }
                continue;
            }
            let Some(item) = trusted_device_items::classify(tcx, resolved) else {
                continue;
            };
            if !REVIEWED_SEMANTIC_TERMINALS_V1.contains(&item) {
                return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
                    "unreviewed semantic terminal `{}` entered the exact MIR closure",
                    item.canonical_path()
                )));
            }
            if observed_terminals.iter().all(|(old, _)| *old != item) {
                observed_terminals.push((item, resolved));
            }
        }
    }
    if observed_core_terminals != [MoeTop2CompilerIntrinsicV1::FabsF32] {
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
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
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
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
            return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
                "semantic terminal `{}` came from unreviewed provider `{}`",
                expected.canonical_path(),
                terminal_provider.crate_name
            )));
        }
        let identity = reviewed_device_definition_identity(tcx, definition)?;
        hash_field(&mut digest, b"reviewed-semantic-terminal-v1");
        hash_device_definition(
            &mut digest,
            expected.canonical_path(),
            &tcx.def_path_str(definition),
            tcx.def_path_hash(definition).local_hash().as_u64(),
            &identity,
        );
    }
    let core_provider = tcx
        .lang_items()
        .get(rustc_hir::lang_items::LangItem::Sized)
        .ok_or_else(|| {
            CollectedMoeTop2ErrorV1::TrustedDefinitions(
                "pinned compiler omitted the core Sized lang item".into(),
            )
        })?
        .krate;
    let core_provider = compiler_provider(tcx, core_provider);
    if core_provider.crate_name != "core"
        || core_provider.stable_crate_id == 0
        || core_provider.crate_hash_observation == [0; 16]
    {
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(
            "pinned core provider identity is incomplete".into(),
        ));
    }
    let fabs_definition = observed_core_terminals
        .iter()
        .find_map(|role| {
            (*role == MoeTop2CompilerIntrinsicV1::FabsF32).then(|| {
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
                            classify_exact_moe_top2_compiler_intrinsic(tcx, *definition)
                                == Some(MoeTop2CompilerIntrinsicV1::FabsF32)
                        })
                    })
                })
            })
        })
        .flatten()
        .ok_or_else(|| {
            CollectedMoeTop2ErrorV1::TrustedDefinitions(
                "fabs semantic terminal definition was not retained".into(),
            )
        })?;
    if compiler_provider(tcx, fabs_definition.krate) != core_provider {
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(
            "fabs semantic terminal did not come from pinned core".into(),
        ));
    }
    hash_field(&mut digest, b"pinned-rustc-core-terminal-v1");
    hash_field(
        &mut digest,
        MoeTop2CompilerIntrinsicV1::FabsF32
            .canonical_path()
            .as_bytes(),
    );
    hash_field(&mut digest, tcx.def_path_str(fabs_definition).as_bytes());
    hash_field(
        &mut digest,
        &tcx.def_path_hash(fabs_definition)
            .local_hash()
            .as_u64()
            .to_le_bytes(),
    );
    hash_field(&mut digest, core_provider.crate_name.as_bytes());
    hash_field(&mut digest, &core_provider.stable_crate_id.to_le_bytes());
    hash_field(&mut digest, &core_provider.crate_hash_observation);
    let actual: [u8; 32] = digest.finalize().into();
    if actual != TRUSTED_TERMINAL_IDENTITY_V3 {
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(format!(
            "trusted-definition/semantic-terminal identity drifted: expected {}, found {}",
            encode_hex(&TRUSTED_TERMINAL_IDENTITY_V3),
            encode_hex(&actual)
        )));
    }
    Ok(actual)
}

fn compiler_provider(
    tcx: TyCtxt<'_>,
    crate_num: rustc_hir::def_id::CrateNum,
) -> CompilerProviderIdentityV1 {
    CompilerProviderIdentityV1 {
        crate_name: tcx.crate_name(crate_num).to_string(),
        stable_crate_id: tcx.stable_crate_id(crate_num).as_u64(),
        crate_hash_observation: tcx.crate_hash(crate_num).as_u128().to_le_bytes(),
    }
}

fn reviewed_device_definition_identity(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
) -> Result<ReviewedDeviceDefinitionIdentityV3, CollectedMoeTop2ErrorV1> {
    let observed =
        trusted_device_items::reviewed_workgroup_sync_provider_definition(tcx, definition)
            .map_err(CollectedMoeTop2ErrorV1::TrustedDefinitions)?;
    let provider = compiler_provider(tcx, definition.krate);
    if observed.crate_name != provider.crate_name
        || observed.stable_crate_id != provider.stable_crate_id
        || observed.crate_hash_observation != provider.crate_hash_observation
        || observed.cargo_metadata_build_observation == [0; 32]
        || observed.source_closure_identity == [0; 32]
        || observed.definition_source_identity == [0; 32]
    {
        return Err(CollectedMoeTop2ErrorV1::TrustedDefinitions(
            "reviewed device provider observation is incomplete".into(),
        ));
    }
    Ok(ReviewedDeviceDefinitionIdentityV3 {
        provider,
        cargo_metadata_build_observation: observed.cargo_metadata_build_observation,
        source_closure_identity: observed.source_closure_identity,
        definition_source_identity: observed.definition_source_identity,
    })
}

fn hash_device_definition(
    digest: &mut Sha256,
    role: &str,
    compiler_path: &str,
    local_def_path_hash: u64,
    identity: &ReviewedDeviceDefinitionIdentityV3,
) {
    hash_field(digest, b"reviewed-fe2o3-device-definition-v1");
    hash_field(digest, role.as_bytes());
    hash_field(digest, compiler_path.as_bytes());
    hash_field(digest, &local_def_path_hash.to_le_bytes());
    hash_field(digest, identity.provider.crate_name.as_bytes());
    hash_field(digest, &identity.provider.stable_crate_id.to_le_bytes());
    hash_field(digest, &identity.cargo_metadata_build_observation);
    hash_field(digest, &identity.source_closure_identity);
    hash_field(digest, &identity.definition_source_identity);
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
) -> Result<[u8; 32], CollectedMoeTop2ErrorV1> {
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
        return Err(CollectedMoeTop2ErrorV1::Admission(detail));
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
        return Err(CollectedMoeTop2ErrorV1::Admission(format!(
            "compiler semantics identity drifted: expected {}, found {}",
            encode_hex(&COMPILER_SEMANTICS_IDENTITY_V1),
            encode_hex(&actual)
        )));
    }
    Ok(actual)
}

fn exact_target_identity() -> Result<TargetIdentity, CollectedMoeTop2ErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| CollectedMoeTop2ErrorV1::Admission(error.to_string()))?,
        IdentityText::new(EXACT_MOE_TOP2_TARGET_V1)
            .map_err(|error| CollectedMoeTop2ErrorV1::Admission(error.to_string()))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Subgroup, Capability::AmdWave],
    )
    .map_err(|error| CollectedMoeTop2ErrorV1::Admission(error.to_string()))
}

fn exact_profile_launch() -> Result<LaunchContract, CollectedMoeTop2ErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedMoeTop2ErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| CollectedMoeTop2ErrorV1::Layout(error.to_string()))?,
        0,
        0,
    )
    .map_err(|error| CollectedMoeTop2ErrorV1::Layout(error.to_string()))
}

mod validated_authority {
    use super::*;

    pub(super) struct ValidatedMoeTop2AuthorityV1<'a> {
        authority: &'a MoeTop2AuthorityV1,
    }

    impl<'a> ValidatedMoeTop2AuthorityV1<'a> {
        pub(super) const fn authority(&self) -> &'a MoeTop2AuthorityV1 {
            self.authority
        }
    }

    pub(super) fn validate_authority(
        authority: &MoeTop2AuthorityV1,
    ) -> Result<ValidatedMoeTop2AuthorityV1<'_>, CollectedMoeTop2ErrorV1> {
        let field = if authority.source_identity != MOE_TOP2_V1_SOURCE_SHA256 {
            Some("source bytes")
        } else if authority.source_namespace != MOE_TOP2_V1_NAMESPACE {
            Some("source namespace")
        } else if authority.compiler_crate_binding != compiler_crate_binding().as_bytes() {
            Some("wrapper-derived compiler crate binding")
        } else if authority.target != EXACT_MOE_TOP2_TARGET_V1 {
            Some("target")
        } else if authority.code_object_version != MOE_TOP2_CODE_OBJECT_VERSION_V1 {
            Some("code object version")
        } else if authority.kernel_export != MOE_TOP2_V1_KERNEL_ID {
            Some("kernel export")
        } else if authority.root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
            Some("root instance identity")
        } else if authority.portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
            Some("complete reachable MIR closure")
        } else if authority.fn_abi_identity != RUSTC_FN_ABI_IDENTITY_V1 {
            Some("rustc FnAbi")
        } else if authority.compiler_semantics_identity != COMPILER_SEMANTICS_IDENTITY_V1
            || authority.trusted_definitions_identity != TRUSTED_TERMINAL_IDENTITY_V3
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
        } else if authority.routing_identity != sha256(ROUTING_BINDING_V1) {
            Some("closed deterministic routing semantics")
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
            return Err(CollectedMoeTop2ErrorV1::ReceiptBinding(field));
        }
        Ok(ValidatedMoeTop2AuthorityV1 { authority })
    }
}

fn authority_identity(authority: &MoeTop2AuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, &authority.source_identity);
    hash_field(&mut digest, &authority.source_namespace);
    hash_field(&mut digest, &authority.compiler_crate_binding);
    hash_field(&mut digest, authority.target.as_bytes());
    hash_field(&mut digest, &authority.code_object_version.to_le_bytes());
    hash_field(&mut digest, authority.kernel_export.as_bytes());
    hash_field(&mut digest, authority.root_instance_identity.as_bytes());
    hash_field(&mut digest, &authority.portable_mir_identity);
    hash_field(&mut digest, &authority.compiler_semantics_identity);
    hash_field(&mut digest, &authority.fn_abi_identity);
    hash_field(&mut digest, &authority.trusted_definitions_identity);
    hash_field(&mut digest, &authority.frontend_contract_identity);
    hash_field(&mut digest, &authority.abi_identity);
    hash_field(&mut digest, &authority.effects_identity);
    hash_field(&mut digest, &authority.source_launch_identity);
    hash_field(&mut digest, &authority.profile_launch_identity);
    hash_field(&mut digest, &authority.routing_identity);
    hash_field(&mut digest, &authority.descriptor_identity);
    hash_field(&mut digest, &authority.canonical_ir_identity);
    hash_field(&mut digest, &authority.correspondence_identity);
    digest.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn compiler_crate_binding() -> CrateBindingIdV1 {
    CrateBindingIdV1::from_hex(REVIEWED_COMPILER_CRATE_BINDING)
        .expect("reviewed MoeTop2 compiler crate binding is canonical")
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
fn exact_authority_for_test() -> MoeTop2AuthorityV1 {
    let mut authority = MoeTop2AuthorityV1 {
        source_identity: MOE_TOP2_V1_SOURCE_SHA256,
        source_namespace: MOE_TOP2_V1_NAMESPACE,
        compiler_crate_binding: compiler_crate_binding().as_bytes(),
        target: EXACT_MOE_TOP2_TARGET_V1.into(),
        code_object_version: 6,
        kernel_export: MOE_TOP2_V1_KERNEL_ID.into(),
        root_instance_identity: REVIEWED_ROOT_INSTANCE_IDENTITY.into(),
        portable_mir_identity: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
        compiler_semantics_identity: COMPILER_SEMANTICS_IDENTITY_V1,
        fn_abi_identity: RUSTC_FN_ABI_IDENTITY_V1,
        trusted_definitions_identity: TRUSTED_TERMINAL_IDENTITY_V3,
        frontend_contract_identity: sha256(EXACT_FRONTEND_CONTRACT_V1),
        abi_identity: sha256(ABI_BINDING_V1),
        effects_identity: sha256(EFFECT_BINDING_V1),
        source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
        profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
        routing_identity: sha256(ROUTING_BINDING_V1),
        descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
        canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
        authority_identity: [0; 32],
    };
    authority.authority_identity = authority_identity(&authority);
    authority
}

#[cfg(test)]
pub(crate) fn exact_frontend_receipt_for_test() -> MoeTop2FrontendReceiptV1 {
    let authority = exact_authority_for_test();
    let validated_authority = validated_authority::validate_authority(&authority)
        .expect("synthetic exact authority validates");
    let structural_record =
        moe_top2_source_kir_correspondence::checked_record_for_test_authority(validated_authority);
    MoeTop2FrontendReceiptV1 {
        authority: Some(authority),
        ir: Some(moe_top2_v1_kernel_ir()),
        profile: Some(MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6()),
        structural_record: Some(structural_record),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_span::source_map::{FilePathMapping, SourceMap};

    use crate::test_temp_dir::TestTempDir;

    fn receipt() -> MoeTop2FrontendReceiptV1 {
        exact_frontend_receipt_for_test()
    }

    fn exact_fn_abi_header() -> CheckedMoeTop2FnAbiHeaderV2 {
        CheckedMoeTop2FnAbiHeaderV2 {
            calling_convention: FN_ABI_CALLING_CONVENTION_RUST_V2,
            c_variadic: false,
            fixed_count: 8,
            argument_count: 8,
            can_unwind: true,
            return_mode: FN_ABI_RETURN_MODE_IGNORE_V2,
            return_size: 0,
            return_alignment: 1,
        }
    }

    fn fn_abi_header_commitment(header: CheckedMoeTop2FnAbiHeaderV2) -> [u8; 32] {
        let mut digest = Sha256::new();
        hash_field(&mut digest, FN_ABI_DOMAIN_V1);
        hash_checked_fn_abi_header(&mut digest, header);
        digest.finalize().into()
    }

    #[test]
    fn rustc_loaded_source_witness_does_not_reopen_replaced_path() {
        let directory = TestTempDir::create("fe2o3-moe-rustc-source-witness");
        let path = directory.path().join("kernel.rs");
        let exact = include_str!("../../../examples/moe_top2_v1/src/kernel.rs");
        std::fs::write(&path, exact).expect("write exact source before rustc load");

        let source_map = SourceMap::new(FilePathMapping::empty());
        let loaded = source_map
            .load_file(&path)
            .expect("load exact source into rustc SourceMap");
        std::fs::write(&path, "// hostile replacement after parse-time load\n")
            .expect("replace backing source path");

        let witness = rustc_loaded_source_witness(&loaded)
            .expect("authenticate rustc-loaded parse-time source");
        assert_eq!(witness.contents.as_str(), exact);
        assert_eq!(witness.identity, MOE_TOP2_V1_SOURCE_SHA256);
        assert_ne!(
            std::fs::read(&path).expect("read replaced backing source"),
            witness.contents.as_bytes()
        );
    }

    #[test]
    fn receipt_selects_only_the_exact_semantic_profile_once() {
        let mut value = receipt();
        let admitted = value.consume().unwrap();
        assert_eq!(admitted.semantic_summary(), (8, 4, 2, 4, 10));
        assert_eq!(admitted.profile().grid, [1, 1, 1]);
        assert_eq!(
            value.consume(),
            Err(CollectedMoeTop2ErrorV1::ReceiptAlreadyConsumed)
        );
    }

    #[test]
    fn authority_and_checked_fn_abi_header_mutations_fail_closed() {
        let baseline = fn_abi_header_commitment(exact_fn_abi_header());
        let header_mutations: [fn(&mut CheckedMoeTop2FnAbiHeaderV2); 8] = [
            |value| value.calling_convention ^= 1,
            |value| value.c_variadic = true,
            |value| value.fixed_count += 1,
            |value| value.argument_count += 1,
            |value| value.can_unwind = false,
            |value| value.return_mode ^= 1,
            |value| value.return_size += 1,
            |value| value.return_alignment += 1,
        ];
        for mutate in header_mutations {
            let mut header = exact_fn_abi_header();
            mutate(&mut header);
            assert_ne!(fn_abi_header_commitment(header), baseline);
        }

        let mut rehashed_but_invalid = exact_authority_for_test();
        rehashed_but_invalid.fn_abi_identity[0] ^= 1;
        rehashed_but_invalid.authority_identity = authority_identity(&rehashed_but_invalid);
        assert_ne!(rehashed_but_invalid.authority_identity, [0; 32]);
        assert!(matches!(
            validated_authority::validate_authority(&rehashed_but_invalid),
            Err(CollectedMoeTop2ErrorV1::ReceiptBinding("rustc FnAbi"))
        ));

        let exact = exact_authority_for_test();
        let validated = validated_authority::validate_authority(&exact)
            .expect("exact authority produces the opaque validation token");
        assert_eq!(
            validated.authority().authority_identity,
            exact.authority_identity
        );

        let mutations: Vec<fn(&mut MoeTop2AuthorityV1)> = vec![
            |value| value.source_identity[0] ^= 1,
            |value| value.source_namespace[0] ^= 1,
            |value| value.compiler_crate_binding[0] ^= 1,
            |value| value.target.push('+'),
            |value| value.code_object_version = 5,
            |value| value.kernel_export.push('_'),
            |value| value.root_instance_identity.push('_'),
            |value| value.portable_mir_identity[0] ^= 1,
            |value| value.compiler_semantics_identity[0] ^= 1,
            |value| value.fn_abi_identity[0] ^= 1,
            |value| value.trusted_definitions_identity[0] ^= 1,
            |value| value.frontend_contract_identity[0] ^= 1,
            |value| value.abi_identity[0] ^= 1,
            |value| value.effects_identity[0] ^= 1,
            |value| value.source_launch_identity[0] ^= 1,
            |value| value.profile_launch_identity[0] ^= 1,
            |value| value.routing_identity[0] ^= 1,
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
                Err(CollectedMoeTop2ErrorV1::ReceiptBinding(_))
            ));
        }
    }

    #[test]
    fn canonical_ir_and_profile_substitutions_fail_after_source_authentication() {
        let mut ir = receipt();
        ir.ir.as_mut().unwrap().routing.swap(3, 4);
        assert!(matches!(
            ir.consume(),
            Err(CollectedMoeTop2ErrorV1::CanonicalIr(_))
        ));

        let mut profile = receipt();
        profile.profile.as_mut().unwrap().code_object_version = 5;
        assert!(matches!(
            profile.consume(),
            Err(CollectedMoeTop2ErrorV1::CanonicalIr(_))
        ));
    }
}
