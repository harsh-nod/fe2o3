//! Exact compiler authentication for the ordinary workgroup-sync sources.
//!
//! The LDS reduction and scoped atomic profiles are deliberately separate.
//! Each authenticates exact attributed source bytes, the wrapper/session
//! registration, rustc FnAbi, trusted definitions, and the complete reachable
//! portable-MIR closure before selecting a closed semantic sidecar. Selection
//! is reviewed correspondence, not generic lowering or a refinement proof.
//! Reviewed semantic terminals deliberately replace a small set of provider
//! bodies in that closure. Their exact role, compiler path, crate-local
//! definition hash, reviewed provider source, and build observation are frozen
//! here; this authenticates the review boundary but does not prove
//! source-to-terminal refinement.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    Capability, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Dimensions,
    Endianness, IdentityText, LaunchContract, Mutability as ArtifactMutability, Name, PointerWidth,
    RustScalarElementTypeV1, ScalarType, TargetIdentity, TypeIdentity,
};
use fe2o3_kernel_ir::{
    LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES, LDS_REDUCTION_V1_KERNEL_ID,
    LDS_REDUCTION_V1_NAMESPACE, LDS_REDUCTION_V1_SOURCE_SHA256, LdsReductionKernelIrV1,
    LdsReductionProfileV1, SCOPED_ATOMIC_V1_KERNEL_ID, SCOPED_ATOMIC_V1_NAMESPACE,
    SCOPED_ATOMIC_V1_SOURCE_SHA256, ScopedAtomicKernelIrV1, ScopedAtomicProfileV1,
    lds_reduction_v1_kernel_ir, scoped_atomic_v1_kernel_ir, verify_lds_reduction_v1,
    verify_scoped_atomic_v1,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{LangItem, Mutability, Safety};
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{InstanceKind, IntTy, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectionResult, TypedKernelProfile};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::semantic_features::WorkgroupSyncCompilerIntrinsicV1;
use crate::trusted_device_items::{self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem};

pub(crate) const COLLECTED_LDS_REDUCTION_PIPELINE_V1: &str = "collected-lds-reduction-v1";
pub(crate) const COLLECTED_SCOPED_ATOMIC_PIPELINE_V1: &str = "collected-scoped-atomic-v1";
pub(crate) const EXACT_WORKGROUP_SYNC_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const WORKGROUP_SYNC_CODE_OBJECT_VERSION_V1: u16 = 6;

const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const WORKSPACE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

const LDS_COMPILER_CRATE_BINDING: &str =
    "fd63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da";
const ATOMIC_COMPILER_CRATE_BINDING: &str =
    "dede4079399a3df33da7bcc9fc46bc84c3ab329642fa27241feaf10aff06388e";
const LDS_REVIEWED_CRATE_NAME: &str = "fe2o3_collected_lds_reduction_v1_fixture";
const ATOMIC_REVIEWED_CRATE_NAME: &str = "fe2o3_collected_scoped_atomic_v1_fixture";
const LDS_REVIEWED_METADATA: &str = "fe2o3-lds-reduction-v1-reviewed";
const ATOMIC_REVIEWED_METADATA: &str = "fe2o3-scoped-atomic-v1-reviewed";
const LDS_SOURCE_REMAP: &str = "/fe2o3-reviewed-workspace/lds-reduction-v1.rs";
const ATOMIC_SOURCE_REMAP: &str = "/fe2o3-reviewed-workspace/scoped-atomic-v1.rs";
const LDS_ROOT_INSTANCE_IDENTITY: &str =
    "__fe2o3_host_kernel_v1_04e227769c2f01cbf7bb7be7531177499aeb78b43e92b3e4b2687c2883920b61";
const ATOMIC_ROOT_INSTANCE_IDENTITY: &str =
    "__fe2o3_host_kernel_v1_95ec6a6b666037ee74a92b6c87cfd3efc868913ca1d5d1e95d41671dd4ccb9b7";

const AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.workgroup-sync.source-authority.v1";
const FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3.workgroup-sync.rustc-fn-abi.v1";
const TRUSTED_DEFINITIONS_DOMAIN_V3: &[u8] =
    b"fe2o3.workgroup-sync.trusted-definitions-and-semantic-terminals.v3";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.workgroup-sync.compiler-semantics.v1";
const LDS_ABI_BINDING_V1: &[u8] = b"ptr64;size=40;align=8;values@0:16:8:slice-i32:shared-readonly;epoch@16:4:4:u32:value;output@24:16:8:slice-i32:unique-readwrite";
const LDS_EFFECT_BINDING_V1: &[u8] = b"one-linear-lds-allocation:i32x64:256-bytes:align4:no-escape;all-64-threads-convergent;lane-publish;publish-read-barrier;read;read-reuse-barrier;lane0-only-output";
const LDS_RESOURCE_BINDING_V1: &[u8] = b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=256;maximum-dynamic-lds=256;cov6-hidden-dynamic-lds-size@relative120:field4:required-value256;allocation-count=1";
const LDS_CANONICAL_IR_BINDING_V1: &[u8] = b"fe2o3::workgroup_lds_reduction_v1;exact-i32x64-scratch;epochs=uninitialized,lane-initialized,published,read,reusable;barriers=publish-read,read-reuse;output=lane0";
const ATOMIC_ABI_BINDING_V1: &[u8] = b"ptr64;size=40;align=8;values@0:16:8:slice-u32:shared-readonly;eligible@16:16:8:slice-u32:shared-readonly;target@32:8:8:global-mut-u32:host-unique:device-shared-atomic";
const ATOMIC_EFFECT_BINDING_V1: &[u8] = b"eligible-lane-exactly-once;fetch-add-u32;ordering=relaxed;scope=system;address-space=global;one-live-aligned-atomic;mathematical-sum-fits-u32";
const ATOMIC_RESOURCE_BINDING_V1: &[u8] = b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=0;maximum-dynamic-lds=0;cov6-hidden-dynamic-lds-size=absent;capability=atomics";
const ATOMIC_CANONICAL_IR_BINDING_V1: &[u8] = b"fe2o3::scoped_atomic_add_v1;conditional-nonzero-eligibility;fetch-add-u32-relaxed-system-global;unique-host-borrow;lanes-alias-one-atomic";
const CORRESPONDENCE_BINDING_V1: &[u8] = b"exact attributed source plus wrapper/session registration, exact rustc FnAbi, frozen trusted definitions, identity-bound reviewed semantic terminals, and complete reachable portable-MIR modulo those terminals select a closed semantic sidecar;reviewed correspondence only;not generic lowering, terminal-body refinement, or a compiler-refinement proof";

// Filled from the pinned compiler fixtures after path-independent portable-MIR import.
const LDS_PORTABLE_MIR_IDENTITY_V1: [u8; 32] = [
    0x20, 0xd5, 0x49, 0x5b, 0x23, 0x66, 0x24, 0xc5, 0x1a, 0x67, 0x87, 0xd9, 0x95, 0x56, 0x94, 0x56,
    0xb1, 0xa6, 0xbb, 0xfc, 0x7c, 0x70, 0xe5, 0x43, 0x69, 0x4d, 0xeb, 0xc6, 0x2b, 0xeb, 0x46, 0xb1,
];
const ATOMIC_PORTABLE_MIR_IDENTITY_V1: [u8; 32] = [
    0x52, 0x1d, 0xec, 0x6e, 0x8e, 0x00, 0xb3, 0x8a, 0x4c, 0x47, 0x9c, 0xf3, 0xb9, 0x3d, 0x51, 0x54,
    0x43, 0x18, 0xcd, 0x2b, 0xac, 0xe9, 0xb0, 0x8c, 0x56, 0xe2, 0xd6, 0xaf, 0x57, 0xab, 0x37, 0xf5,
];
const LDS_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0xb3, 0x84, 0x04, 0x57, 0xdb, 0x66, 0x5f, 0x11, 0x4c, 0xae, 0xff, 0x92, 0xa4, 0xc7, 0xdd, 0xbe,
    0x63, 0x88, 0xac, 0x14, 0xbe, 0xc4, 0x8c, 0x29, 0x77, 0xc9, 0xa6, 0x21, 0x16, 0x81, 0x40, 0xc6,
];
const ATOMIC_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0xfa, 0xd7, 0x32, 0x25, 0x2d, 0xa6, 0x44, 0xac, 0xb7, 0xa3, 0x8f, 0x09, 0x13, 0xe0, 0x62, 0x46,
    0x12, 0x09, 0x3a, 0x7d, 0x98, 0x29, 0x42, 0x49, 0x7c, 0x3d, 0xe4, 0xda, 0x4f, 0x4b, 0xc8, 0x2f,
];
const LDS_COMPILER_SEMANTICS_IDENTITY_V1: [u8; 32] = [
    0x1c, 0x9f, 0xfd, 0xb9, 0x49, 0xc2, 0x18, 0xc2, 0xca, 0xd9, 0x87, 0x55, 0x89, 0x57, 0xa2, 0x71,
    0x9a, 0xf4, 0x92, 0x34, 0x91, 0x98, 0xbe, 0x95, 0xa9, 0x43, 0xf8, 0x46, 0x91, 0x05, 0xfd, 0xf2,
];
const ATOMIC_COMPILER_SEMANTICS_IDENTITY_V1: [u8; 32] = [
    0xbc, 0xf7, 0xe8, 0x74, 0xdb, 0x23, 0x61, 0x57, 0xdd, 0x6a, 0x8d, 0x8d, 0x76, 0xc6, 0x9b, 0x69,
    0x04, 0x17, 0x3e, 0xfe, 0xb5, 0x4f, 0x89, 0x05, 0xb9, 0xae, 0x1d, 0x48, 0x10, 0xca, 0x7b, 0x76,
];

// Frozen after observing the pinned fixture dependency graph in independent
// canonical workspaces. These values bind every trusted definition and
// reviewed semantic terminal without rustc's path-source crate hash.
const LDS_TRUSTED_TERMINAL_IDENTITY_V3: [u8; 32] = [
    0x50, 0x97, 0xff, 0x92, 0xf4, 0x88, 0x1d, 0x71, 0x17, 0x18, 0x29, 0x30, 0x84, 0x8d, 0x55, 0xab,
    0x78, 0x1e, 0xe6, 0x82, 0x24, 0xe1, 0xac, 0x78, 0x9e, 0xbf, 0x85, 0xf8, 0xbd, 0x41, 0x98, 0xcf,
];
const ATOMIC_TRUSTED_TERMINAL_IDENTITY_V3: [u8; 32] = [
    0x20, 0xa0, 0x07, 0x6e, 0x0e, 0xe9, 0xeb, 0x4e, 0x8d, 0xd9, 0x0e, 0x60, 0x1b, 0x36, 0x8f, 0xf3,
    0x95, 0x78, 0x5d, 0xfe, 0xf1, 0xfd, 0x5c, 0x80, 0x6d, 0x13, 0x18, 0x74, 0x16, 0x75, 0xe8, 0x14,
];

const LDS_ARGUMENT_KINDS_V1: [GeneralTypedArgumentKindV3; 3] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::I32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::I32),
];

const LDS_TRUSTED_ITEMS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::DisjointSlice,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::WorkgroupLdsScope,
    TrustedDeviceItem::DynamicLdsExactFromCompiler,
    TrustedDeviceItem::Gfx942CollectivesContext,
    TrustedDeviceItem::Gfx942CollectivesFromCompiler,
    TrustedDeviceItem::Gfx942WorkgroupReduceSum,
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];
const ATOMIC_TRUSTED_ITEMS_V1: &[TrustedDeviceItem] = &[
    TrustedDeviceItem::DeviceGlobalMutPtr,
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkgroupSyncProfileKindV1 {
    LdsReduction,
    ScopedAtomic,
}

impl WorkgroupSyncProfileKindV1 {
    const fn pipeline(self) -> &'static str {
        match self {
            Self::LdsReduction => COLLECTED_LDS_REDUCTION_PIPELINE_V1,
            Self::ScopedAtomic => COLLECTED_SCOPED_ATOMIC_PIPELINE_V1,
        }
    }

    const fn kernel(self) -> &'static str {
        match self {
            Self::LdsReduction => LDS_REDUCTION_V1_KERNEL_ID,
            Self::ScopedAtomic => SCOPED_ATOMIC_V1_KERNEL_ID,
        }
    }

    const fn source_identity(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_REDUCTION_V1_SOURCE_SHA256,
            Self::ScopedAtomic => SCOPED_ATOMIC_V1_SOURCE_SHA256,
        }
    }

    const fn namespace(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_REDUCTION_V1_NAMESPACE,
            Self::ScopedAtomic => SCOPED_ATOMIC_V1_NAMESPACE,
        }
    }

    const fn crate_name(self) -> &'static str {
        match self {
            Self::LdsReduction => LDS_REVIEWED_CRATE_NAME,
            Self::ScopedAtomic => ATOMIC_REVIEWED_CRATE_NAME,
        }
    }

    const fn metadata(self) -> &'static str {
        match self {
            Self::LdsReduction => LDS_REVIEWED_METADATA,
            Self::ScopedAtomic => ATOMIC_REVIEWED_METADATA,
        }
    }

    const fn source_remap(self) -> &'static str {
        match self {
            Self::LdsReduction => LDS_SOURCE_REMAP,
            Self::ScopedAtomic => ATOMIC_SOURCE_REMAP,
        }
    }

    const fn root_instance_identity(self) -> &'static str {
        match self {
            Self::LdsReduction => LDS_ROOT_INSTANCE_IDENTITY,
            Self::ScopedAtomic => ATOMIC_ROOT_INSTANCE_IDENTITY,
        }
    }

    const fn portable_mir(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_PORTABLE_MIR_IDENTITY_V1,
            Self::ScopedAtomic => ATOMIC_PORTABLE_MIR_IDENTITY_V1,
        }
    }

    const fn fn_abi(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_FN_ABI_IDENTITY_V1,
            Self::ScopedAtomic => ATOMIC_FN_ABI_IDENTITY_V1,
        }
    }

    const fn compiler_semantics(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_COMPILER_SEMANTICS_IDENTITY_V1,
            Self::ScopedAtomic => ATOMIC_COMPILER_SEMANTICS_IDENTITY_V1,
        }
    }

    const fn trusted_items(self) -> &'static [TrustedDeviceItem] {
        match self {
            Self::LdsReduction => LDS_TRUSTED_ITEMS_V1,
            Self::ScopedAtomic => ATOMIC_TRUSTED_ITEMS_V1,
        }
    }

    const fn compiler_terminals(self) -> &'static [WorkgroupSyncCompilerIntrinsicV1] {
        use WorkgroupSyncCompilerIntrinsicV1 as Terminal;
        match self {
            Self::LdsReduction => &[
                Terminal::ThreadIdxX,
                Terminal::ThreadIdxY,
                Terminal::ThreadIdxZ,
                Terminal::BlockIdxX,
                Terminal::BlockIdxY,
                Terminal::BlockIdxZ,
                Terminal::BlockDimX,
                Terminal::BlockDimY,
                Terminal::BlockDimZ,
                Terminal::LaunchExtent1d,
                Terminal::ScratchFromDynamicLds,
                Terminal::ColdPath,
            ],
            Self::ScopedAtomic => &[Terminal::ThreadIdxX, Terminal::AtomicXadd],
        }
    }

    const fn trusted_terminal_identity(self) -> [u8; 32] {
        match self {
            Self::LdsReduction => LDS_TRUSTED_TERMINAL_IDENTITY_V3,
            Self::ScopedAtomic => ATOMIC_TRUSTED_TERMINAL_IDENTITY_V3,
        }
    }

    const fn abi_binding(self) -> &'static [u8] {
        match self {
            Self::LdsReduction => LDS_ABI_BINDING_V1,
            Self::ScopedAtomic => ATOMIC_ABI_BINDING_V1,
        }
    }

    const fn effect_binding(self) -> &'static [u8] {
        match self {
            Self::LdsReduction => LDS_EFFECT_BINDING_V1,
            Self::ScopedAtomic => ATOMIC_EFFECT_BINDING_V1,
        }
    }

    const fn resource_binding(self) -> &'static [u8] {
        match self {
            Self::LdsReduction => LDS_RESOURCE_BINDING_V1,
            Self::ScopedAtomic => ATOMIC_RESOURCE_BINDING_V1,
        }
    }

    const fn canonical_ir_binding(self) -> &'static [u8] {
        match self {
            Self::LdsReduction => LDS_CANONICAL_IR_BINDING_V1,
            Self::ScopedAtomic => ATOMIC_CANONICAL_IR_BINDING_V1,
        }
    }
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
struct WorkgroupSyncAuthorityV1 {
    kind: WorkgroupSyncProfileKindV1,
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
    resources_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    correspondence_identity: [u8; 32],
    authority_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
enum SelectedWorkgroupSyncProfileV1 {
    Lds(LdsReductionKernelIrV1, LdsReductionProfileV1),
    Atomic(ScopedAtomicKernelIrV1, ScopedAtomicProfileV1),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct WorkgroupSyncFrontendReceiptV1 {
    authority: Option<WorkgroupSyncAuthorityV1>,
    selected: Option<SelectedWorkgroupSyncProfileV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedWorkgroupSyncProfileV1 {
    kind: WorkgroupSyncProfileKindV1,
    source_authority_identity: [u8; 32],
    descriptor_identity: [u8; 32],
}

impl WorkgroupSyncFrontendReceiptV1 {
    fn authority(&self) -> &WorkgroupSyncAuthorityV1 {
        self.authority
            .as_ref()
            .expect("unconsumed workgroup-sync receipt")
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

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedWorkgroupSyncProfileV1, CollectedWorkgroupSyncErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedWorkgroupSyncErrorV1::ReceiptAlreadyConsumed)?;
        let selected = self
            .selected
            .take()
            .ok_or(CollectedWorkgroupSyncErrorV1::ReceiptAlreadyConsumed)?;
        validate_authority(&authority)?;
        match selected {
            SelectedWorkgroupSyncProfileV1::Lds(ir, profile) => {
                if authority.kind != WorkgroupSyncProfileKindV1::LdsReduction {
                    return Err(CollectedWorkgroupSyncErrorV1::ReceiptBinding(
                        "profile kind",
                    ));
                }
                verify_lds_reduction_v1(&ir, &profile).map_err(|error| {
                    CollectedWorkgroupSyncErrorV1::CanonicalIr(error.to_string())
                })?;
            }
            SelectedWorkgroupSyncProfileV1::Atomic(ir, profile) => {
                if authority.kind != WorkgroupSyncProfileKindV1::ScopedAtomic {
                    return Err(CollectedWorkgroupSyncErrorV1::ReceiptBinding(
                        "profile kind",
                    ));
                }
                verify_scoped_atomic_v1(&ir, &profile).map_err(|error| {
                    CollectedWorkgroupSyncErrorV1::CanonicalIr(error.to_string())
                })?;
            }
        }
        Ok(AuthenticatedWorkgroupSyncProfileV1 {
            kind: authority.kind,
            source_authority_identity: authority.authority_identity,
            descriptor_identity: authority.resources_identity,
        })
    }
}

impl AuthenticatedWorkgroupSyncProfileV1 {
    pub(crate) const fn kind(&self) -> WorkgroupSyncProfileKindV1 {
        self.kind
    }

    pub(crate) fn source_authority_hex(&self) -> String {
        encode_hex(&self.source_authority_identity)
    }

    pub(crate) fn descriptor_hex(&self) -> String {
        encode_hex(&self.descriptor_identity)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CollectedWorkgroupSyncErrorV1 {
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

impl fmt::Display for CollectedWorkgroupSyncErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(detail) => {
                write!(formatter, "workgroup-sync admission failed: {detail}")
            }
            Self::SourceIdentity { expected, actual } => write!(
                formatter,
                "source bytes mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::Abi(detail) => write!(formatter, "workgroup-sync ABI mismatch: {detail}"),
            Self::Layout(detail) => write!(formatter, "workgroup-sync layout mismatch: {detail}"),
            Self::PortableMir(detail) => write!(formatter, "portable MIR rejected: {detail}"),
            Self::PortableMirIdentity { expected, actual } => write!(
                formatter,
                "complete reachable portable-MIR closure mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::FnAbiIdentity { expected, actual } => write!(
                formatter,
                "rustc FnAbi mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::TrustedDefinitions(detail) => {
                write!(formatter, "trusted definition closure rejected: {detail}")
            }
            Self::CanonicalIr(detail) => {
                write!(formatter, "canonical semantic IR rejected: {detail}")
            }
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("workgroup-sync receipt already consumed")
            }
            Self::ReceiptBinding(field) => write!(formatter, "receipt binding mismatch: {field}"),
        }
    }
}

impl Error for CollectedWorkgroupSyncErrorV1 {}

pub(crate) fn quarantine_scoped_atomic_general_contract(
    logical_name: &str,
    export_name: &str,
) -> bool {
    std::env::var("FE2O3_CODEGEN_PIPELINE").as_deref() == Ok(COLLECTED_SCOPED_ATOMIC_PIPELINE_V1)
        && logical_name == SCOPED_ATOMIC_V1_KERNEL_ID
        && export_name == SCOPED_ATOMIC_V1_KERNEL_ID
}

pub(crate) fn is_exact_workgroup_sync_compiler_intrinsic(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::DefId,
) -> bool {
    classify_exact_workgroup_sync_compiler_intrinsic(tcx, def_id)
        .is_some_and(|terminal| !terminal.is_rustc_intrinsic())
}

pub(crate) fn is_exact_workgroup_sync_rustc_intrinsic(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::DefId,
) -> bool {
    classify_exact_workgroup_sync_compiler_intrinsic(tcx, def_id)
        .is_some_and(WorkgroupSyncCompilerIntrinsicV1::is_rustc_intrinsic)
}

pub(crate) fn classify_exact_workgroup_sync_compiler_intrinsic(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::DefId,
) -> Option<WorkgroupSyncCompilerIntrinsicV1> {
    use WorkgroupSyncCompilerIntrinsicV1 as Intrinsic;

    let pipeline = std::env::var("FE2O3_CODEGEN_PIPELINE");
    if !matches!(
        pipeline.as_deref(),
        Ok(COLLECTED_LDS_REDUCTION_PIPELINE_V1) | Ok(COLLECTED_SCOPED_ATOMIC_PIPELINE_V1)
    ) || def_id.is_local()
    {
        return None;
    }
    let path = tcx.def_path_str(def_id);
    if path.ends_with("::intrinsics::cold_path") {
        return Some(Intrinsic::ColdPath);
    }
    if pipeline.as_deref() == Ok(COLLECTED_SCOPED_ATOMIC_PIPELINE_V1)
        && path.ends_with("::intrinsics::atomic_xadd")
    {
        return Some(Intrinsic::AtomicXadd);
    }
    let provider = trusted_device_items::definition(
        tcx,
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
    )?;
    if provider.krate != def_id.krate {
        return None;
    }
    [
        ("::thread::thread_idx_x", Intrinsic::ThreadIdxX),
        ("::thread::thread_idx_y", Intrinsic::ThreadIdxY),
        ("::thread::thread_idx_z", Intrinsic::ThreadIdxZ),
        ("::thread::block_idx_x", Intrinsic::BlockIdxX),
        ("::thread::block_idx_y", Intrinsic::BlockIdxY),
        ("::thread::block_idx_z", Intrinsic::BlockIdxZ),
        ("::thread::block_dim_x", Intrinsic::BlockDimX),
        ("::thread::block_dim_y", Intrinsic::BlockDimY),
        ("::thread::block_dim_z", Intrinsic::BlockDimZ),
        ("::thread::launch_extent_1d", Intrinsic::LaunchExtent1d),
    ]
    .into_iter()
    .find_map(|(suffix, item)| path.ends_with(suffix).then_some(item))
    .or_else(|| {
        (path.contains("::WorkgroupCollectiveScratch") && path.ends_with("::from_dynamic_lds"))
            .then_some(Intrinsic::ScratchFromDynamicLds)
    })
}

pub(crate) fn authenticate_collected_workgroup_sync_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<WorkgroupSyncFrontendReceiptV1, CollectedWorkgroupSyncErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics_identity =
        require_compiler_semantics(&observe_compiler_semantics(tcx), kind)?;
    let root = exact_root(&collection.functions)?;
    require_registration(root, kind)?;
    let source_identity = observe_source_identity(tcx, root, kind)?;
    require_signature(tcx, root.instance, kind)?;
    let abi = require_layout(root, kind)?;
    let fn_abi_identity = require_fn_abi(tcx, root.instance, kind)?;
    let trusted_definitions_identity =
        trusted_definitions_and_terminals_identity(tcx, collection, kind)?;
    let target_identity = exact_target_identity(kind)?;
    let profile_launch = exact_profile_launch(kind)?;
    let imported = crate::mir_import::import_collection(tcx, collection)
        .map_err(|error| CollectedWorkgroupSyncErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            kind.kernel(),
            &target_identity,
            &abi,
            &profile_launch,
        ))
        .map_err(|error| CollectedWorkgroupSyncErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = *portable_mir_identity.as_bytes();
    if portable_mir_identity != kind.portable_mir() {
        return Err(CollectedWorkgroupSyncErrorV1::PortableMirIdentity {
            expected: kind.portable_mir(),
            actual: portable_mir_identity,
        });
    }
    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if !crate::collected_tiled_gemm_v1::is_kernel_root_build_identity(&root_instance_identity)
        || root_instance_identity != kind.root_instance_identity()
    {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(format!(
            "root instance has noncanonical generated identity `{root_instance_identity}`"
        )));
    }
    let selected = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            let ir = lds_reduction_v1_kernel_ir();
            let profile = LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6();
            verify_lds_reduction_v1(&ir, &profile)
                .map_err(|error| CollectedWorkgroupSyncErrorV1::CanonicalIr(error.to_string()))?;
            SelectedWorkgroupSyncProfileV1::Lds(ir, profile)
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            let ir = scoped_atomic_v1_kernel_ir();
            let profile = ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6();
            verify_scoped_atomic_v1(&ir, &profile)
                .map_err(|error| CollectedWorkgroupSyncErrorV1::CanonicalIr(error.to_string()))?;
            SelectedWorkgroupSyncProfileV1::Atomic(ir, profile)
        }
    };
    let mut authority = WorkgroupSyncAuthorityV1 {
        kind,
        source_identity,
        source_namespace: kind.namespace(),
        compiler_crate_binding: compiler_crate_binding(kind).as_bytes(),
        target: target.as_str().to_owned(),
        code_object_version: WORKGROUP_SYNC_CODE_OBJECT_VERSION_V1,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_identity,
        compiler_semantics_identity,
        fn_abi_identity,
        trusted_definitions_identity,
        frontend_contract_identity: sha256(
            root.frontend_contract
                .as_ref()
                .expect("registration checked frontend contract")
                .canonical_bytes(),
        ),
        abi_identity: sha256(kind.abi_binding()),
        effects_identity: sha256(kind.effect_binding()),
        resources_identity: sha256(kind.resource_binding()),
        canonical_ir_identity: sha256(kind.canonical_ir_binding()),
        correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
        authority_identity: [0; 32],
    };
    authority.authority_identity = authority_identity(&authority);
    validate_authority(&authority)?;
    Ok(WorkgroupSyncFrontendReceiptV1 {
        authority: Some(authority),
        selected: Some(selected),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    if target != EXACT_WORKGROUP_SYNC_TARGET_V1 {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(format!(
            "requires exact target `{EXACT_WORKGROUP_SYNC_TARGET_V1}`, found `{target}`"
        )));
    }
    if custom_llvm_pipeline {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(
            "custom LLVM arguments or passes are forbidden".into(),
        ));
    }
    Ok(())
}

fn exact_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedWorkgroupSyncErrorV1> {
    let mut roots = functions
        .iter()
        .filter(|function| function.is_kernel_entry());
    let root = roots.next().ok_or_else(|| {
        CollectedWorkgroupSyncErrorV1::Admission("exact closure has no kernel root".into())
    })?;
    if roots.next().is_some() || functions.len() > 64 {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(format!(
            "exact closure requires one root and at most 64 reachable functions, found {}",
            functions.len()
        )));
    }
    Ok(root)
}

fn require_registration(
    root: &CollectedFunction<'_>,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    let expected_binding = derive_kernel_binding_id_v1(
        compiler_crate_binding(kind),
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        kind.kernel(),
        kind.kernel(),
    );
    if root.export_name != kind.kernel()
        || root.logical_name.as_deref() != Some(kind.kernel())
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
        return Err(CollectedWorkgroupSyncErrorV1::Admission(format!(
            "expected unique ordinary #[kernel(typed)] `{}` with reviewed wrapper/session binding and exact 64x1x1 launch contract",
            kind.kernel()
        )));
    }
    Ok(())
}

fn observe_source_identity(
    tcx: TyCtxt<'_>,
    root: &CollectedFunction<'_>,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<[u8; 32], CollectedWorkgroupSyncErrorV1> {
    let file_name = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(root.instance.def_id()))
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let bytes = std::fs::read(&file_name).map_err(|error| {
        CollectedWorkgroupSyncErrorV1::Admission(format!(
            "source file `{file_name}` is unavailable for exact-byte authentication: {error}"
        ))
    })?;
    let declaration = format!("namespace = \"{}\"", encode_hex(&kind.namespace()));
    if bytes
        .windows(declaration.len())
        .filter(|window| *window == declaration.as_bytes())
        .count()
        != 1
    {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(
            "exact source must contain one reviewed fallback namespace declaration".into(),
        ));
    }
    let actual = sha256(&bytes);
    if actual != kind.source_identity() {
        return Err(CollectedWorkgroupSyncErrorV1::SourceIdentity {
            expected: kind.source_identity(),
            actual,
        });
    }
    Ok(actual)
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(CollectedWorkgroupSyncErrorV1::Abi(
            "kernel must be one nongeneric ordinary function item".into(),
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| CollectedWorkgroupSyncErrorV1::Abi("signature normalization failed".into()))?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    let common = signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.output() == tcx.types.unit
        && signature.inputs().len() == 3;
    let exact = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            is_shared_slice(signature.inputs()[0], IntTy::I32)
                && matches!(signature.inputs()[1].kind(), TyKind::Uint(UintTy::U32))
                && is_disjoint_i32_slice(tcx, signature.inputs()[2])
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            is_shared_u32_slice(signature.inputs()[0])
                && is_shared_u32_slice(signature.inputs()[1])
                && is_global_mut_u32(tcx, signature.inputs()[2])
        }
    };
    if !common || !exact {
        return Err(CollectedWorkgroupSyncErrorV1::Abi(format!(
            "signature differs from exact {} profile: `{signature}`",
            kind.pipeline()
        )));
    }
    Ok(())
}

fn is_shared_slice(ty: Ty<'_>, integer: IntTy) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Int(value) if *value == integer))
    )
}

fn is_shared_u32_slice(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Uint(UintTy::U32)))
    )
}

fn is_disjoint_i32_slice(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, args) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::DisjointSlice)
        && args.len() == 2
        && args
            .first()
            .and_then(|value| value.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Int(IntTy::I32)))
}

fn is_global_mut_u32(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, args) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did())
        == Some(TrustedDeviceItem::DeviceGlobalMutPtr)
        && args.len() == 1
        && args
            .first()
            .and_then(|value| value.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Uint(UintTy::U32)))
}

fn require_layout(
    root: &CollectedFunction<'_>,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<AbiLayout, CollectedWorkgroupSyncErrorV1> {
    match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => require_lds_layout(root),
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            if root.general_typed_contract.is_some() || root.typed_layout_identities.is_some() {
                return Err(CollectedWorkgroupSyncErrorV1::Layout(
                    "scoped atomic must remain outside the generic scalar/slice contract".into(),
                ));
            }
            exact_atomic_abi()
        }
    }
}

fn require_lds_layout(
    root: &CollectedFunction<'_>,
) -> Result<AbiLayout, CollectedWorkgroupSyncErrorV1> {
    let contract = root.general_typed_contract.as_ref().ok_or_else(|| {
        CollectedWorkgroupSyncErrorV1::Layout("General V3 LDS contract is absent".into())
    })?;
    let actual = contract
        .arguments()
        .iter()
        .map(|value| value.kind())
        .collect::<Vec<_>>();
    if actual != LDS_ARGUMENT_KINDS_V1
        || root
            .typed_layout_identities
            .as_ref()
            .map(|identities| identities.len())
            != Some(3)
    {
        return Err(CollectedWorkgroupSyncErrorV1::Layout(format!(
            "expected exact LDS argument kinds {LDS_ARGUMENT_KINDS_V1:?}, found {actual:?}"
        )));
    }
    let abi = contract.abi();
    if abi.size() != u64::from(LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES)
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 3
    {
        return Err(CollectedWorkgroupSyncErrorV1::Layout(format!(
            "expected ptr64 size-40 align-8 three-field LDS ABI, found {abi:?}"
        )));
    }
    let fields = abi.fields();
    let exact = matches!(
        fields[0].kind(),
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4
        }
    ) && fields[0].offset() == 0
        && fields[0].access() == Access::ReadOnly
        && fields[0].ownership() == ArgumentOwnership::SharedBorrow
        && fields[1].kind() == AbiKind::Scalar(ScalarType::U32)
        && fields[1].offset() == 16
        && fields[1].access() == Access::ByValue
        && fields[2].offset() == 24
        && matches!(
            fields[2].kind(),
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4
            }
        )
        && fields[2].mutability() == ArtifactMutability::Mutable
        && fields[2].access() == Access::ReadWrite
        && fields[2].address_space() == AddressSpace::Global
        && fields[2].ownership() == ArgumentOwnership::UniqueBorrow
        && fields[2].alias_class() == AliasClass::Exclusive;
    if !exact {
        return Err(CollectedWorkgroupSyncErrorV1::Layout(
            "LDS ABI access, ownership, type, address-space, or offset drifted".into(),
        ));
    }
    require_source_launch(contract.launch())?;
    Ok(abi.clone())
}

fn exact_atomic_abi() -> Result<AbiLayout, CollectedWorkgroupSyncErrorV1> {
    // These labels only satisfy the artifact model's identity fields. Exact raw
    // rustc Ty and FnAbi checks above are the authority for this quarantined ABI.
    let fields = vec![
        exact_field(
            "arg0",
            0,
            16,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            ArtifactMutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )?,
        exact_field(
            "arg1",
            16,
            16,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            ArtifactMutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )?,
        exact_field(
            "arg2",
            32,
            8,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            ArtifactMutability::Mutable,
            Access::ReadWrite,
            AddressSpace::Global,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )?,
    ];
    AbiLayout::new(40, 8, PointerWidth::Bits64, fields)
        .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn exact_field(
    name: &str,
    offset: u64,
    size: u64,
    kind: AbiKind,
    mutability: ArtifactMutability,
    access: Access,
    address_space: AddressSpace,
    ownership: ArgumentOwnership,
    alias: AliasClass,
) -> Result<AbiField, CollectedWorkgroupSyncErrorV1> {
    let identity = TypeIdentity::new(
        DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(sha256(
            format!("{name}:type").as_bytes(),
        ))),
        DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(sha256(
            format!("{name}:layout:{offset}:{size}").as_bytes(),
        ))),
    );
    AbiField::new(
        Name::new(name)
            .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))?,
        offset,
        size,
        8,
        kind,
        mutability,
        access,
        address_space,
        identity,
        ownership,
        alias,
    )
    .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))
}

fn require_source_launch(launch: &LaunchContract) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    let exact = Dimensions::new(64, 1, 1)
        .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))?;
    let max_grid = Dimensions::new(u32::MAX, 1, 1)
        .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))?;
    if launch.rank() != 1
        || launch.block_size() != BlockSize::Exact(exact)
        || launch.max_grid() != max_grid
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedWorkgroupSyncErrorV1::Layout(
            "source launch must be exact WG64, one-dimensional grid, and no source-declared LDS"
                .into(),
        ));
    }
    Ok(())
}

fn require_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
    kind: WorkgroupSyncProfileKindV1,
) -> Result<[u8; 32], CollectedWorkgroupSyncErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        CollectedWorkgroupSyncErrorV1::Abi(format!("FnAbi query failed: {error:?}"))
    })?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 3
        || abi.args.len() != 3
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(CollectedWorkgroupSyncErrorV1::Abi(format!(
            "FnAbi header must be Rust(args=3)->unit, found {abi:?}"
        )));
    }
    let expected_sizes = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => [16, 4, 16],
        WorkgroupSyncProfileKindV1::ScopedAtomic => [16, 16, 8],
    };
    let expected_alignments = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => [8, 4, 8],
        WorkgroupSyncProfileKindV1::ScopedAtomic => [8, 8, 8],
    };
    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_DOMAIN_V1);
    hash_field(&mut digest, kind.pipeline().as_bytes());
    hash_field(&mut digest, &[u8::from(abi.c_variadic)]);
    hash_field(&mut digest, &abi.fixed_count.to_le_bytes());
    hash_field(&mut digest, &[u8::from(abi.can_unwind)]);
    for (index, argument) in abi.args.iter().enumerate() {
        if argument.layout.size.bytes() != expected_sizes[index]
            || argument.layout.align.abi.bytes() != expected_alignments[index]
        {
            return Err(CollectedWorkgroupSyncErrorV1::Abi(format!(
                "FnAbi argument {index} size or alignment drifted"
            )));
        }
        hash_field(&mut digest, &argument.layout.size.bytes().to_le_bytes());
        hash_field(
            &mut digest,
            &argument.layout.align.abi.bytes().to_le_bytes(),
        );
        match argument.mode {
            PassMode::Pair(first, second) if expected_sizes[index] == 16 => {
                hash_field(&mut digest, &[2]);
                hash_arg_attributes(&mut digest, first);
                hash_arg_attributes(&mut digest, second);
            }
            PassMode::Direct(attributes) if expected_sizes[index] != 16 => {
                hash_field(&mut digest, &[1]);
                hash_arg_attributes(&mut digest, attributes);
            }
            _ => {
                return Err(CollectedWorkgroupSyncErrorV1::Abi(format!(
                    "FnAbi argument {index} pass mode drifted"
                )));
            }
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != kind.fn_abi() {
        return Err(CollectedWorkgroupSyncErrorV1::FnAbiIdentity {
            expected: kind.fn_abi(),
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerProviderIdentityV1 {
    crate_name: String,
    stable_crate_id: u64,
    crate_hash: [u8; 16],
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
    kind: WorkgroupSyncProfileKindV1,
) -> Result<[u8; 32], CollectedWorkgroupSyncErrorV1> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, TRUSTED_DEFINITIONS_DOMAIN_V3);
    hash_field(&mut digest, kind.pipeline().as_bytes());
    let mut provider = None;
    for item in kind.trusted_items() {
        let definition = trusted_device_items::definition(tcx, *item).ok_or_else(|| {
            CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
                "missing exact diagnostic item `{}`",
                item.canonical_path()
            ))
        })?;
        if definition.is_local() || provider.is_some_and(|value| value != definition.krate) {
            return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
                "diagnostic item `{}` did not come from one external device provider",
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
    let device_provider = provider.ok_or_else(|| {
        CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
            "exact profile has no reviewed device provider".into(),
        )
    })?;
    let device_provider_identity = compiler_provider(tcx, device_provider);
    let core_provider = tcx
        .lang_items()
        .get(LangItem::Sized)
        .ok_or_else(|| {
            CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
                "pinned compiler omitted the core Sized lang item".into(),
            )
        })?
        .krate;
    let core_provider_identity = compiler_provider(tcx, core_provider);
    if core_provider_identity.crate_name != "core"
        || core_provider_identity.stable_crate_id == 0
        || core_provider_identity.crate_hash == [0; 16]
    {
        return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
            "pinned core provider identity is incomplete".into(),
        ));
    }
    let mut terminals = BTreeMap::new();
    for function in &collection.functions {
        let body = tcx.instance_mir(function.instance.def);
        for block in body.basic_blocks.iter() {
            let Some(terminator) = &block.terminator else {
                return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
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
                CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
                    "semantic-terminal call resolution failed".into(),
                )
            })?
            .map_or(*def_id, |instance| instance.def_id());
            let Some(role) = classify_exact_workgroup_sync_compiler_intrinsic(tcx, resolved) else {
                continue;
            };
            if !kind.compiler_terminals().contains(&role) {
                return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
                    "unreviewed semantic terminal `{}` entered the exact MIR closure",
                    role.canonical_path()
                )));
            }
            if terminals
                .insert(role, resolved)
                .is_some_and(|old| old != resolved)
            {
                return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
                    "semantic role `{}` resolved to multiple definitions",
                    role.canonical_path()
                )));
            }
        }
    }
    if terminals.len() != kind.compiler_terminals().len() {
        return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
            "semantic-terminal set drifted: expected {:?}, found {:?}",
            kind.compiler_terminals(),
            terminals.keys().collect::<Vec<_>>()
        )));
    }
    for role in kind.compiler_terminals() {
        let definition = *terminals.get(role).ok_or_else(|| {
            CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
                "missing semantic terminal `{}`",
                role.canonical_path()
            ))
        })?;
        let terminal_provider = compiler_provider(tcx, definition.krate);
        require_terminal_provider(
            *role,
            &terminal_provider,
            &device_provider_identity,
            &core_provider_identity,
        )?;
        let compiler_path = tcx.def_path_str(definition);
        let local_def_path_hash = tcx.def_path_hash(definition).local_hash().as_u64();
        if role.is_rustc_intrinsic() {
            hash_core_terminal(
                &mut digest,
                *role,
                &compiler_path,
                local_def_path_hash,
                &terminal_provider,
            );
        } else {
            let identity = reviewed_device_definition_identity(tcx, definition)?;
            hash_device_definition(
                &mut digest,
                role.canonical_path(),
                &compiler_path,
                local_def_path_hash,
                &identity,
            );
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != kind.trusted_terminal_identity() {
        return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
            "trusted-definition/semantic-terminal identity drifted: expected {}, found {}",
            encode_hex(&kind.trusted_terminal_identity()),
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
        crate_hash: tcx.crate_hash(crate_num).as_u128().to_le_bytes(),
    }
}

fn require_terminal_provider(
    role: WorkgroupSyncCompilerIntrinsicV1,
    actual: &CompilerProviderIdentityV1,
    device: &CompilerProviderIdentityV1,
    core: &CompilerProviderIdentityV1,
) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    let accepted = if role.is_rustc_intrinsic() {
        actual == core
    } else {
        actual == device
    };
    if accepted {
        Ok(())
    } else {
        Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(format!(
            "semantic terminal `{}` came from unreviewed provider `{}`",
            role.canonical_path(),
            actual.crate_name
        )))
    }
}

fn reviewed_device_definition_identity(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
) -> Result<ReviewedDeviceDefinitionIdentityV3, CollectedWorkgroupSyncErrorV1> {
    let observed =
        trusted_device_items::reviewed_workgroup_sync_provider_definition(tcx, definition)
            .map_err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions)?;
    let provider = compiler_provider(tcx, definition.krate);
    if observed.crate_name != provider.crate_name
        || observed.stable_crate_id != provider.stable_crate_id
        || observed.crate_hash_observation != provider.crate_hash
        || observed.cargo_metadata_build_observation == [0; 32]
        || observed.source_closure_identity == [0; 32]
        || observed.definition_source_identity == [0; 32]
    {
        return Err(CollectedWorkgroupSyncErrorV1::TrustedDefinitions(
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

fn hash_core_terminal(
    digest: &mut Sha256,
    role: WorkgroupSyncCompilerIntrinsicV1,
    compiler_path: &str,
    local_def_path_hash: u64,
    provider: &CompilerProviderIdentityV1,
) {
    hash_field(digest, b"pinned-rustc-core-terminal-v1");
    hash_field(digest, role.canonical_path().as_bytes());
    hash_field(digest, compiler_path.as_bytes());
    hash_field(digest, &local_def_path_hash.to_le_bytes());
    hash_field(digest, provider.crate_name.as_bytes());
    hash_field(digest, &provider.stable_crate_id.to_le_bytes());
    hash_field(digest, &provider.crate_hash);
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
    kind: WorkgroupSyncProfileKindV1,
) -> Result<[u8; 32], CollectedWorkgroupSyncErrorV1> {
    let expected_passes = [("JumpThreading".to_owned(), false)];
    let mismatch = if observed.rustc_release != REVIEWED_RUSTC_RELEASE
        || observed.rustc_commit != REVIEWED_RUSTC_COMMIT
        || observed.llvm_version != REVIEWED_RUSTC_LLVM
    {
        Some("rustc release, commit, or LLVM version drifted".to_owned())
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
    } else if observed.crate_name != kind.crate_name()
        || observed.crate_metadata != [kind.metadata()]
    {
        Some("reviewed crate name or ordered metadata drifted".into())
    } else if derive_crate_binding_id_v1(
        &observed.crate_name,
        observed.crate_metadata.iter().map(String::as_str),
    ) != compiler_crate_binding(kind)
    {
        Some("crate name and metadata do not derive the reviewed binding".into())
    } else if observed.remap_path_destinations != [kind.source_remap(), WORKSPACE_REMAP_DESTINATION]
    {
        Some(format!(
            "source remapping differs from the reviewed fixture: {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(detail));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, COMPILER_SEMANTICS_DOMAIN_V1);
    hash_field(&mut digest, kind.pipeline().as_bytes());
    hash_field(&mut digest, observed.rustc_release.as_bytes());
    hash_field(&mut digest, observed.rustc_commit.as_bytes());
    hash_field(&mut digest, observed.llvm_version.as_bytes());
    hash_field(&mut digest, observed.panic_strategy.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.overflow_checks)]);
    hash_field(&mut digest, observed.optimize.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.debug_assertions)]);
    hash_field(&mut digest, &(observed.mir_opt_level as u64).to_le_bytes());
    hash_field(&mut digest, observed.crate_name.as_bytes());
    hash_field(&mut digest, observed.crate_metadata[0].as_bytes());
    let actual: [u8; 32] = digest.finalize().into();
    if actual != kind.compiler_semantics() {
        return Err(CollectedWorkgroupSyncErrorV1::Admission(format!(
            "compiler semantics identity drifted: expected {}, found {}",
            encode_hex(&kind.compiler_semantics()),
            encode_hex(&actual)
        )));
    }
    Ok(actual)
}

fn exact_target_identity(
    kind: WorkgroupSyncProfileKindV1,
) -> Result<TargetIdentity, CollectedWorkgroupSyncErrorV1> {
    let capabilities = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            vec![Capability::WorkgroupMemory, Capability::AmdWave]
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => vec![Capability::Atomics],
    };
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| CollectedWorkgroupSyncErrorV1::Admission(error.to_string()))?,
        IdentityText::new(EXACT_WORKGROUP_SYNC_TARGET_V1)
            .map_err(|error| CollectedWorkgroupSyncErrorV1::Admission(error.to_string()))?,
        PointerWidth::Bits64,
        Endianness::Little,
        capabilities,
    )
    .map_err(|error| CollectedWorkgroupSyncErrorV1::Admission(error.to_string()))
}

fn exact_profile_launch(
    kind: WorkgroupSyncProfileKindV1,
) -> Result<LaunchContract, CollectedWorkgroupSyncErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))?,
        match kind {
            WorkgroupSyncProfileKindV1::LdsReduction => 256,
            WorkgroupSyncProfileKindV1::ScopedAtomic => 0,
        },
        0,
    )
    .map_err(|error| CollectedWorkgroupSyncErrorV1::Layout(error.to_string()))
}

fn validate_authority(
    authority: &WorkgroupSyncAuthorityV1,
) -> Result<(), CollectedWorkgroupSyncErrorV1> {
    let kind = authority.kind;
    let field = if authority.source_identity != kind.source_identity() {
        Some("source bytes")
    } else if authority.source_namespace != kind.namespace() {
        Some("source namespace")
    } else if authority.compiler_crate_binding != compiler_crate_binding(kind).as_bytes() {
        Some("wrapper/session crate binding")
    } else if authority.target != EXACT_WORKGROUP_SYNC_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != WORKGROUP_SYNC_CODE_OBJECT_VERSION_V1 {
        Some("code object version")
    } else if authority.kernel_export != kind.kernel() {
        Some("kernel export")
    } else if authority.root_instance_identity != kind.root_instance_identity() {
        Some("generated wrapper identity")
    } else if authority.portable_mir_identity != kind.portable_mir() {
        Some("complete reachable MIR closure")
    } else if authority.fn_abi_identity != kind.fn_abi() {
        Some("rustc FnAbi")
    } else if authority.compiler_semantics_identity != kind.compiler_semantics() {
        Some("compiler semantics")
    } else if authority.trusted_definitions_identity != kind.trusted_terminal_identity() {
        Some("trusted definitions and semantic terminals")
    } else if authority.frontend_contract_identity != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.abi_identity != sha256(kind.abi_binding()) {
        Some("ABI")
    } else if authority.effects_identity != sha256(kind.effect_binding()) {
        Some("effects")
    } else if authority.resources_identity != sha256(kind.resource_binding()) {
        Some("geometry/resources")
    } else if authority.canonical_ir_identity != sha256(kind.canonical_ir_binding()) {
        Some("canonical semantic IR")
    } else if authority.correspondence_identity != sha256(CORRESPONDENCE_BINDING_V1) {
        Some("reviewed correspondence boundary")
    } else if authority.authority_identity != authority_identity(authority) {
        Some("authority commitment")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedWorkgroupSyncErrorV1::ReceiptBinding(field));
    }
    Ok(())
}

fn authority_identity(authority: &WorkgroupSyncAuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, authority.kind.pipeline().as_bytes());
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
    hash_field(&mut digest, &authority.resources_identity);
    hash_field(&mut digest, &authority.canonical_ir_identity);
    hash_field(&mut digest, &authority.correspondence_identity);
    digest.finalize().into()
}

fn compiler_crate_binding(kind: WorkgroupSyncProfileKindV1) -> CrateBindingIdV1 {
    let value = match kind {
        WorkgroupSyncProfileKindV1::LdsReduction => LDS_COMPILER_CRATE_BINDING,
        WorkgroupSyncProfileKindV1::ScopedAtomic => ATOMIC_COMPILER_CRATE_BINDING,
    };
    CrateBindingIdV1::from_hex(value).expect("reviewed compiler crate binding is canonical")
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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
mod tests {
    use super::*;

    fn receipt(kind: WorkgroupSyncProfileKindV1) -> WorkgroupSyncFrontendReceiptV1 {
        let selected = match kind {
            WorkgroupSyncProfileKindV1::LdsReduction => SelectedWorkgroupSyncProfileV1::Lds(
                lds_reduction_v1_kernel_ir(),
                LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6(),
            ),
            WorkgroupSyncProfileKindV1::ScopedAtomic => SelectedWorkgroupSyncProfileV1::Atomic(
                scoped_atomic_v1_kernel_ir(),
                ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6(),
            ),
        };
        let mut authority = WorkgroupSyncAuthorityV1 {
            kind,
            source_identity: kind.source_identity(),
            source_namespace: kind.namespace(),
            compiler_crate_binding: compiler_crate_binding(kind).as_bytes(),
            target: EXACT_WORKGROUP_SYNC_TARGET_V1.into(),
            code_object_version: 6,
            kernel_export: kind.kernel().into(),
            root_instance_identity: kind.root_instance_identity().into(),
            portable_mir_identity: kind.portable_mir(),
            compiler_semantics_identity: kind.compiler_semantics(),
            fn_abi_identity: kind.fn_abi(),
            trusted_definitions_identity: kind.trusted_terminal_identity(),
            frontend_contract_identity: sha256(EXACT_FRONTEND_CONTRACT_V1),
            abi_identity: sha256(kind.abi_binding()),
            effects_identity: sha256(kind.effect_binding()),
            resources_identity: sha256(kind.resource_binding()),
            canonical_ir_identity: sha256(kind.canonical_ir_binding()),
            correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
            authority_identity: [0; 32],
        };
        authority.authority_identity = authority_identity(&authority);
        WorkgroupSyncFrontendReceiptV1 {
            authority: Some(authority),
            selected: Some(selected),
        }
    }

    #[test]
    fn both_receipts_select_only_the_exact_profile_once() {
        for kind in [
            WorkgroupSyncProfileKindV1::LdsReduction,
            WorkgroupSyncProfileKindV1::ScopedAtomic,
        ] {
            let mut value = receipt(kind);
            assert_eq!(value.consume().unwrap().kind(), kind);
            assert_eq!(
                value.consume(),
                Err(CollectedWorkgroupSyncErrorV1::ReceiptAlreadyConsumed)
            );
        }
    }

    #[test]
    fn authority_field_mutations_fail_closed() {
        let mutations: Vec<fn(&mut WorkgroupSyncAuthorityV1)> = vec![
            |value| value.source_identity[0] ^= 1,
            |value| value.source_namespace[0] ^= 1,
            |value| value.compiler_crate_binding[0] ^= 1,
            |value| value.target.push('+'),
            |value| value.code_object_version = 5,
            |value| value.kernel_export.push_str("_substitution"),
            |value| value.root_instance_identity.push('0'),
            |value| value.portable_mir_identity[0] ^= 1,
            |value| value.compiler_semantics_identity = [0; 32],
            |value| value.fn_abi_identity[0] ^= 1,
            |value| value.trusted_definitions_identity = [0; 32],
            |value| value.frontend_contract_identity[0] ^= 1,
            |value| value.abi_identity[0] ^= 1,
            |value| value.effects_identity[0] ^= 1,
            |value| value.resources_identity[0] ^= 1,
            |value| value.canonical_ir_identity[0] ^= 1,
            |value| value.correspondence_identity[0] ^= 1,
        ];
        for kind in [
            WorkgroupSyncProfileKindV1::LdsReduction,
            WorkgroupSyncProfileKindV1::ScopedAtomic,
        ] {
            for mutate in &mutations {
                let mut value = receipt(kind);
                mutate(value.authority.as_mut().unwrap());
                value.authority.as_mut().unwrap().authority_identity =
                    authority_identity(value.authority.as_ref().unwrap());
                assert!(matches!(
                    value.consume(),
                    Err(CollectedWorkgroupSyncErrorV1::ReceiptBinding(_))
                ));
            }
        }

        let mut stale_commitment = receipt(WorkgroupSyncProfileKindV1::LdsReduction);
        stale_commitment
            .authority
            .as_mut()
            .unwrap()
            .authority_identity[0] ^= 1;
        assert!(matches!(
            stale_commitment.consume(),
            Err(CollectedWorkgroupSyncErrorV1::ReceiptBinding(
                "authority commitment"
            ))
        ));
    }

    #[test]
    fn semantic_profile_substitutions_fail_after_authentication() {
        let mut lds = receipt(WorkgroupSyncProfileKindV1::LdsReduction);
        let Some(SelectedWorkgroupSyncProfileV1::Lds(ir, _)) = lds.selected.as_mut() else {
            unreachable!()
        };
        ir.lds.pointer_escape = true;
        assert!(matches!(
            lds.consume(),
            Err(CollectedWorkgroupSyncErrorV1::CanonicalIr(_))
        ));

        let mut atomic = receipt(WorkgroupSyncProfileKindV1::ScopedAtomic);
        let Some(SelectedWorkgroupSyncProfileV1::Atomic(ir, _)) = atomic.selected.as_mut() else {
            unreachable!()
        };
        ir.unique_host_borrow = false;
        assert!(matches!(
            atomic.consume(),
            Err(CollectedWorkgroupSyncErrorV1::CanonicalIr(_))
        ));
    }

    #[test]
    fn device_definition_identity_is_portable_but_fail_closed() {
        use WorkgroupSyncCompilerIntrinsicV1 as Terminal;

        fn identity(
            role: &str,
            path: &str,
            local_def_path_hash: u64,
            definition: &ReviewedDeviceDefinitionIdentityV3,
        ) -> [u8; 32] {
            let mut digest = Sha256::new();
            hash_device_definition(&mut digest, role, path, local_def_path_hash, definition);
            digest.finalize().into()
        }

        let device = CompilerProviderIdentityV1 {
            crate_name: "fe2o3_device".into(),
            stable_crate_id: 7,
            crate_hash: [3; 16],
        };
        let definition = ReviewedDeviceDefinitionIdentityV3 {
            provider: device.clone(),
            cargo_metadata_build_observation: [4; 32],
            source_closure_identity: [5; 32],
            definition_source_identity: [6; 32],
        };
        let role = Terminal::ScratchFromDynamicLds.canonical_path();
        let path = "_::__fe2o3_kernel_device::WorkgroupCollectiveScratch::from_dynamic_lds";
        let exact = identity(role, path, 11, &definition);
        assert_ne!(
            exact,
            identity(Terminal::ThreadIdxX.canonical_path(), path, 11, &definition,)
        );
        assert_ne!(
            exact,
            identity(
                role,
                "_::__fe2o3_kernel_device::fake::from_dynamic_lds",
                11,
                &definition,
            )
        );
        assert_ne!(exact, identity(role, path, 12, &definition));

        let mut mutation = definition.clone();
        mutation.provider.crate_name = "fake_fe2o3_device".into();
        assert_ne!(exact, identity(role, path, 11, &mutation));
        mutation = definition.clone();
        mutation.provider.stable_crate_id ^= 1;
        assert_ne!(exact, identity(role, path, 11, &mutation));
        mutation = definition.clone();
        mutation.cargo_metadata_build_observation[0] ^= 1;
        assert_ne!(exact, identity(role, path, 11, &mutation));
        mutation = definition.clone();
        mutation.source_closure_identity[0] ^= 1;
        assert_ne!(exact, identity(role, path, 11, &mutation));
        mutation = definition.clone();
        mutation.definition_source_identity[0] ^= 1;
        assert_ne!(exact, identity(role, path, 11, &mutation));

        // Rustc's full crate hash contains Cargo path-source disambiguation.
        // It remains a same-session provider check but is deliberately absent
        // from the portable identity preimage.
        mutation = definition.clone();
        mutation.provider.crate_hash[0] ^= 1;
        assert_eq!(exact, identity(role, path, 11, &mutation));

        let core = CompilerProviderIdentityV1 {
            crate_name: "core".into(),
            stable_crate_id: 9,
            crate_hash: [8; 16],
        };
        assert!(require_terminal_provider(Terminal::ThreadIdxX, &device, &device, &core).is_ok());
        assert!(
            require_terminal_provider(Terminal::ThreadIdxX, &mutation.provider, &device, &core)
                .is_err()
        );
        assert!(require_terminal_provider(Terminal::AtomicXadd, &core, &device, &core).is_ok());
        let impostor_core = CompilerProviderIdentityV1 {
            crate_name: "impostor_core".into(),
            ..core.clone()
        };
        assert!(
            require_terminal_provider(Terminal::AtomicXadd, &impostor_core, &device, &core)
                .is_err()
        );
    }

    #[test]
    fn core_terminal_identity_binds_pinned_provider_and_local_definition() {
        use WorkgroupSyncCompilerIntrinsicV1 as Terminal;

        fn identity(
            path: &str,
            local_def_path_hash: u64,
            provider: &CompilerProviderIdentityV1,
        ) -> [u8; 32] {
            let mut digest = Sha256::new();
            hash_core_terminal(
                &mut digest,
                Terminal::AtomicXadd,
                path,
                local_def_path_hash,
                provider,
            );
            digest.finalize().into()
        }

        let core = CompilerProviderIdentityV1 {
            crate_name: "core".into(),
            stable_crate_id: 9,
            crate_hash: [8; 16],
        };
        let exact = identity("std::intrinsics::atomic_xadd", 17, &core);
        assert_ne!(exact, identity("std::intrinsics::atomic_xsub", 17, &core));
        assert_ne!(exact, identity("std::intrinsics::atomic_xadd", 18, &core));
        let mut mutation = core.clone();
        mutation.stable_crate_id ^= 1;
        assert_ne!(
            exact,
            identity("std::intrinsics::atomic_xadd", 17, &mutation)
        );
        mutation = core;
        mutation.crate_hash[0] ^= 1;
        assert_ne!(
            exact,
            identity("std::intrinsics::atomic_xadd", 17, &mutation)
        );
    }
}
