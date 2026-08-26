//! Semantic identities recognized by device lowering.
//!
//! Recognition starts from a rustc [`DefId`]. Diagnostic-item equality is only
//! accepted after the provider definition is anchored to the reviewed sibling
//! `fe2o3-device` source tree used to build this backend. Rustc's stable crate
//! ID and crate hash are retained as same-session provenance observations, but
//! portable semantic identities bind only canonical source-derived fields.
//!
//! This remains a compiler build-observation boundary, not cryptographic
//! package authentication. A publisher signature or transparency-log identity
//! must be checked before the managed build when that stronger claim is needed.
//! General GEMM binds imported source hashes, exact semantic definitions, and
//! the device-type dependency edge. A Cargo manifest that selects the same
//! reviewed source and dependency graph is intentionally equivalent; manifest
//! authorship and package provenance are outside this authority.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::lang_items::LangItem;
use rustc_middle::ty::{FloatTy, TyCtxt, TyKind};
use rustc_span::{SourceFileHash, Symbol};
use sha2::{Digest as _, Sha256};

use dialect_amdgcn::{
    DeviceMathDiagnosticItem, DeviceValueDiagnosticItem, Fe2o3DeviceDiagnosticItem,
};
use fe2o3_kernel_ir::{NarrowFloatFormat, WidenedFloatBinaryOp};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;

const MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/MATRIX-PROVIDER-SOURCE-IDENTITY/V2\0";
const ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX-PROVIDER-SOURCE-IDENTITY/V1\0";
const WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKGROUP-SYNC-PROVIDER-SOURCE-IDENTITY/V1\0";
const WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKGROUP-SYNC-PROVIDER-SOURCE-CLOSURE/V1\0";
const REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1: [u8; 32] = [
    0x9c, 0x8a, 0x34, 0xfb, 0x3e, 0xd0, 0x49, 0x87, 0x2f, 0x19, 0x56, 0x28, 0x82, 0x58, 0x06, 0xec,
    0xb4, 0x21, 0x66, 0xc0, 0x1c, 0xfe, 0xb1, 0x0d, 0x8a, 0xfc, 0x12, 0x59, 0x9b, 0x78, 0x69, 0x42,
];
#[allow(
    dead_code,
    reason = "consumed by the staged row-softmax V2 provider protocol"
)]
const ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2: &[u8] =
    b"FE2O3/ROW-SOFTMAX-PROVIDER-SOURCE-CLOSURE/V2\0";
#[allow(
    dead_code,
    reason = "consumed by the staged matrix V3 provider protocol"
)]
const MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3: &[u8] =
    b"FE2O3/MATRIX-PROVIDER-SOURCE-CLOSURE/V3\0";
const GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM-PROVIDER-SOURCE-IDENTITY/V1\0";
const GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM-PROVIDER-SEMANTIC-SOURCE-TREE/V1\0";
const GENERAL_GEMM_DEPENDENCY_SEMANTIC_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM-DEPENDENCY-SEMANTIC-IDENTITY/V1\0";
// Exact reviewed Rust source tree for the compiler-issued semantic surface.
// The Cargo manifest/package that selected this source is not authenticated.
const REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_TREE_V1: [u8; 32] = [
    0xee, 0x5d, 0xcd, 0xb5, 0x44, 0x12, 0xc9, 0x5e, 0xe8, 0x2e, 0xa3, 0x2e, 0x0a, 0x46, 0x9f, 0x31,
    0xc3, 0xb3, 0xb7, 0xd8, 0xe1, 0x76, 0x5c, 0xf3, 0xa7, 0xf4, 0xac, 0x4f, 0x3d, 0xab, 0xa5, 0x7b,
];
const REVIEWED_GENERAL_GEMM_TYPESTATE_DEFINITION_SOURCE_V1: [u8; 32] = [
    0x6e, 0x25, 0xc4, 0xfc, 0xfc, 0x64, 0xc2, 0xc4, 0xd2, 0x2c, 0xa8, 0x0a, 0x6b, 0xbe, 0x6a, 0xe6,
    0x56, 0x73, 0x6a, 0xae, 0x66, 0x49, 0xab, 0x12, 0x17, 0xad, 0xc5, 0x2e, 0x47, 0x92, 0xcf, 0xe4,
];
const REVIEWED_GENERAL_GEMM_PROOF_DEFINITION_SOURCE_V1: [u8; 32] = [
    0x5d, 0xdf, 0x25, 0xf8, 0x70, 0x03, 0x3a, 0x57, 0xfe, 0xd7, 0x7b, 0xd2, 0xd7, 0xf9, 0x63, 0x1c,
    0x0b, 0xbd, 0xb7, 0x86, 0x11, 0x8f, 0x8c, 0x37, 0x28, 0x3e, 0x76, 0x09, 0xd9, 0x35, 0x2d, 0x18,
];
// Portable semantic identity of the reviewed `fe2o3_device::DisjointSlice`
// definition and reference source closure used by the store signatures.
const REVIEWED_GENERAL_GEMM_DISJOINT_SLICE_DEPENDENCY_V1: [u8; 32] = [
    0x47, 0x5e, 0x9b, 0xa4, 0x37, 0x62, 0x83, 0xbc, 0x90, 0x22, 0x87, 0x78, 0x7d, 0xaa, 0xb7, 0x3d,
    0xdb, 0xbb, 0xdd, 0x37, 0x27, 0x0f, 0xd9, 0x4f, 0xad, 0x10, 0xc5, 0xe3, 0xdf, 0xe6, 0x04, 0x72,
];

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn reviewed_general_gemm_provider_semantics_identity_v1() -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/GENERAL-GEMM-PROVIDER-SEMANTICS/V1\0");
    digest.update(REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_TREE_V1);
    digest.update(REVIEWED_GENERAL_GEMM_TYPESTATE_DEFINITION_SOURCE_V1);
    digest.update(REVIEWED_GENERAL_GEMM_PROOF_DEFINITION_SOURCE_V1);
    digest.update(REVIEWED_GENERAL_GEMM_DISJOINT_SLICE_DEPENDENCY_V1);
    digest.finalize().into()
}
const PROVIDER_SEMANTIC_DEFINITION_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/PROVIDER-SEMANTIC-DEFINITION-TRANSCRIPT/V1\0";
#[cfg(test)]
const PINNED_CORE_SEMANTIC_TERMINAL_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/PINNED-CORE-SEMANTIC-TERMINAL-TRANSCRIPT/V1\0";
const STRUCTURAL_LOCAL_DEFINITION_COMPONENT_DOMAIN_V1: &[u8] =
    b"FE2O3/STRUCTURAL-LOCAL-DEFINITION-COMPONENT/V1\0";
const REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../fe2o3-device");
const REVIEWED_FE2O3_DEVICE_SOURCE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../fe2o3-device/src");
const REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/tiled_gemm_general_v1/device-api/src"
);

static WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE: OnceLock<Result<[u8; 32], String>> = OnceLock::new();
#[allow(
    dead_code,
    reason = "consumed by the staged row-softmax V2 provider protocol"
)]
static ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_V2: OnceLock<Result<[u8; 32], String>> = OnceLock::new();
#[allow(
    dead_code,
    reason = "consumed by the staged matrix V3 provider protocol"
)]
static MATRIX_PROVIDER_SOURCE_CLOSURE_V3: OnceLock<Result<[u8; 32], String>> = OnceLock::new();
static GENERAL_GEMM_PROVIDER_SOURCE_TREE_V1: OnceLock<Result<[u8; 32], String>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) struct ReviewedMatrixProviderObservationV2 {
    pub(crate) crate_name: String,
    pub(crate) stable_crate_id: u64,
    pub(crate) crate_hash: [u8; 16],
    pub(crate) cargo_metadata_build_observation: [u8; 32],
    pub(crate) source_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) struct ReviewedRowSoftmaxProviderDefinitionV1 {
    pub(crate) crate_name: String,
    pub(crate) stable_crate_id: u64,
    pub(crate) crate_hash: [u8; 16],
    pub(crate) cargo_metadata_build_observation: [u8; 32],
    pub(crate) source_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerProviderObservationV1 {
    pub(crate) crate_name: String,
    pub(crate) stable_crate_id: u64,
    pub(crate) crate_hash_observation: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderSemanticDefinitionRoleV1 {
    TrustedDefinition,
    SemanticTerminal,
}

impl ProviderSemanticDefinitionRoleV1 {
    const fn canonical_name(self) -> &'static [u8] {
        match self {
            Self::TrustedDefinition => b"trusted-definition",
            Self::SemanticTerminal => b"semantic-terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewedProviderSemanticProfileV1 {
    WorkgroupFlashMoeV4,
    RowSoftmaxV2,
    MatrixV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderSemanticDefinitionExpectationV1<'a> {
    pub(crate) definition_role: ProviderSemanticDefinitionRoleV1,
    pub(crate) canonical_role: &'a str,
    pub(crate) canonical_definition_path: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewedProviderSemanticDefinitionV1 {
    /// These rustc values prove same-session crate membership only. They are
    /// intentionally excluded from `durable_semantic_identity` because Cargo
    /// can change them after an unrelated transitive feature change.
    pub(crate) provider: CompilerProviderObservationV1,
    profile: ReviewedProviderSemanticProfileV1,
    pub(crate) canonical_definition_path: String,
    pub(crate) structural_local_definition_component: [u8; 32],
    pub(crate) cargo_metadata_build_observation: [u8; 32],
    pub(crate) source_closure_identity: [u8; 32],
    pub(crate) definition_source_identity: [u8; 32],
}

impl ReviewedProviderSemanticDefinitionV1 {
    fn validate(&self) -> Result<(), String> {
        if self.provider.crate_name != "fe2o3_device"
            || self.provider.stable_crate_id == 0
            || self.provider.crate_hash_observation == [0; 16]
            || self.canonical_definition_path.is_empty()
            || self.structural_local_definition_component == [0; 32]
            || self.cargo_metadata_build_observation == [0; 32]
            || self.source_closure_identity == [0; 32]
            || self.definition_source_identity == [0; 32]
        {
            return Err("reviewed provider semantic definition is incomplete".to_owned());
        }
        let local_definition_path = self
            .canonical_definition_path
            .strip_prefix("fe2o3_device::")
            .ok_or_else(|| "reviewed provider definition path is not canonical".to_owned())?;
        if structural_local_definition_component_v1(local_definition_path)?
            != self.structural_local_definition_component
        {
            return Err("reviewed provider structural definition component changed".to_owned());
        }
        Ok(())
    }

    pub(crate) fn durable_semantic_identity(
        &self,
        definition_role: ProviderSemanticDefinitionRoleV1,
        canonical_role: &str,
    ) -> Result<[u8; 32], String> {
        self.validate()?;
        if canonical_role.is_empty() {
            return Err("reviewed provider semantic definition is incomplete".to_owned());
        }

        let mut hasher = Sha256::new();
        hash_source_identity_field(
            &mut hasher,
            PROVIDER_SEMANTIC_DEFINITION_TRANSCRIPT_DOMAIN_V1,
        );
        hash_source_identity_field(&mut hasher, definition_role.canonical_name());
        hash_source_identity_field(&mut hasher, canonical_role.as_bytes());
        hash_source_identity_field(&mut hasher, self.provider.crate_name.as_bytes());
        hash_source_identity_field(&mut hasher, self.canonical_definition_path.as_bytes());
        hash_source_identity_field(&mut hasher, &self.structural_local_definition_component);
        hash_source_identity_field(&mut hasher, &self.cargo_metadata_build_observation);
        hash_source_identity_field(&mut hasher, &self.source_closure_identity);
        hash_source_identity_field(&mut hasher, &self.definition_source_identity);
        Ok(hasher.finalize().into())
    }

    #[allow(
        dead_code,
        reason = "called by the staged row-softmax V2 and matrix V3 collectors"
    )]
    pub(crate) fn durable_semantic_identity_for_profile(
        &self,
        expected_profile: ReviewedProviderSemanticProfileV1,
        definition_role: ProviderSemanticDefinitionRoleV1,
        canonical_role: &str,
    ) -> Result<[u8; 32], String> {
        if self.profile != expected_profile {
            return Err("reviewed provider semantic profile was substituted".to_owned());
        }
        self.durable_semantic_identity(definition_role, canonical_role)
    }
}

fn general_gemm_dependency_semantic_identity_v1(
    definition: &ReviewedProviderSemanticDefinitionV1,
    compiled_definition_source_identity: [u8; 32],
    definition_role: ProviderSemanticDefinitionRoleV1,
    canonical_role: &str,
) -> Result<[u8; 32], String> {
    if definition.profile != ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4 {
        return Err("reviewed general-GEMM dependency profile was substituted".to_owned());
    }
    if definition.provider.crate_name != "fe2o3_device"
        || definition.canonical_definition_path.is_empty()
        || definition.structural_local_definition_component == [0; 32]
        || definition.source_closure_identity == [0; 32]
        || definition.definition_source_identity == [0; 32]
        || compiled_definition_source_identity == [0; 32]
        || canonical_role.is_empty()
    {
        return Err("reviewed general-GEMM dependency identity is incomplete".to_owned());
    }
    let local_definition_path = definition
        .canonical_definition_path
        .strip_prefix("fe2o3_device::")
        .ok_or_else(|| {
            "reviewed general-GEMM dependency definition path is not canonical".to_owned()
        })?;
    if structural_local_definition_component_v1(local_definition_path)?
        != definition.structural_local_definition_component
    {
        return Err(
            "reviewed general-GEMM dependency structural definition component changed".to_owned(),
        );
    }
    if compiled_definition_source_identity != definition.definition_source_identity {
        return Err(
            "reviewed general-GEMM DisjointSlice compiled source identity changed".to_owned(),
        );
    }

    let mut hasher = Sha256::new();
    hash_source_identity_field(
        &mut hasher,
        GENERAL_GEMM_DEPENDENCY_SEMANTIC_IDENTITY_DOMAIN_V1,
    );
    hash_source_identity_field(&mut hasher, definition_role.canonical_name());
    hash_source_identity_field(&mut hasher, canonical_role.as_bytes());
    hash_source_identity_field(&mut hasher, definition.provider.crate_name.as_bytes());
    hash_source_identity_field(&mut hasher, definition.canonical_definition_path.as_bytes());
    hash_source_identity_field(
        &mut hasher,
        &definition.structural_local_definition_component,
    );
    hash_source_identity_field(&mut hasher, &definition.source_closure_identity);
    hash_source_identity_field(&mut hasher, &compiled_definition_source_identity);
    Ok(hasher.finalize().into())
}

#[allow(
    dead_code,
    reason = "called by the staged row-softmax V2 and matrix V3 collectors"
)]
pub(crate) fn validate_ordered_provider_semantic_definitions_v1(
    definitions: &[ReviewedProviderSemanticDefinitionV1],
    expectations: &[ProviderSemanticDefinitionExpectationV1<'_>],
) -> Result<(CompilerProviderObservationV1, Vec<[u8; 32]>), String> {
    if definitions.is_empty() || definitions.len() != expectations.len() {
        return Err("reviewed provider definition sequence has the wrong length".to_owned());
    }

    let mut expected_paths = BTreeSet::new();
    let mut expected_roles = BTreeSet::new();
    let mut observed_paths = BTreeSet::new();
    let provider = definitions[0].provider.clone();
    let profile = definitions[0].profile;
    let mut identities = Vec::with_capacity(definitions.len());
    for (definition, expectation) in definitions.iter().zip(expectations) {
        definition.validate()?;
        if expectation.canonical_definition_path.is_empty()
            || !expected_paths.insert(expectation.canonical_definition_path)
        {
            return Err("reviewed provider definition sequence has duplicate expectations".into());
        }
        if expectation.canonical_role.is_empty()
            || !expected_roles.insert(expectation.canonical_role)
        {
            return Err("reviewed provider definition sequence has duplicate roles".into());
        }
        if !observed_paths.insert(definition.canonical_definition_path.as_str()) {
            return Err("reviewed provider definition sequence has duplicate definitions".into());
        }
        if definition.canonical_definition_path != expectation.canonical_definition_path {
            return Err("reviewed provider definition sequence is reordered or substituted".into());
        }
        if definition.provider != provider {
            return Err("reviewed provider changed within the compiler session".into());
        }
        if definition.profile != profile {
            return Err("reviewed provider semantic profile changed within the sequence".into());
        }
        identities.push(definition.durable_semantic_identity_for_profile(
            profile,
            expectation.definition_role,
            expectation.canonical_role,
        )?);
    }
    Ok((provider, identities))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RejectedTrustedProvider {
    pub(crate) marker: &'static str,
    pub(crate) expected_provider_crate: &'static str,
    pub(crate) reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedHalfOperation {
    FromF32(NarrowFloatFormat),
    ToF32(NarrowFloatFormat),
    WidenedBinary {
        format: NarrowFloatFormat,
        op: WidenedFloatBinaryOp,
    },
    Bf16x2FusedMultiplyAdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedAmdGpuInlineOperation {
    VMovB32,
    VAddU32,
    VSubU32,
    VAndB32,
    VOrB32,
    VXorB32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedAmdGpuDiagnosticOperation {
    Print0,
    Print1,
    Print2,
    AssertFail,
    Clock32,
    Trap,
    DebugTrap,
    ProfilingMarker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedGeneralGemmSurfaceV1 {
    Typestate,
    ProofSensitive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedGeneralGemmOperationV1 {
    Acquire,
    Lane,
    WorkgroupX,
    WorkgroupY,
    LoadA,
    LoadB,
    LoadC,
    Stage,
    StageValue,
    WaitStage,
    ReadStage,
    Publish,
    Mfma,
    MfmaValue,
    Reuse,
    Store,
    StoreEpilogue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedDeviceItem {
    KernelError,
    DisjointSlice,
    DeviceGlobalMutPtr,
    WorkgroupLdsScope,
    WorkgroupLdsScopeCurrent,
    DynamicLdsExactCurrent,
    Invocation3D,
    Invocation3DCurrent,
    ThreadIndexX,
    ThreadIndexY,
    ThreadIndexZ,
    WorkgroupIndexX,
    WorkgroupIndexY,
    WorkgroupIndexZ,
    WorkgroupDimensionX,
    WorkgroupDimensionY,
    WorkgroupDimensionZ,
    GridDimensionX,
    GridDimensionY,
    GridDimensionZ,
    ThreadIndex,
    DisjointIndex,
    ShiftedIndexSpace,
    BlockedIndexSpace,
    Tiled2DIndexSpace,
    RowStriped2DIndexSpace,
    GridExclusiveIndexSpace,
    DisjointBlock,
    DisjointTile2D,
    DisjointRowStripe2D,
    GridLeader,
    ThreadIndex1d,
    ThreadIndexGet,
    ThreadIndexIntoDisjoint,
    ThreadIndexCheckedShift,
    ThreadIndexCheckedBlock,
    ThreadIndexCheckedTiled2D,
    ThreadIndexCheckedRowStriped2D,
    DisjointIndexGet,
    DisjointIndexCheckedShift,
    DisjointBlockComponentIndex,
    GridLeaderCurrent,
    ThreadIndexOffset,
    ThreadIndexOffsetSigned,
    ThreadIndexStride,
    ThreadIndexStrideOffset,
    DisjointSliceGetMut,
    DisjointSliceGetDisjointMut,
    DisjointSliceGetMutExclusive,
    DisjointSliceGetBlockMut,
    DisjointSliceGetTiled2DMut,
    DisjointSliceGetRowStriped2DMut,
    DisjointSliceGetMutAt,
    DisjointSliceLen,
    StridedReadView2D,
    StridedReadView2DError,
    StridedReadView2DFromSharedSlice,
    StridedReadView2DLoadOr,
    DeviceGlobalMutPtrU32AsAtomic,
    DeviceGlobalMutPtrI32AsAtomic,
    DeviceGlobalMutPtrU64AsAtomic,
    DeviceGlobalMutPtrI64AsAtomic,
    MemoryOffsetFrom,
    MemoryVolatileLoad,
    MemoryVolatileStore,
    MemoryCopyNonOverlapping,
    MemoryCopyOneNonOverlapping,
    Gfx942CollectivesContext,
    Gfx942CollectivesCurrent,
    Gfx942SubgroupReduceSumF32,
    Gfx942SubgroupReduceMaxF32,
    Gfx942StaticLdsU32x256,
    Gfx942StaticLdsU32x256Type,
    Gfx942Wave64ReduceActiveU32,
    Gfx942Workgroup256ReduceActiveU32,
    Gfx942Wave64ReduceSum,
    Gfx942Wave64InclusiveScanSum,
    Gfx942Wave64ExclusiveScanSum,
    Gfx942WorkgroupReduceSum,
    Gfx942WorkgroupInclusiveScanSum,
    Gfx942WorkgroupExclusiveScanSum,
    Gfx942BarrierArrive,
    Gfx942BarrierWait,
    WaveLane,
    Wave64,
    WaveLaneCurrent,
    Gfx942LdsBf16TilePairM16x16,
    Gfx942LdsBf16TilePairPublishM16x16,
    LdsTile16x16WriteMfmaBf16,
    LdsTile16x16ReadMfmaBf16,
    WorkgroupSyncthreads,
    DeviceMatrix,
    DeviceMatrixCurrent,
    Bf16MfmaProfile,
    MfmaOperandA,
    MfmaOperandB,
    MfmaRegisterTile16x16,
    MfmaLdsXor4Storage,
    MfmaAccumulatorRowMajor,
    Bf16MfmaFragment,
    F32AccumulatorFragment,
    F32AccumulatorFragmentZero,
    F32AccumulatorFragmentIntoValues,
    Bf16MfmaMatrixView,
    Bf16MfmaMatrixViewError,
    Bf16MfmaMatrixARowMajor,
    Bf16MfmaMatrixBRowMajor,
    Bf16MfmaMatrixALoadZeroFilledV2,
    Bf16MfmaMatrixBLoadZeroFilledV2,
    DeviceMatrixMultiplyAccumulate,
    GeneralGemm(TrustedGeneralGemmSurfaceV1, TrustedGeneralGemmOperationV1),
    DeviceValue(DeviceValueDiagnosticItem),
    DeviceMath(DeviceMathDiagnosticItem),
    HalfOperation(TrustedHalfOperation),
    AmdGpuInline(TrustedAmdGpuInlineOperation),
    AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation),
}

const TRUSTED_ITEMS: &[(TrustedDeviceItem, &str, &str)] = &[
    (
        TrustedDeviceItem::KernelError,
        "fe2o3_device_kernel_error_v1",
        "fe2o3_device::KernelError",
    ),
    (
        TrustedDeviceItem::DisjointSlice,
        "fe2o3_device_disjoint_slice",
        "fe2o3_device::DisjointSlice",
    ),
    (
        TrustedDeviceItem::StridedReadView2D,
        "fe2o3_device_strided_read_view_2d_v1",
        "fe2o3_device::StridedReadView2D",
    ),
    (
        TrustedDeviceItem::StridedReadView2DError,
        "fe2o3_device_strided_read_view_2d_error_v1",
        "fe2o3_device::StridedReadView2DError",
    ),
    (
        TrustedDeviceItem::StridedReadView2DFromSharedSlice,
        "fe2o3_device_strided_read_view_2d_from_shared_slice_v1",
        "fe2o3_device::StridedReadView2D::from_shared_slice",
    ),
    (
        TrustedDeviceItem::StridedReadView2DLoadOr,
        "fe2o3_device_strided_read_view_2d_load_or_v1",
        "fe2o3_device::StridedReadView2D::load_or",
    ),
    (
        TrustedDeviceItem::DeviceGlobalMutPtr,
        "fe2o3_device_ffi_global_mut_ptr_v1",
        "fe2o3_device::DeviceGlobalMutPtr",
    ),
    (
        TrustedDeviceItem::WorkgroupLdsScope,
        "fe2o3_device_workgroup_lds_scope",
        "fe2o3_device::WorkgroupLdsScope",
    ),
    (
        TrustedDeviceItem::WorkgroupLdsScopeCurrent,
        "fe2o3_device_workgroup_lds_scope_current",
        "fe2o3_device::WorkgroupLdsScope::current",
    ),
    (
        TrustedDeviceItem::DynamicLdsExactCurrent,
        "fe2o3_device_dynamic_lds_exact_current_v1",
        "fe2o3_device::DynamicLds::<T>::exact_current",
    ),
    (
        TrustedDeviceItem::Invocation3D,
        "fe2o3_device_invocation_3d",
        "fe2o3_device::Invocation3D",
    ),
    (
        TrustedDeviceItem::Invocation3DCurrent,
        "fe2o3_device_invocation_3d_current",
        "fe2o3_device::Invocation3D::current",
    ),
    (
        TrustedDeviceItem::ThreadIndexX,
        "fe2o3_device_thread_index_x_v1",
        "fe2o3_device::thread::thread_idx_x",
    ),
    (
        TrustedDeviceItem::ThreadIndexY,
        "fe2o3_device_thread_index_y_v1",
        "fe2o3_device::thread::thread_idx_y",
    ),
    (
        TrustedDeviceItem::ThreadIndexZ,
        "fe2o3_device_thread_index_z_v1",
        "fe2o3_device::thread::thread_idx_z",
    ),
    (
        TrustedDeviceItem::WorkgroupIndexX,
        "fe2o3_device_workgroup_index_x_v1",
        "fe2o3_device::thread::block_idx_x",
    ),
    (
        TrustedDeviceItem::WorkgroupIndexY,
        "fe2o3_device_workgroup_index_y_v1",
        "fe2o3_device::thread::block_idx_y",
    ),
    (
        TrustedDeviceItem::WorkgroupIndexZ,
        "fe2o3_device_workgroup_index_z_v1",
        "fe2o3_device::thread::block_idx_z",
    ),
    (
        TrustedDeviceItem::WorkgroupDimensionX,
        "fe2o3_device_workgroup_dimension_x_v1",
        "fe2o3_device::thread::block_dim_x",
    ),
    (
        TrustedDeviceItem::WorkgroupDimensionY,
        "fe2o3_device_workgroup_dimension_y_v1",
        "fe2o3_device::thread::block_dim_y",
    ),
    (
        TrustedDeviceItem::WorkgroupDimensionZ,
        "fe2o3_device_workgroup_dimension_z_v1",
        "fe2o3_device::thread::block_dim_z",
    ),
    (
        TrustedDeviceItem::GridDimensionX,
        "fe2o3_device_grid_dimension_x_v1",
        "fe2o3_device::thread::grid_dim_x",
    ),
    (
        TrustedDeviceItem::GridDimensionY,
        "fe2o3_device_grid_dimension_y_v1",
        "fe2o3_device::thread::grid_dim_y",
    ),
    (
        TrustedDeviceItem::GridDimensionZ,
        "fe2o3_device_grid_dimension_z_v1",
        "fe2o3_device::thread::grid_dim_z",
    ),
    (
        TrustedDeviceItem::ThreadIndex,
        "fe2o3_device_thread_index",
        "fe2o3_device::ThreadIndex",
    ),
    (
        TrustedDeviceItem::DisjointIndex,
        "fe2o3_device_disjoint_index",
        "fe2o3_device::DisjointIndex",
    ),
    (
        TrustedDeviceItem::ShiftedIndexSpace,
        "fe2o3_device_shifted_index_space",
        "fe2o3_device::Shifted",
    ),
    (
        TrustedDeviceItem::BlockedIndexSpace,
        "fe2o3_device_blocked_index_space",
        "fe2o3_device::Blocked",
    ),
    (
        TrustedDeviceItem::Tiled2DIndexSpace,
        "fe2o3_device_tiled_2d_index_space",
        "fe2o3_device::Tiled2D",
    ),
    (
        TrustedDeviceItem::RowStriped2DIndexSpace,
        "fe2o3_device_row_striped_2d_index_space",
        "fe2o3_device::RowStriped2D",
    ),
    (
        TrustedDeviceItem::GridExclusiveIndexSpace,
        "fe2o3_device_grid_exclusive_index_space",
        "fe2o3_device::GridExclusive",
    ),
    (
        TrustedDeviceItem::GridLeader,
        "fe2o3_device_grid_leader",
        "fe2o3_device::GridLeader",
    ),
    (
        TrustedDeviceItem::DisjointBlock,
        "fe2o3_device_disjoint_block",
        "fe2o3_device::DisjointBlock",
    ),
    (
        TrustedDeviceItem::DisjointTile2D,
        "fe2o3_device_disjoint_tile_2d",
        "fe2o3_device::DisjointTile2D",
    ),
    (
        TrustedDeviceItem::DisjointRowStripe2D,
        "fe2o3_device_disjoint_row_stripe_2d",
        "fe2o3_device::DisjointRowStripe2D",
    ),
    (
        TrustedDeviceItem::ThreadIndex1d,
        "fe2o3_device_thread_index_1d",
        "fe2o3_device::thread::index_1d",
    ),
    (
        TrustedDeviceItem::ThreadIndexGet,
        "fe2o3_device_thread_index_get",
        "fe2o3_device::ThreadIndex::get",
    ),
    (
        TrustedDeviceItem::ThreadIndexIntoDisjoint,
        "fe2o3_device_thread_index_into_disjoint",
        "fe2o3_device::ThreadIndex::into_disjoint",
    ),
    (
        TrustedDeviceItem::ThreadIndexCheckedShift,
        "fe2o3_device_thread_index_checked_shift",
        "fe2o3_device::ThreadIndex::checked_shift",
    ),
    (
        TrustedDeviceItem::ThreadIndexCheckedBlock,
        "fe2o3_device_thread_index_checked_block",
        "fe2o3_device::ThreadIndex::checked_block",
    ),
    (
        TrustedDeviceItem::ThreadIndexCheckedTiled2D,
        "fe2o3_device_thread_index_checked_tiled_2d",
        "fe2o3_device::ThreadIndex::checked_tiled_2d",
    ),
    (
        TrustedDeviceItem::ThreadIndexCheckedRowStriped2D,
        "fe2o3_device_thread_index_checked_row_striped_2d",
        "fe2o3_device::ThreadIndex::checked_row_striped_2d",
    ),
    (
        TrustedDeviceItem::DisjointIndexGet,
        "fe2o3_device_disjoint_index_get",
        "fe2o3_device::DisjointIndex::get",
    ),
    (
        TrustedDeviceItem::DisjointIndexCheckedShift,
        "fe2o3_device_disjoint_index_checked_shift",
        "fe2o3_device::DisjointIndex::checked_shift",
    ),
    (
        TrustedDeviceItem::DisjointBlockComponentIndex,
        "fe2o3_device_disjoint_block_component_index",
        "fe2o3_device::DisjointBlock::component_index",
    ),
    (
        TrustedDeviceItem::GridLeaderCurrent,
        "fe2o3_device_grid_leader_current",
        "fe2o3_device::thread::grid_leader",
    ),
    (
        TrustedDeviceItem::ThreadIndexOffset,
        "fe2o3_device_thread_index_offset",
        "fe2o3_device::ThreadIndex::offset",
    ),
    (
        TrustedDeviceItem::ThreadIndexOffsetSigned,
        "fe2o3_device_thread_index_offset_signed",
        "fe2o3_device::ThreadIndex::offset_signed",
    ),
    (
        TrustedDeviceItem::ThreadIndexStride,
        "fe2o3_device_thread_index_stride",
        "fe2o3_device::ThreadIndex::stride",
    ),
    (
        TrustedDeviceItem::ThreadIndexStrideOffset,
        "fe2o3_device_thread_index_stride_offset",
        "fe2o3_device::ThreadIndex::stride_offset",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetMut,
        "fe2o3_device_disjoint_slice_get_mut",
        "fe2o3_device::DisjointSlice::<T>::get_mut",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetDisjointMut,
        "fe2o3_device_disjoint_slice_get_disjoint_mut",
        "fe2o3_device::DisjointSlice::<T>::get_disjoint_mut",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetMutExclusive,
        "fe2o3_device_disjoint_slice_get_mut_exclusive",
        "fe2o3_device::DisjointSlice::<T, GridExclusive>::get_mut_exclusive",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetBlockMut,
        "fe2o3_device_disjoint_slice_get_block_mut",
        "fe2o3_device::DisjointSlice::<T, Blocked<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE>>::get_block_mut",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetTiled2DMut,
        "fe2o3_device_disjoint_slice_get_tiled_2d_mut",
        "fe2o3_device::DisjointSlice::<T, Tiled2D<IndexSpace, LANES_PER_TILE, TILE_ROWS, TILE_COLUMNS, ELEMENTS_PER_LANE>>::get_tiled_2d_mut",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetRowStriped2DMut,
        "fe2o3_device_disjoint_slice_get_row_striped_2d_mut",
        "fe2o3_device::DisjointSlice::<T, RowStriped2D<IndexSpace, LANES_PER_ROW, ELEMENTS_PER_LANE>>::get_row_striped_2d_mut",
    ),
    (
        TrustedDeviceItem::DisjointSliceGetMutAt,
        "fe2o3_device_disjoint_slice_get_mut_at",
        "fe2o3_device::DisjointSlice::<T>::get_mut_at",
    ),
    (
        TrustedDeviceItem::DisjointSliceLen,
        "fe2o3_device_disjoint_slice_len",
        "fe2o3_device::DisjointSlice::<T>::len",
    ),
    (
        TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic,
        "fe2o3_device_global_mut_ptr_u32_as_atomic_v1",
        "fe2o3_device::DeviceGlobalMutPtr::<u32>::as_atomic",
    ),
    (
        TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic,
        "fe2o3_device_global_mut_ptr_i32_as_atomic_v1",
        "fe2o3_device::DeviceGlobalMutPtr::<i32>::as_atomic",
    ),
    (
        TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic,
        "fe2o3_device_global_mut_ptr_u64_as_atomic_v1",
        "fe2o3_device::DeviceGlobalMutPtr::<u64>::as_atomic",
    ),
    (
        TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
        "fe2o3_device_global_mut_ptr_i64_as_atomic_v1",
        "fe2o3_device::DeviceGlobalMutPtr::<i64>::as_atomic",
    ),
    (
        TrustedDeviceItem::MemoryOffsetFrom,
        "fe2o3_device_memory_offset_from_v1",
        "fe2o3_device::memory::offset_from",
    ),
    (
        TrustedDeviceItem::MemoryVolatileLoad,
        "fe2o3_device_memory_volatile_load_v1",
        "fe2o3_device::memory::volatile_load",
    ),
    (
        TrustedDeviceItem::MemoryVolatileStore,
        "fe2o3_device_memory_volatile_store_v1",
        "fe2o3_device::memory::volatile_store",
    ),
    (
        TrustedDeviceItem::MemoryCopyNonOverlapping,
        "fe2o3_device_memory_copy_nonoverlapping_v1",
        "fe2o3_device::memory::copy_nonoverlapping_unchecked",
    ),
    (
        TrustedDeviceItem::MemoryCopyOneNonOverlapping,
        "fe2o3_device_memory_copy_one_nonoverlapping_v1",
        "fe2o3_device::memory::copy_one_nonoverlapping",
    ),
    (
        TrustedDeviceItem::Gfx942CollectivesContext,
        "fe2o3_device_gfx942_collectives_context_v1",
        "fe2o3_device::Gfx942Collectives",
    ),
    (
        TrustedDeviceItem::Gfx942CollectivesCurrent,
        "fe2o3_device_gfx942_collectives_current_v1",
        "fe2o3_device::Gfx942Collectives::current",
    ),
    (
        TrustedDeviceItem::Gfx942SubgroupReduceSumF32,
        "fe2o3_device_gfx942_subgroup_reduce_sum_f32_v1",
        "fe2o3_device::Gfx942Collectives::subgroup_reduce_sum_f32",
    ),
    (
        TrustedDeviceItem::Gfx942SubgroupReduceMaxF32,
        "fe2o3_device_gfx942_subgroup_reduce_max_f32_v1",
        "fe2o3_device::Gfx942Collectives::subgroup_reduce_max_f32",
    ),
    (
        TrustedDeviceItem::Gfx942StaticLdsU32x256,
        "fe2o3_device_gfx942_static_lds_u32x256_v1",
        "fe2o3_device::Gfx942Collectives::static_lds_u32x256",
    ),
    (
        TrustedDeviceItem::Gfx942StaticLdsU32x256Type,
        "fe2o3_device_gfx942_static_lds_u32x256_type_v1",
        "fe2o3_device::Gfx942StaticLdsU32x256",
    ),
    (
        TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
        "fe2o3_device_gfx942_wave64_reduce_active_u32_v1",
        "fe2o3_device::Gfx942Collectives::wave64_reduce_sum_active_u32",
    ),
    (
        TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
        "fe2o3_device_gfx942_workgroup256_reduce_active_u32_v1",
        "fe2o3_device::Gfx942Collectives::workgroup256_reduce_sum_active_u32",
    ),
    (
        TrustedDeviceItem::Gfx942Wave64ReduceSum,
        "fe2o3_device_gfx942_wave64_reduce_sum_v1",
        "fe2o3_device::SubgroupTile::<64>::reduce_sum",
    ),
    (
        TrustedDeviceItem::Gfx942Wave64InclusiveScanSum,
        "fe2o3_device_gfx942_wave64_inclusive_scan_sum_v1",
        "fe2o3_device::SubgroupTile::<64>::inclusive_scan_sum",
    ),
    (
        TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum,
        "fe2o3_device_gfx942_wave64_exclusive_scan_sum_v1",
        "fe2o3_device::SubgroupTile::<64>::exclusive_scan_sum",
    ),
    (
        TrustedDeviceItem::Gfx942WorkgroupReduceSum,
        "fe2o3_device_gfx942_workgroup_reduce_sum_v1",
        "fe2o3_device::Workgroup::reduce_sum",
    ),
    (
        TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum,
        "fe2o3_device_gfx942_workgroup_inclusive_scan_sum_v1",
        "fe2o3_device::Workgroup::inclusive_scan_sum",
    ),
    (
        TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum,
        "fe2o3_device_gfx942_workgroup_exclusive_scan_sum_v1",
        "fe2o3_device::Workgroup::exclusive_scan_sum",
    ),
    (
        TrustedDeviceItem::Gfx942BarrierArrive,
        "fe2o3_device_gfx942_barrier_arrive_v1",
        "fe2o3_device::sync::gfx942_barrier_arrive",
    ),
    (
        TrustedDeviceItem::Gfx942BarrierWait,
        "fe2o3_device_gfx942_barrier_wait_v1",
        "fe2o3_device::sync::gfx942_barrier_wait",
    ),
    (
        TrustedDeviceItem::WaveLane,
        "fe2o3_device_wave_lane",
        "fe2o3_device::WaveLane",
    ),
    (
        TrustedDeviceItem::Wave64,
        "fe2o3_device_wave64_width_v1",
        "fe2o3_device::Wave64",
    ),
    (
        TrustedDeviceItem::WaveLaneCurrent,
        "fe2o3_device_wave_lane_current",
        "fe2o3_device::WaveLane::current",
    ),
    (
        TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
        "fe2o3_device_gfx942_lds_bf16_tile_pair_m16x16_v1",
        "fe2o3_device::gfx942_lds_bf16_tile_pair_m16x16_v1",
    ),
    (
        TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16,
        "fe2o3_device_gfx942_lds_bf16_tile_pair_publish_v1",
        "fe2o3_device::gfx942_publish_lds_bf16_tile_pair_m16x16_v1",
    ),
    (
        TrustedDeviceItem::LdsTile16x16WriteMfmaBf16,
        "fe2o3_device_lds_tile16x16_write_mfma_bf16_v1",
        "fe2o3_device::LdsTile16x16::write_mfma_fragment",
    ),
    (
        TrustedDeviceItem::LdsTile16x16ReadMfmaBf16,
        "fe2o3_device_lds_tile16x16_read_mfma_bf16_v1",
        "fe2o3_device::LdsTile16x16::read_mfma_fragment",
    ),
    (
        TrustedDeviceItem::WorkgroupSyncthreads,
        "fe2o3_device_workgroup_syncthreads_v1",
        "fe2o3_device::sync::syncthreads",
    ),
    (
        TrustedDeviceItem::DeviceMatrix,
        "fe2o3_device_matrix_context_v1",
        "fe2o3_device::DeviceMatrix",
    ),
    (
        TrustedDeviceItem::DeviceMatrixCurrent,
        "fe2o3_device_matrix_context_current_v1",
        "fe2o3_device::DeviceMatrix::current",
    ),
    (
        TrustedDeviceItem::Bf16MfmaProfile,
        "fe2o3_device_mfma_bf16_f32_m16n16k16_profile_v1",
        "fe2o3_device::Bf16F32M16N16K16",
    ),
    (
        TrustedDeviceItem::MfmaOperandA,
        "fe2o3_device_mfma_operand_a_role_v1",
        "fe2o3_device::MfmaOperandA",
    ),
    (
        TrustedDeviceItem::MfmaOperandB,
        "fe2o3_device_mfma_operand_b_role_v1",
        "fe2o3_device::MfmaOperandB",
    ),
    (
        TrustedDeviceItem::MfmaRegisterTile16x16,
        "fe2o3_device_mfma_tile16x16_register_distribution_v1",
        "fe2o3_device::MfmaRegisterTile16x16",
    ),
    (
        TrustedDeviceItem::MfmaLdsXor4Storage,
        "fe2o3_device_mfma_lds_xor4_storage_layout_v1",
        "fe2o3_device::MfmaLdsXor4",
    ),
    (
        TrustedDeviceItem::MfmaAccumulatorRowMajor,
        "fe2o3_device_mfma_accumulator_row_major_distribution_v1",
        "fe2o3_device::MfmaAccumulatorRowMajor",
    ),
    (
        TrustedDeviceItem::Bf16MfmaFragment,
        "fe2o3_device_bf16_mfma_fragment_v1",
        "fe2o3_device::Bf16MfmaFragment",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragment,
        "fe2o3_device_f32_accumulator_fragment_v1",
        "fe2o3_device::F32AccumulatorFragment",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragmentZero,
        "fe2o3_device_f32_accumulator_fragment_zero_v1",
        "fe2o3_device::F32AccumulatorFragment::zero",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
        "fe2o3_device_f32_accumulator_fragment_into_values_v1",
        "fe2o3_device::F32AccumulatorFragment::into_values",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixView,
        "fe2o3_device_bf16_mfma_matrix_view_v1",
        "fe2o3_device::Bf16MfmaMatrix",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixViewError,
        "fe2o3_device_bf16_mfma_matrix_view_error_v1",
        "fe2o3_device::Bf16MatrixViewError",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixARowMajor,
        "fe2o3_device_bf16_mfma_matrix_a_row_major_v1",
        "fe2o3_device::Bf16MfmaAMatrix::row_major",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixBRowMajor,
        "fe2o3_device_bf16_mfma_matrix_b_row_major_v1",
        "fe2o3_device::Bf16MfmaBMatrix::row_major",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2,
        "fe2o3_device_bf16_mfma_matrix_a_load_zero_filled_v2",
        "fe2o3_device::Bf16MfmaAMatrix::load_m16k16",
    ),
    (
        TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2,
        "fe2o3_device_bf16_mfma_matrix_b_load_zero_filled_v2",
        "fe2o3_device::Bf16MfmaBMatrix::load_k16n16",
    ),
    (
        TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
        "fe2o3_device_matrix_mfma_bf16_f32_m16n16k16_v1",
        "fe2o3_device::DeviceMatrix::multiply_accumulate",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Acquire,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_acquire_v1",
        "fe2o3_gemm_device_v1::acquire_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Stage,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_stage_v1",
        "fe2o3_gemm_device_v1::stage_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Publish,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_publish_v1",
        "fe2o3_gemm_device_v1::publish_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Mfma,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_mfma_v1",
        "fe2o3_gemm_device_v1::mfma_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Reuse,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_reuse_v1",
        "fe2o3_gemm_device_v1::reuse_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::Typestate,
            TrustedGeneralGemmOperationV1::Store,
        ),
        "fe2o3_device_general_tiled_gemm_wave64_store_v1",
        "fe2o3_gemm_device_v1::store_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Acquire,
        ),
        "fe2o3_device_general_tiled_gemm_proof_acquire_v1",
        "fe2o3_gemm_device_v1::proof_acquire_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Lane,
        ),
        "fe2o3_device_general_tiled_gemm_proof_lane_v1",
        "fe2o3_gemm_device_v1::proof_lane_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::WorkgroupX,
        ),
        "fe2o3_device_general_tiled_gemm_proof_workgroup_x_v1",
        "fe2o3_gemm_device_v1::proof_workgroup_x_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::WorkgroupY,
        ),
        "fe2o3_device_general_tiled_gemm_proof_workgroup_y_v1",
        "fe2o3_gemm_device_v1::proof_workgroup_y_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::LoadA,
        ),
        "fe2o3_device_general_tiled_gemm_proof_load_a_v1",
        "fe2o3_gemm_device_v1::proof_load_a_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::LoadB,
        ),
        "fe2o3_device_general_tiled_gemm_proof_load_b_v1",
        "fe2o3_gemm_device_v1::proof_load_b_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::LoadC,
        ),
        "fe2o3_device_general_tiled_gemm_proof_load_c_v1",
        "fe2o3_gemm_device_v1::proof_load_c_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Stage,
        ),
        "fe2o3_device_general_tiled_gemm_proof_stage_v1",
        "fe2o3_gemm_device_v1::proof_stage_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::StageValue,
        ),
        "fe2o3_device_general_tiled_gemm_proof_stage_value_v1",
        "fe2o3_gemm_device_v1::proof_stage_value_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::WaitStage,
        ),
        "fe2o3_device_general_tiled_gemm_proof_wait_stage_v1",
        "fe2o3_gemm_device_v1::proof_wait_stage_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::ReadStage,
        ),
        "fe2o3_device_general_tiled_gemm_proof_read_stage_v1",
        "fe2o3_gemm_device_v1::proof_read_stage_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Publish,
        ),
        "fe2o3_device_general_tiled_gemm_proof_publish_v1",
        "fe2o3_gemm_device_v1::proof_publish_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Mfma,
        ),
        "fe2o3_device_general_tiled_gemm_proof_mfma_v1",
        "fe2o3_gemm_device_v1::proof_mfma_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::MfmaValue,
        ),
        "fe2o3_device_general_tiled_gemm_proof_mfma_value_v1",
        "fe2o3_gemm_device_v1::proof_mfma_value_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Reuse,
        ),
        "fe2o3_device_general_tiled_gemm_proof_reuse_v1",
        "fe2o3_gemm_device_v1::proof_reuse_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::Store,
        ),
        "fe2o3_device_general_tiled_gemm_proof_store_v1",
        "fe2o3_gemm_device_v1::proof_store_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::GeneralGemm(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            TrustedGeneralGemmOperationV1::StoreEpilogue,
        ),
        "fe2o3_device_general_tiled_gemm_proof_store_epilogue_v1",
        "fe2o3_gemm_device_v1::proof_store_epilogue_gfx942_tiled_gemm_wave64_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VMovB32),
        "fe2o3_device_amdgpu_v_mov_b32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_mov_b32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAddU32),
        "fe2o3_device_amdgpu_v_add_u32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_add_u32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VSubU32),
        "fe2o3_device_amdgpu_v_sub_u32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_sub_u32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAndB32),
        "fe2o3_device_amdgpu_v_and_b32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_and_b32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VOrB32),
        "fe2o3_device_amdgpu_v_or_b32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_or_b32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VXorB32),
        "fe2o3_device_amdgpu_v_xor_b32_v1",
        "fe2o3_device::diagnostics::__amdgpu_v_xor_b32_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print0),
        "fe2o3_device_gpu_printf_0_v1",
        "fe2o3_device::diagnostics::__gpu_printf_0_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print1),
        "fe2o3_device_gpu_printf_1_v1",
        "fe2o3_device::diagnostics::__gpu_printf_1_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print2),
        "fe2o3_device_gpu_printf_2_v1",
        "fe2o3_device::diagnostics::__gpu_printf_2_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::AssertFail),
        "fe2o3_device_gpu_assert_fail_v1",
        "fe2o3_device::diagnostics::__gpu_assert_fail_v1",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Clock32),
        "fe2o3_device_clock32_v1",
        "fe2o3_device::diagnostics::clock32",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
        "fe2o3_device_trap_v1",
        "fe2o3_device::diagnostics::trap",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::DebugTrap),
        "fe2o3_device_debugtrap_v1",
        "fe2o3_device::diagnostics::debugtrap",
    ),
    (
        TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::ProfilingMarker),
        "fe2o3_device_profiling_marker_v1",
        "fe2o3_device::diagnostics::__profiling_marker_v1",
    ),
];

const HALF_MATH_DIAGNOSTIC_ITEMS: &[(&str, &str)] = &[
    ("fe2o3_device_f16_v1", "fe2o3_device::F16"),
    ("fe2o3_device_bf16_v1", "fe2o3_device::Bf16"),
    ("fe2o3_device_bf16x2_v1", "fe2o3_device::Bf16x2"),
    ("fe2o3_device_math_context_v1", "fe2o3_device::DeviceMath"),
    (
        "fe2o3_device_math_context_from_compiler_v1",
        "fe2o3_device::DeviceMath::current",
    ),
    (
        "fe2o3_device_math_sqrt_f32_v1",
        "fe2o3_device::DeviceMath::sqrt_f32",
    ),
    (
        "fe2o3_device_math_fma_f32_v1",
        "fe2o3_device::DeviceMath::mul_add_f32",
    ),
    (
        "fe2o3_device_math_floor_f32_v1",
        "fe2o3_device::DeviceMath::floor_f32",
    ),
    (
        "fe2o3_device_math_ceil_f32_v1",
        "fe2o3_device::DeviceMath::ceil_f32",
    ),
    (
        "fe2o3_device_math_trunc_f32_v1",
        "fe2o3_device::DeviceMath::trunc_f32",
    ),
    (
        "fe2o3_device_math_roundeven_f32_v1",
        "fe2o3_device::DeviceMath::round_ties_even_f32",
    ),
    (
        "fe2o3_device_math_sin_f32_v1",
        "fe2o3_device::DeviceMath::sin_f32",
    ),
    (
        "fe2o3_device_math_cos_f32_v1",
        "fe2o3_device::DeviceMath::cos_f32",
    ),
    (
        "fe2o3_device_math_exp_f32_v1",
        "fe2o3_device::DeviceMath::exp_f32",
    ),
    (
        "fe2o3_device_math_exp2_f32_v1",
        "fe2o3_device::DeviceMath::exp2_f32",
    ),
    (
        "fe2o3_device_math_log_f32_v1",
        "fe2o3_device::DeviceMath::ln_f32",
    ),
    (
        "fe2o3_device_math_log2_f32_v1",
        "fe2o3_device::DeviceMath::log2_f32",
    ),
    (
        "fe2o3_device_math_log10_f32_v1",
        "fe2o3_device::DeviceMath::log10_f32",
    ),
    (
        "fe2o3_device_math_fma_bf16x2_v1",
        "fe2o3_device::DeviceMath::mul_add_bf16x2",
    ),
];

impl TrustedDeviceItem {
    pub(crate) fn canonical_path(self) -> &'static str {
        match self {
            Self::DeviceValue(value) => half_math_path(Fe2o3DeviceDiagnosticItem::Value(value)),
            Self::DeviceMath(math) => half_math_path(Fe2o3DeviceDiagnosticItem::Math(math)),
            Self::HalfOperation(operation) => operation.canonical_path(),
            _ => TRUSTED_ITEMS
                .iter()
                .find_map(|(item, _, path)| (*item == self).then_some(*path))
                .expect("every trusted device item has one canonical path"),
        }
    }

    pub(crate) const fn expected_provider_crate(self) -> &'static str {
        match self {
            Self::GeneralGemm(_, _) => "fe2o3_gemm_device_v1",
            _ => "fe2o3_device",
        }
    }
}

pub(crate) fn definition(tcx: TyCtxt<'_>, item: TrustedDeviceItem) -> Option<DefId> {
    let marker = TRUSTED_ITEMS
        .iter()
        .find_map(|(candidate, marker, _)| (*candidate == item).then_some(*marker))
        .or_else(|| match item {
            TrustedDeviceItem::DeviceValue(value) => {
                HALF_MATH_DIAGNOSTIC_ITEMS.iter().find_map(|(marker, _)| {
                    (dialect_amdgcn::recognize_fe2o3_device_diagnostic_item(marker)
                        == Some(Fe2o3DeviceDiagnosticItem::Value(value)))
                    .then_some(*marker)
                })
            }
            TrustedDeviceItem::DeviceMath(math) => {
                HALF_MATH_DIAGNOSTIC_ITEMS.iter().find_map(|(marker, _)| {
                    (dialect_amdgcn::recognize_fe2o3_device_diagnostic_item(marker)
                        == Some(Fe2o3DeviceDiagnosticItem::Math(math)))
                    .then_some(*marker)
                })
            }
            _ => None,
        })?;
    let def_id = tcx.get_diagnostic_item(Symbol::intern(marker))?;
    (classify(tcx, def_id) == Some(item)).then_some(def_id)
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<TrustedDeviceItem> {
    let direct = TRUSTED_ITEMS
        .iter()
        .find_map(|(item, marker, _)| {
            (tcx.get_diagnostic_item(Symbol::intern(marker)) == Some(def_id)).then_some(*item)
        })
        .or_else(|| {
            HALF_MATH_DIAGNOSTIC_ITEMS.iter().find_map(|(marker, _)| {
                if tcx.get_diagnostic_item(Symbol::intern(marker)) != Some(def_id) {
                    return None;
                }
                let item = dialect_amdgcn::recognize_fe2o3_device_diagnostic_item(marker)
                    .expect("half/math registry markers must remain canonical");
                Some(match item {
                    Fe2o3DeviceDiagnosticItem::Value(value) => {
                        TrustedDeviceItem::DeviceValue(value)
                    }
                    Fe2o3DeviceDiagnosticItem::Math(math) => TrustedDeviceItem::DeviceMath(math),
                })
            })
        });
    if let Some(item) = direct {
        return provider_rule(tcx, def_id, item).is_ok().then_some(item);
    }
    classify_half_operation(tcx, def_id).map(TrustedDeviceItem::HalfOperation)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn rejected_provider_marker(tcx: TyCtxt<'_>, def_id: DefId) -> Option<&'static str> {
    rejected_provider(tcx, def_id).map(|rejection| rejection.marker)
}

pub(crate) fn rejected_provider(tcx: TyCtxt<'_>, def_id: DefId) -> Option<RejectedTrustedProvider> {
    let (item, marker) = TRUSTED_ITEMS
        .iter()
        .find_map(|(item, marker, _)| {
            (tcx.get_diagnostic_item(Symbol::intern(marker)) == Some(def_id))
                .then_some((*item, *marker))
        })
        .or_else(|| {
            HALF_MATH_DIAGNOSTIC_ITEMS.iter().find_map(|(marker, _)| {
                if tcx.get_diagnostic_item(Symbol::intern(marker)) != Some(def_id) {
                    return None;
                }
                let recognized = dialect_amdgcn::recognize_fe2o3_device_diagnostic_item(marker)?;
                let item = match recognized {
                    Fe2o3DeviceDiagnosticItem::Value(value) => {
                        TrustedDeviceItem::DeviceValue(value)
                    }
                    Fe2o3DeviceDiagnosticItem::Math(math) => TrustedDeviceItem::DeviceMath(math),
                };
                Some((item, *marker))
            })
        })?;
    provider_rule(tcx, def_id, item)
        .err()
        .map(|reason| RejectedTrustedProvider {
            marker,
            expected_provider_crate: item.expected_provider_crate(),
            reason,
        })
}

fn provider_rule(tcx: TyCtxt<'_>, def_id: DefId, item: TrustedDeviceItem) -> Result<(), String> {
    if matches!(item, TrustedDeviceItem::GeneralGemm(_, _)) {
        reviewed_general_gemm_provider_definition_v1(tcx, def_id, item)
    } else {
        let definition = reviewed_provider_semantic_definition_v1(tcx, def_id)?;
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &definition)
    }
}

fn validate_reviewed_fe2o3_device_provider_definition_v1(
    item: TrustedDeviceItem,
    definition: &ReviewedProviderSemanticDefinitionV1,
) -> Result<(), String> {
    validate_safe_execution_provider_definition_v1(definition)?;
    if let Some(expected_definition_path) = exact_provider_compiler_definition_path_v1(item)
        && definition.canonical_definition_path != expected_definition_path
    {
        return Err(format!(
            "provider definition path is `{}`, expected `{expected_definition_path}` for `{}`",
            definition.canonical_definition_path,
            item.canonical_path()
        ));
    }
    definition.durable_semantic_identity(
        ProviderSemanticDefinitionRoleV1::SemanticTerminal,
        item.canonical_path(),
    )?;
    Ok(())
}
fn exact_provider_compiler_definition_path_v1(item: TrustedDeviceItem) -> Option<&'static str> {
    match item {
        TrustedDeviceItem::KernelError => Some("fe2o3_device::kernel_result::KernelError"),
        TrustedDeviceItem::DisjointSlice => Some("fe2o3_device::DisjointSlice"),
        TrustedDeviceItem::StridedReadView2D => Some("fe2o3_device::views::StridedReadView2D"),
        TrustedDeviceItem::StridedReadView2DError => {
            Some("fe2o3_device::views::StridedReadView2DError")
        }
        TrustedDeviceItem::StridedReadView2DFromSharedSlice => {
            Some("fe2o3_device::views::{impl#4}::from_shared_slice")
        }
        TrustedDeviceItem::StridedReadView2DLoadOr => {
            Some("fe2o3_device::views::{impl#4}::load_or")
        }
        TrustedDeviceItem::ThreadIndex => Some("fe2o3_device::thread::ThreadIndex"),
        TrustedDeviceItem::DisjointIndex => Some("fe2o3_device::thread::DisjointIndex"),
        TrustedDeviceItem::ShiftedIndexSpace => Some("fe2o3_device::thread::Shifted"),
        TrustedDeviceItem::BlockedIndexSpace => Some("fe2o3_device::thread::Blocked"),
        TrustedDeviceItem::Tiled2DIndexSpace => Some("fe2o3_device::thread::Tiled2D"),
        TrustedDeviceItem::RowStriped2DIndexSpace => Some("fe2o3_device::thread::RowStriped2D"),
        TrustedDeviceItem::GridExclusiveIndexSpace => Some("fe2o3_device::thread::GridExclusive"),
        TrustedDeviceItem::GridLeader => Some("fe2o3_device::thread::GridLeader"),
        TrustedDeviceItem::DisjointBlock => Some("fe2o3_device::thread::DisjointBlock"),
        TrustedDeviceItem::DisjointTile2D => Some("fe2o3_device::thread::DisjointTile2D"),
        TrustedDeviceItem::DisjointRowStripe2D => Some("fe2o3_device::thread::DisjointRowStripe2D"),
        TrustedDeviceItem::ThreadIndex1d => Some("fe2o3_device::thread::index_1d"),
        TrustedDeviceItem::ThreadIndexGet => Some("fe2o3_device::thread::{impl#7}::get"),
        TrustedDeviceItem::ThreadIndexIntoDisjoint => {
            Some("fe2o3_device::thread::{impl#7}::into_disjoint")
        }
        TrustedDeviceItem::ThreadIndexCheckedShift => {
            Some("fe2o3_device::thread::{impl#7}::checked_shift")
        }
        TrustedDeviceItem::ThreadIndexCheckedBlock => {
            Some("fe2o3_device::thread::{impl#7}::checked_block")
        }
        TrustedDeviceItem::ThreadIndexCheckedTiled2D => {
            Some("fe2o3_device::thread::{impl#7}::checked_tiled_2d")
        }
        TrustedDeviceItem::ThreadIndexCheckedRowStriped2D => {
            Some("fe2o3_device::thread::{impl#7}::checked_row_striped_2d")
        }
        TrustedDeviceItem::DisjointIndexGet => Some("fe2o3_device::thread::{impl#8}::get"),
        TrustedDeviceItem::DisjointIndexCheckedShift => {
            Some("fe2o3_device::thread::{impl#8}::checked_shift")
        }
        TrustedDeviceItem::DisjointBlockComponentIndex => {
            Some("fe2o3_device::thread::{impl#10}::component_index")
        }
        TrustedDeviceItem::GridLeaderCurrent => Some("fe2o3_device::thread::grid_leader"),
        TrustedDeviceItem::DisjointSliceGetMut => Some("fe2o3_device::{impl#0}::get_mut"),
        TrustedDeviceItem::DisjointSliceGetDisjointMut => {
            Some("fe2o3_device::{impl#0}::get_disjoint_mut")
        }
        TrustedDeviceItem::DisjointSliceGetMutExclusive => {
            Some("fe2o3_device::{impl#1}::get_mut_exclusive")
        }
        TrustedDeviceItem::DisjointSliceGetBlockMut => {
            Some("fe2o3_device::{impl#2}::get_block_mut")
        }
        TrustedDeviceItem::DisjointSliceGetTiled2DMut => {
            Some("fe2o3_device::{impl#3}::get_tiled_2d_mut")
        }
        TrustedDeviceItem::DisjointSliceGetRowStriped2DMut => {
            Some("fe2o3_device::{impl#4}::get_row_striped_2d_mut")
        }
        TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic => {
            Some("fe2o3_device::atomic::{impl#0}::as_atomic")
        }
        TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic => {
            Some("fe2o3_device::atomic::{impl#1}::as_atomic")
        }
        TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic => {
            Some("fe2o3_device::atomic::{impl#2}::as_atomic")
        }
        TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic => {
            Some("fe2o3_device::atomic::{impl#3}::as_atomic")
        }
        _ if safe_execution_provider_bound_item(item) => {
            Some(safe_execution_compiler_definition_path(item))
        }
        _ => None,
    }
}
fn validate_safe_execution_provider_definition_v1(
    definition: &ReviewedProviderSemanticDefinitionV1,
) -> Result<(), String> {
    definition.validate()?;
    if definition.profile != ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4 {
        return Err("safe execution provider semantic profile was substituted".to_owned());
    }
    if definition.source_closure_identity != REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1 {
        return Err(format!(
            "safe execution provider source closure does not match the reviewed V1 identity: {:02x?}",
            definition.source_closure_identity
        ));
    }
    Ok(())
}

fn safe_execution_compiler_definition_path(item: TrustedDeviceItem) -> &'static str {
    match item {
        TrustedDeviceItem::WorkgroupLdsScope => "fe2o3_device::lds::WorkgroupLdsScope",
        TrustedDeviceItem::WorkgroupLdsScopeCurrent => "fe2o3_device::lds::{impl#2}::current",
        TrustedDeviceItem::DynamicLdsExactCurrent => "fe2o3_device::lds::{impl#4}::exact_current",
        TrustedDeviceItem::Invocation3D => "fe2o3_device::thread::Invocation3D",
        TrustedDeviceItem::Invocation3DCurrent => "fe2o3_device::thread::{impl#6}::current",
        TrustedDeviceItem::Gfx942CollectivesContext => {
            "fe2o3_device::collective::Gfx942Collectives"
        }
        TrustedDeviceItem::Gfx942CollectivesCurrent => {
            "fe2o3_device::collective::{impl#0}::current"
        }
        TrustedDeviceItem::Gfx942SubgroupReduceSumF32 => {
            "fe2o3_device::collective::{impl#0}::subgroup_reduce_sum_f32"
        }
        TrustedDeviceItem::Gfx942SubgroupReduceMaxF32 => {
            "fe2o3_device::collective::{impl#0}::subgroup_reduce_max_f32"
        }
        TrustedDeviceItem::Gfx942StaticLdsU32x256 => {
            "fe2o3_device::collective::{impl#0}::static_lds_u32x256"
        }
        TrustedDeviceItem::Gfx942StaticLdsU32x256Type => {
            "fe2o3_device::collective::Gfx942StaticLdsU32x256"
        }
        TrustedDeviceItem::Gfx942Wave64ReduceActiveU32 => {
            "fe2o3_device::collective::{impl#0}::wave64_reduce_sum_active_u32"
        }
        TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32 => {
            "fe2o3_device::collective::{impl#0}::workgroup256_reduce_sum_active_u32"
        }
        TrustedDeviceItem::Gfx942Wave64ReduceSum => {
            "fe2o3_device::collective::{impl#5}::reduce_sum"
        }
        TrustedDeviceItem::Gfx942Wave64InclusiveScanSum => {
            "fe2o3_device::collective::{impl#5}::inclusive_scan_sum"
        }
        TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum => {
            "fe2o3_device::collective::{impl#5}::exclusive_scan_sum"
        }
        TrustedDeviceItem::Gfx942WorkgroupReduceSum => {
            "fe2o3_device::collective::{impl#6}::reduce_sum"
        }
        TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum => {
            "fe2o3_device::collective::{impl#6}::inclusive_scan_sum"
        }
        TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum => {
            "fe2o3_device::collective::{impl#6}::exclusive_scan_sum"
        }
        TrustedDeviceItem::WaveLane => "fe2o3_device::wave::WaveLane",
        TrustedDeviceItem::Wave64 => "fe2o3_device::wave::Wave64",
        TrustedDeviceItem::WaveLaneCurrent => "fe2o3_device::wave::{impl#4}::current",
        TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16 => {
            "fe2o3_device::tensor::gfx942_lds_bf16_tile_pair_m16x16_v1"
        }
        TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16 => {
            "fe2o3_device::tensor::gfx942_publish_lds_bf16_tile_pair_m16x16_v1"
        }
        TrustedDeviceItem::LdsTile16x16WriteMfmaBf16 => {
            "fe2o3_device::tensor::{impl#16}::write_mfma_fragment"
        }
        TrustedDeviceItem::LdsTile16x16ReadMfmaBf16 => {
            "fe2o3_device::tensor::{impl#17}::read_mfma_fragment"
        }
        TrustedDeviceItem::WorkgroupSyncthreads => "fe2o3_device::sync::syncthreads",
        TrustedDeviceItem::DeviceMatrix => "fe2o3_device::tensor::DeviceMatrix",
        TrustedDeviceItem::DeviceMatrixCurrent => "fe2o3_device::tensor::{impl#9}::current",
        TrustedDeviceItem::Bf16MfmaProfile => "fe2o3_device::tensor::Bf16F32M16N16K16",
        TrustedDeviceItem::MfmaOperandA => "fe2o3_device::tensor::MfmaOperandA",
        TrustedDeviceItem::MfmaOperandB => "fe2o3_device::tensor::MfmaOperandB",
        TrustedDeviceItem::MfmaRegisterTile16x16 => "fe2o3_device::tensor::MfmaRegisterTile16x16",
        TrustedDeviceItem::MfmaLdsXor4Storage => "fe2o3_device::tensor::MfmaLdsXor4",
        TrustedDeviceItem::MfmaAccumulatorRowMajor => {
            "fe2o3_device::tensor::MfmaAccumulatorRowMajor"
        }
        TrustedDeviceItem::Bf16MfmaFragment => "fe2o3_device::tensor::Bf16MfmaFragment",
        TrustedDeviceItem::F32AccumulatorFragment => "fe2o3_device::tensor::F32AccumulatorFragment",
        TrustedDeviceItem::F32AccumulatorFragmentZero => "fe2o3_device::tensor::{impl#4}::zero",
        TrustedDeviceItem::F32AccumulatorFragmentIntoValues => {
            "fe2o3_device::tensor::{impl#5}::into_values"
        }
        TrustedDeviceItem::Bf16MfmaMatrixView => "fe2o3_device::tensor::Bf16MfmaMatrix",
        TrustedDeviceItem::Bf16MfmaMatrixViewError => "fe2o3_device::tensor::Bf16MatrixViewError",
        TrustedDeviceItem::Bf16MfmaMatrixARowMajor => "fe2o3_device::tensor::{impl#7}::row_major",
        TrustedDeviceItem::Bf16MfmaMatrixBRowMajor => "fe2o3_device::tensor::{impl#8}::row_major",
        TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2 => {
            "fe2o3_device::tensor::{impl#7}::load_m16k16"
        }
        TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2 => {
            "fe2o3_device::tensor::{impl#8}::load_k16n16"
        }
        TrustedDeviceItem::DeviceMatrixMultiplyAccumulate => {
            "fe2o3_device::tensor::{impl#9}::multiply_accumulate"
        }
        _ => item.canonical_path(),
    }
}

const fn safe_execution_provider_bound_item(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::WorkgroupLdsScope
            | TrustedDeviceItem::WorkgroupLdsScopeCurrent
            | TrustedDeviceItem::DynamicLdsExactCurrent
            | TrustedDeviceItem::Invocation3D
            | TrustedDeviceItem::Invocation3DCurrent
            | TrustedDeviceItem::ThreadIndexX
            | TrustedDeviceItem::ThreadIndexY
            | TrustedDeviceItem::ThreadIndexZ
            | TrustedDeviceItem::WorkgroupIndexX
            | TrustedDeviceItem::WorkgroupIndexY
            | TrustedDeviceItem::WorkgroupIndexZ
            | TrustedDeviceItem::WorkgroupDimensionX
            | TrustedDeviceItem::WorkgroupDimensionY
            | TrustedDeviceItem::WorkgroupDimensionZ
            | TrustedDeviceItem::GridDimensionX
            | TrustedDeviceItem::GridDimensionY
            | TrustedDeviceItem::GridDimensionZ
            | TrustedDeviceItem::Gfx942CollectivesContext
            | TrustedDeviceItem::Gfx942CollectivesCurrent
            | TrustedDeviceItem::Gfx942SubgroupReduceSumF32
            | TrustedDeviceItem::Gfx942SubgroupReduceMaxF32
            | TrustedDeviceItem::Gfx942StaticLdsU32x256
            | TrustedDeviceItem::Gfx942StaticLdsU32x256Type
            | TrustedDeviceItem::Gfx942Wave64ReduceActiveU32
            | TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32
            | TrustedDeviceItem::Gfx942Wave64ReduceSum
            | TrustedDeviceItem::Gfx942Wave64InclusiveScanSum
            | TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum
            | TrustedDeviceItem::Gfx942WorkgroupReduceSum
            | TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum
            | TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum
            | TrustedDeviceItem::WaveLane
            | TrustedDeviceItem::Wave64
            | TrustedDeviceItem::WaveLaneCurrent
            | TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16
            | TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16
            | TrustedDeviceItem::LdsTile16x16WriteMfmaBf16
            | TrustedDeviceItem::LdsTile16x16ReadMfmaBf16
            | TrustedDeviceItem::WorkgroupSyncthreads
            | TrustedDeviceItem::DeviceMatrix
            | TrustedDeviceItem::DeviceMatrixCurrent
            | TrustedDeviceItem::Bf16MfmaProfile
            | TrustedDeviceItem::MfmaOperandA
            | TrustedDeviceItem::MfmaOperandB
            | TrustedDeviceItem::MfmaRegisterTile16x16
            | TrustedDeviceItem::MfmaLdsXor4Storage
            | TrustedDeviceItem::MfmaAccumulatorRowMajor
            | TrustedDeviceItem::Bf16MfmaFragment
            | TrustedDeviceItem::F32AccumulatorFragment
            | TrustedDeviceItem::F32AccumulatorFragmentZero
            | TrustedDeviceItem::F32AccumulatorFragmentIntoValues
            | TrustedDeviceItem::Bf16MfmaMatrixView
            | TrustedDeviceItem::Bf16MfmaMatrixViewError
            | TrustedDeviceItem::Bf16MfmaMatrixARowMajor
            | TrustedDeviceItem::Bf16MfmaMatrixBRowMajor
            | TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2
            | TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2
            | TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
            | TrustedDeviceItem::Tiled2DIndexSpace
            | TrustedDeviceItem::RowStriped2DIndexSpace
            | TrustedDeviceItem::DisjointTile2D
            | TrustedDeviceItem::DisjointRowStripe2D
            | TrustedDeviceItem::ThreadIndexCheckedTiled2D
            | TrustedDeviceItem::ThreadIndexCheckedRowStriped2D
            | TrustedDeviceItem::DisjointSliceGetTiled2DMut
            | TrustedDeviceItem::DisjointSliceGetRowStriped2DMut
            | TrustedDeviceItem::StridedReadView2D
            | TrustedDeviceItem::StridedReadView2DError
            | TrustedDeviceItem::StridedReadView2DFromSharedSlice
            | TrustedDeviceItem::StridedReadView2DLoadOr
    )
}

fn named_external_provider(
    tcx: TyCtxt<'_>,
    crate_num: rustc_hir::def_id::CrateNum,
) -> Result<String, String> {
    named_external_provider_as(tcx, crate_num, "fe2o3_device")
}

fn named_external_provider_as(
    tcx: TyCtxt<'_>,
    crate_num: rustc_hir::def_id::CrateNum,
    expected_crate_name: &str,
) -> Result<String, String> {
    if crate_num == LOCAL_CRATE {
        return Err("provider is the local compilation crate".to_owned());
    }
    let crate_name = tcx.crate_name(crate_num).as_str().to_owned();
    if crate_name != expected_crate_name {
        return Err(format!("provider crate name is `{crate_name}`"));
    }
    Ok(crate_name)
}

fn reviewed_general_gemm_provider_definition_v1(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
    item: TrustedDeviceItem,
) -> Result<(), String> {
    let crate_name =
        named_external_provider_as(tcx, provider_definition.krate, "fe2o3_gemm_device_v1")?;
    let actual_path = tcx.def_path_str(provider_definition);
    if actual_path != item.canonical_path() {
        return Err(format!(
            "provider definition path is `{actual_path}`, expected `{}`",
            item.canonical_path()
        ));
    }
    let source_tree = GENERAL_GEMM_PROVIDER_SOURCE_TREE_V1
        .get_or_init(|| {
            reviewed_source_tree_identity(
                Path::new(REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT),
                GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1,
            )
        })
        .clone()?;
    let provider = compiler_provider_observation_v1(tcx, provider_definition.krate);
    if provider.crate_name != crate_name
        || provider.stable_crate_id == 0
        || provider.crate_hash_observation == [0; 16]
        || source_tree == [0; 32]
    {
        return Err("reviewed general-GEMM provider observation is incomplete".to_owned());
    }
    let TrustedDeviceItem::GeneralGemm(surface, _) = item else {
        return Err("general-GEMM provider rule received a non-GEMM item".to_owned());
    };
    validate_reviewed_general_gemm_source_tree_v1(source_tree)?;
    validate_reviewed_general_gemm_surface_v1(tcx, surface, &provider)?;
    validate_reviewed_general_gemm_dependency_v1(tcx, surface, provider_definition.krate)?;
    Ok(())
}

fn validate_reviewed_general_gemm_source_tree_v1(source_tree: [u8; 32]) -> Result<(), String> {
    if source_tree != REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_TREE_V1 {
        return Err(format!(
            "general-GEMM provider semantic source tree does not match the reviewed V1 identity: {source_tree:02x?}"
        ));
    }
    Ok(())
}

fn validate_reviewed_general_gemm_definition_source_v1(
    surface: TrustedGeneralGemmSurfaceV1,
    definition_source: [u8; 32],
) -> Result<(), String> {
    let expected_definition = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => {
            REVIEWED_GENERAL_GEMM_TYPESTATE_DEFINITION_SOURCE_V1
        }
        TrustedGeneralGemmSurfaceV1::ProofSensitive => {
            REVIEWED_GENERAL_GEMM_PROOF_DEFINITION_SOURCE_V1
        }
    };
    if definition_source != expected_definition {
        return Err(format!(
            "general-GEMM provider definition source does not match the reviewed V1 identity: {definition_source:02x?}"
        ));
    }
    Ok(())
}

fn validate_reviewed_general_gemm_terminal_provider_v1(
    expected: &CompilerProviderObservationV1,
    actual: &CompilerProviderObservationV1,
) -> Result<(), String> {
    if actual != expected {
        return Err(
            "reviewed general-GEMM terminal provider changed within the compiler session"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_reviewed_general_gemm_surface_v1(
    tcx: TyCtxt<'_>,
    surface: TrustedGeneralGemmSurfaceV1,
    provider: &CompilerProviderObservationV1,
) -> Result<(), String> {
    let mut terminal_count = 0_usize;
    for (candidate, marker, canonical_path) in TRUSTED_ITEMS {
        let TrustedDeviceItem::GeneralGemm(candidate_surface, _) = candidate else {
            continue;
        };
        if *candidate_surface != surface {
            continue;
        }
        terminal_count += 1;
        let definition = tcx
            .get_diagnostic_item(Symbol::intern(marker))
            .ok_or_else(|| format!("reviewed general-GEMM terminal `{marker}` is unavailable"))?;
        let actual_provider = compiler_provider_observation_v1(tcx, definition.krate);
        validate_reviewed_general_gemm_terminal_provider_v1(provider, &actual_provider)?;
        let actual_path = tcx.def_path_str(definition);
        if actual_path != *canonical_path {
            return Err(format!(
                "general-GEMM terminal path is `{actual_path}`, expected `{canonical_path}`"
            ));
        }
        let definition_source = reviewed_compiled_provider_source_identity_at_root(
            tcx,
            definition,
            Path::new(REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )?;
        validate_reviewed_general_gemm_definition_source_v1(surface, definition_source)?;
    }
    let expected_terminal_count = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => 6,
        TrustedGeneralGemmSurfaceV1::ProofSensitive => 17,
    };
    if terminal_count != expected_terminal_count {
        return Err(format!(
            "reviewed general-GEMM surface has {terminal_count} terminals, expected {expected_terminal_count}"
        ));
    }
    Ok(())
}

fn validate_reviewed_general_gemm_dependency_v1(
    tcx: TyCtxt<'_>,
    surface: TrustedGeneralGemmSurfaceV1,
    provider_crate: rustc_hir::def_id::CrateNum,
) -> Result<(), String> {
    let store_item = TrustedDeviceItem::GeneralGemm(surface, TrustedGeneralGemmOperationV1::Store);
    let store_marker = TRUSTED_ITEMS
        .iter()
        .find_map(|(candidate, marker, _)| (*candidate == store_item).then_some(*marker))
        .expect("every general-GEMM surface has one store terminal");
    let store = tcx
        .get_diagnostic_item(Symbol::intern(store_marker))
        .ok_or_else(|| format!("reviewed general-GEMM store `{store_marker}` is unavailable"))?;
    if store.krate != provider_crate || tcx.def_kind(store) != DefKind::Fn {
        return Err("reviewed general-GEMM store definition was substituted".to_owned());
    }

    let disjoint_slice = tcx
        .get_diagnostic_item(Symbol::intern("fe2o3_device_disjoint_slice"))
        .ok_or_else(|| "reviewed fe2o3_device DisjointSlice is unavailable".to_owned())?;
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(store).instantiate_identity());
    let context = signature
        .inputs()
        .first()
        .ok_or_else(|| "reviewed general-GEMM store omitted its context".to_owned())?;
    let context = match context.kind() {
        TyKind::Ref(_, context, _) => *context,
        _ => *context,
    };
    let TyKind::Adt(context_definition, _) = context.kind() else {
        return Err("reviewed general-GEMM store context is not a provider ADT".to_owned());
    };
    let expected_context_path = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => "fe2o3_gemm_device_v1::Gfx942TiledGemmWave64V1",
        TrustedGeneralGemmSurfaceV1::ProofSensitive => {
            "fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1"
        }
    };
    if context_definition.did().krate != provider_crate
        || tcx.def_path_str(context_definition.did()) != expected_context_path
    {
        return Err("reviewed general-GEMM store context definition was substituted".to_owned());
    }
    let context_source = reviewed_compiled_provider_source_identity_at_root(
        tcx,
        context_definition.did(),
        Path::new(REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT),
        GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
    )?;
    validate_reviewed_general_gemm_definition_source_v1(
        TrustedGeneralGemmSurfaceV1::Typestate,
        context_source,
    )?;

    let c = signature
        .inputs()
        .get(1)
        .ok_or_else(|| "reviewed general-GEMM store omitted its C slice".to_owned())?;
    let TyKind::Ref(_, c, _) = c.kind() else {
        return Err("reviewed general-GEMM store C argument is not a reference".to_owned());
    };
    let TyKind::Adt(c_definition, c_arguments) = c.kind() else {
        return Err("reviewed general-GEMM store C argument is not DisjointSlice".to_owned());
    };
    if c_definition.did() != disjoint_slice
        || c_arguments.len() != 2
        || !c_arguments
            .first()
            .and_then(|argument| argument.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Float(FloatTy::F32)))
    {
        return Err(
            "reviewed general-GEMM store substituted its fe2o3_device DisjointSlice dependency"
                .to_owned(),
        );
    }

    let dependency = reviewed_provider_semantic_definition_v1(tcx, disjoint_slice)?;
    let compiled_dependency_source = reviewed_compiled_provider_source_identity_at_root(
        tcx,
        disjoint_slice,
        Path::new(REVIEWED_FE2O3_DEVICE_SOURCE_ROOT),
        WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
    )?;
    if dependency.canonical_definition_path != "fe2o3_device::DisjointSlice" {
        return Err("reviewed general-GEMM DisjointSlice dependency path changed".to_owned());
    }
    let dependency_identity = general_gemm_dependency_semantic_identity_v1(
        &dependency,
        compiled_dependency_source,
        ProviderSemanticDefinitionRoleV1::TrustedDefinition,
        "general-gemm-disjoint-slice-dependency-v1",
    )?;
    validate_reviewed_general_gemm_dependency_identity_v1(dependency_identity)
}

fn validate_reviewed_general_gemm_dependency_identity_v1(
    dependency_identity: [u8; 32],
) -> Result<(), String> {
    if dependency_identity != REVIEWED_GENERAL_GEMM_DISJOINT_SLICE_DEPENDENCY_V1 {
        return Err(format!(
            "reviewed general-GEMM DisjointSlice dependency identity changed: {dependency_identity:02x?}"
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn reviewed_matrix_provider_observation(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
) -> Result<ReviewedMatrixProviderObservationV2, String> {
    let crate_num = provider_definition.krate;
    let crate_name = named_external_provider(tcx, crate_num)?;
    let stable_crate_id = tcx.stable_crate_id(crate_num).as_u64();
    let source_identity = reviewed_matrix_source_identity(tcx, provider_definition)?;
    let cargo_metadata_build_observation =
        decode_sha256_environment(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)?;
    Ok(ReviewedMatrixProviderObservationV2 {
        crate_name,
        stable_crate_id,
        crate_hash: tcx.crate_hash(crate_num).as_u128().to_le_bytes(),
        cargo_metadata_build_observation,
        source_identity,
    })
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn reviewed_row_softmax_provider_definition(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
) -> Result<ReviewedRowSoftmaxProviderDefinitionV1, String> {
    let crate_num = provider_definition.krate;
    let crate_name = named_external_provider(tcx, crate_num)?;
    let stable_crate_id = tcx.stable_crate_id(crate_num).as_u64();
    let source_identity = reviewed_provider_source_identity(
        tcx,
        provider_definition,
        ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
    )?;
    let cargo_metadata_build_observation =
        decode_sha256_environment(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)?;
    Ok(ReviewedRowSoftmaxProviderDefinitionV1 {
        crate_name,
        stable_crate_id,
        crate_hash: tcx.crate_hash(crate_num).as_u128().to_le_bytes(),
        cargo_metadata_build_observation,
        source_identity,
    })
}

pub(crate) fn compiler_provider_observation_v1(
    tcx: TyCtxt<'_>,
    crate_num: rustc_hir::def_id::CrateNum,
) -> CompilerProviderObservationV1 {
    CompilerProviderObservationV1 {
        crate_name: tcx.crate_name(crate_num).to_string(),
        stable_crate_id: tcx.stable_crate_id(crate_num).as_u64(),
        crate_hash_observation: tcx.crate_hash(crate_num).as_u128().to_le_bytes(),
    }
}

pub(crate) fn reviewed_provider_semantic_definition_v1(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
) -> Result<ReviewedProviderSemanticDefinitionV1, String> {
    reviewed_provider_semantic_definition_with_profile_v1(
        tcx,
        provider_definition,
        WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        &WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE,
        ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
    )
}

#[allow(
    dead_code,
    reason = "consumed by the staged row-softmax V2 provider protocol"
)]
pub(crate) fn reviewed_row_softmax_provider_semantic_definition_v2(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
) -> Result<ReviewedProviderSemanticDefinitionV1, String> {
    reviewed_provider_semantic_definition_with_profile_v1(
        tcx,
        provider_definition,
        ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
        &ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_V2,
        ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
    )
}

#[allow(
    dead_code,
    reason = "consumed by the staged matrix V3 provider protocol"
)]
pub(crate) fn reviewed_matrix_provider_semantic_definition_v3(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
) -> Result<ReviewedProviderSemanticDefinitionV1, String> {
    reviewed_provider_semantic_definition_with_profile_v1(
        tcx,
        provider_definition,
        MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2,
        MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3,
        &MATRIX_PROVIDER_SOURCE_CLOSURE_V3,
        ReviewedProviderSemanticProfileV1::MatrixV3,
    )
}

fn reviewed_provider_semantic_definition_with_profile_v1(
    tcx: TyCtxt<'_>,
    provider_definition: DefId,
    definition_source_domain: &[u8],
    source_closure_domain: &[u8],
    source_closure_cache: &OnceLock<Result<[u8; 32], String>>,
    profile: ReviewedProviderSemanticProfileV1,
) -> Result<ReviewedProviderSemanticDefinitionV1, String> {
    let crate_num = provider_definition.krate;
    let crate_name = named_external_provider(tcx, crate_num)?;
    let provider = compiler_provider_observation_v1(tcx, crate_num);
    let definition_source_identity = reviewed_compiled_provider_source_identity_at_root(
        tcx,
        provider_definition,
        Path::new(REVIEWED_FE2O3_DEVICE_SOURCE_ROOT),
        definition_source_domain,
    )?;
    let source_closure_identity = source_closure_cache
        .get_or_init(|| {
            reviewed_provider_source_closure_identity(
                Path::new(REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT),
                source_closure_domain,
            )
        })
        .clone()?;
    if provider.crate_name != crate_name {
        return Err("reviewed provider crate-name observation changed within the session".into());
    }
    let structural_local_definition_path = tcx
        .def_path(provider_definition)
        .to_string_no_crate_verbose();
    let canonical_definition_path =
        canonical_compiler_definition_path(&crate_name, &structural_local_definition_path)?;
    let structural_local_definition_component =
        structural_local_definition_component_v1(&structural_local_definition_path)?;
    Ok(ReviewedProviderSemanticDefinitionV1 {
        provider,
        profile,
        canonical_definition_path,
        structural_local_definition_component,
        cargo_metadata_build_observation: decode_sha256_environment(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
        )?,
        source_closure_identity,
        definition_source_identity,
    })
}

fn canonical_compiler_definition_path(
    authenticated_crate_name: &str,
    structural_local_path: &str,
) -> Result<String, String> {
    let structural_local_path = structural_local_path.trim_start_matches("::");
    if authenticated_crate_name.is_empty() || structural_local_path.is_empty() {
        return Err("compiler definition path is incomplete".to_owned());
    }
    Ok(format!(
        "{authenticated_crate_name}::{structural_local_path}"
    ))
}

pub(crate) fn structural_local_definition_component_v1(
    structural_local_path: &str,
) -> Result<[u8; 32], String> {
    let structural_local_path = structural_local_path.trim_start_matches("::");
    if structural_local_path.is_empty() {
        return Err("structural local definition path is empty".to_owned());
    }
    let mut hasher = Sha256::new();
    hash_source_identity_field(&mut hasher, STRUCTURAL_LOCAL_DEFINITION_COMPONENT_DOMAIN_V1);
    hash_source_identity_field(&mut hasher, structural_local_path.as_bytes());
    Ok(hasher.finalize().into())
}

#[cfg(test)]
pub(crate) fn pinned_core_semantic_terminal_identity_v1(
    provider: &CompilerProviderObservationV1,
    canonical_role: &str,
    structural_local_definition_path: &str,
) -> Result<[u8; 32], String> {
    let structural_local_definition_component =
        structural_local_definition_component_v1(structural_local_definition_path)?;
    if provider.crate_name != "core"
        || provider.stable_crate_id == 0
        || provider.crate_hash_observation == [0; 16]
        || canonical_role.is_empty()
        || structural_local_definition_component == [0; 32]
    {
        return Err("pinned core semantic terminal observation is incomplete".to_owned());
    }
    let canonical_definition_path =
        canonical_compiler_definition_path(&provider.crate_name, structural_local_definition_path)?;
    let mut hasher = Sha256::new();
    hash_source_identity_field(
        &mut hasher,
        PINNED_CORE_SEMANTIC_TERMINAL_TRANSCRIPT_DOMAIN_V1,
    );
    hash_source_identity_field(&mut hasher, canonical_role.as_bytes());
    hash_source_identity_field(&mut hasher, provider.crate_name.as_bytes());
    hash_source_identity_field(&mut hasher, canonical_definition_path.as_bytes());
    hash_source_identity_field(&mut hasher, &structural_local_definition_component);
    Ok(hasher.finalize().into())
}

fn reviewed_provider_source_closure_identity(
    package_root: &Path,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    if domain.is_empty() {
        return Err("reviewed provider source-closure domain is empty".to_owned());
    }
    require_directory_without_symlink(package_root, "package root")?;
    let package_root = std::fs::canonicalize(package_root).map_err(|error| {
        format!("reviewed fe2o3-device package root is unavailable to the managed build: {error}")
    })?;
    let mut files = vec![package_root.join("Cargo.toml")];
    let build_script = package_root.join("build.rs");
    match std::fs::symlink_metadata(&build_script) {
        Ok(_) => {
            require_regular_file_without_symlink(&build_script)?;
            files.push(build_script);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "reviewed fe2o3-device source `{}` cannot be inspected: {error}",
                build_script.display()
            ));
        }
    }
    require_regular_file_without_symlink(&package_root.join("Cargo.toml"))?;
    let source_root = package_root.join("src");
    require_directory_without_symlink(&source_root, "source directory")?;
    collect_reviewed_source_files(&source_root, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    hasher.update(domain);
    for file in files {
        let (relative, bytes) = reviewed_source_file(&package_root, &file)?;
        hash_source_identity_field(&mut hasher, relative.as_bytes());
        hash_source_identity_field(&mut hasher, &bytes);
    }
    Ok(hasher.finalize().into())
}

fn reviewed_source_tree_identity(source_root: &Path, domain: &[u8]) -> Result<[u8; 32], String> {
    if domain.is_empty() {
        return Err("reviewed provider source-tree domain is empty".to_owned());
    }
    require_directory_without_symlink(source_root, "source directory")?;
    let source_root = std::fs::canonicalize(source_root).map_err(|error| {
        format!(
            "reviewed provider source tree `{}` is unavailable: {error}",
            source_root.display()
        )
    })?;
    let mut files = Vec::new();
    collect_reviewed_source_files(&source_root, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    hasher.update(domain);
    for file in files {
        let (relative, bytes) = reviewed_source_file(&source_root, &file)?;
        hash_source_identity_field(&mut hasher, relative.as_bytes());
        hash_source_identity_field(&mut hasher, &bytes);
    }
    Ok(hasher.finalize().into())
}

fn require_directory_without_symlink(path: &Path, description: &str) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "reviewed fe2o3-device {description} `{}` is unavailable: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "reviewed fe2o3-device {description} `{}` is not a regular directory",
            path.display()
        ));
    }
    Ok(())
}

fn require_regular_file_without_symlink(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "reviewed fe2o3-device source `{}` is unavailable: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "reviewed fe2o3-device source `{}` is not a regular file",
            path.display()
        ));
    }
    Ok(())
}

fn reviewed_source_file(package_root: &Path, file: &Path) -> Result<(String, Vec<u8>), String> {
    require_regular_file_without_symlink(file)?;
    let canonical = std::fs::canonicalize(file).map_err(|error| {
        format!(
            "reviewed fe2o3-device source `{}` is unavailable: {error}",
            file.display()
        )
    })?;
    let relative = canonical.strip_prefix(package_root).map_err(|_| {
        format!(
            "reviewed fe2o3-device source `{}` escaped its package root",
            canonical.display()
        )
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        format!(
            "reviewed fe2o3-device source path `{}` is not UTF-8",
            relative.display()
        )
    })?;
    let bytes = std::fs::read(&canonical).map_err(|error| {
        format!(
            "reviewed fe2o3-device source `{}` cannot be observed: {error}",
            canonical.display()
        )
    })?;
    Ok((relative.to_owned(), bytes))
}

fn collect_reviewed_source_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(root).map_err(|error| {
        format!(
            "reviewed fe2o3-device source directory `{}` is unavailable: {error}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "reviewed fe2o3-device source directory `{}` cannot be read: {error}",
                root.display()
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "reviewed fe2o3-device source `{}` cannot be inspected: {error}",
                entry.path().display()
            )
        })?;
        if file_type.is_dir() {
            collect_reviewed_source_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        } else {
            return Err(format!(
                "reviewed fe2o3-device source `{}` is not a regular file",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn hash_source_identity_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn reviewed_matrix_source_identity(tcx: TyCtxt<'_>, def_id: DefId) -> Result<[u8; 32], String> {
    reviewed_provider_source_identity(tcx, def_id, MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn reviewed_provider_source_identity(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    reviewed_provider_source_identity_at_root(
        tcx,
        def_id,
        Path::new(REVIEWED_FE2O3_DEVICE_SOURCE_ROOT),
        domain,
    )
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn reviewed_provider_source_identity_at_root(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    reviewed_root: &Path,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    let file_name = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(def_id))
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let reviewed_root = std::fs::canonicalize(reviewed_root).map_err(|error| {
        format!("reviewed provider source root is unavailable to the managed build: {error}")
    })?;
    reviewed_provider_source_identity_from_path(&reviewed_root, Path::new(&file_name), domain)
}

fn reviewed_compiled_provider_source_identity_at_root(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    reviewed_root: &Path,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    let span = tcx.def_span(def_id);
    let source_file = tcx.sess.source_map().lookup_source_file(span.lo());
    if source_file.cnum != def_id.krate {
        return Err("provider definition source came from a different compiler crate".to_owned());
    }
    let file_name = source_file
        .name
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let source_path = Path::new(&file_name);
    let source_bytes = std::fs::read(source_path).map_err(|error| {
        format!(
            "provider source file `{}` cannot be observed by the managed build: {error}",
            source_path.display()
        )
    })?;
    let source = std::str::from_utf8(&source_bytes).map_err(|_| {
        format!(
            "provider source file `{}` is not UTF-8",
            source_path.display()
        )
    })?;
    validate_compiled_provider_source_hash_v1(&source_file.src_hash, source, source_path)?;
    reviewed_provider_source_identity_from_path(reviewed_root, source_path, domain)
}

fn validate_compiled_provider_source_hash_v1(
    compiled_hash: &SourceFileHash,
    reviewed_source: &str,
    source_path: &Path,
) -> Result<(), String> {
    if compiled_hash.matches(reviewed_source) {
        return Ok(());
    }
    Err(format!(
        "compiled provider source hash does not match reviewed bytes for `{}`",
        source_path.display()
    ))
}

fn reviewed_provider_source_identity_from_path(
    reviewed_root: &Path,
    source: &Path,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    if domain.is_empty() {
        return Err("reviewed provider definition-source domain is empty".to_owned());
    }
    require_regular_file_without_symlink(source)?;
    let source = std::fs::canonicalize(source).map_err(|error| {
        format!(
            "provider source file `{}` is unavailable to the managed build: {error}",
            source.display()
        )
    })?;
    let reviewed_root = std::fs::canonicalize(reviewed_root).map_err(|error| {
        format!("reviewed provider source root is unavailable to the managed build: {error}")
    })?;
    let relative = source.strip_prefix(&reviewed_root).map_err(|_| {
        format!(
            "provider source file `{}` is outside the reviewed fe2o3-device source root",
            source.display()
        )
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        format!(
            "provider source file `{}` has a non-UTF-8 reviewed path",
            source.display()
        )
    })?;
    let source_bytes = std::fs::read(&source).map_err(|error| {
        format!(
            "provider source file `{}` cannot be observed by the managed build: {error}",
            source.display()
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((relative.len() as u64).to_le_bytes());
    hasher.update(relative.as_bytes());
    hasher.update((source_bytes.len() as u64).to_le_bytes());
    hasher.update(source_bytes);
    Ok(hasher.finalize().into())
}

fn decode_sha256_environment(name: &str) -> Result<[u8; 32], String> {
    let value = std::env::var(name).map_err(|_| format!("managed build omitted {name}"))?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("managed build supplied malformed {name}"));
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let text = std::str::from_utf8(pair).expect("ASCII hex is UTF-8");
        *output = u8::from_str_radix(text, 16)
            .map_err(|_| format!("managed build supplied malformed {name}"))?;
    }
    Ok(digest)
}

fn half_math_path(item: Fe2o3DeviceDiagnosticItem) -> &'static str {
    HALF_MATH_DIAGNOSTIC_ITEMS
        .iter()
        .find_map(|(marker, path)| {
            (dialect_amdgcn::recognize_fe2o3_device_diagnostic_item(marker) == Some(item))
                .then_some(*path)
        })
        .expect("every trusted half/math diagnostic item has one canonical path")
}

impl TrustedHalfOperation {
    fn canonical_path(self) -> &'static str {
        use NarrowFloatFormat::{Bf16, F16};
        use WidenedFloatBinaryOp::{Add, Divide, Multiply, Subtract};
        match self {
            Self::FromF32(F16) => "fe2o3_device::F16::from_f32",
            Self::FromF32(Bf16) => "fe2o3_device::Bf16::from_f32",
            Self::ToF32(F16) => "fe2o3_device::F16::to_f32",
            Self::ToF32(Bf16) => "fe2o3_device::Bf16::to_f32",
            Self::WidenedBinary {
                format: F16,
                op: Add,
            } => "<fe2o3_device::F16 as core::ops::Add>::add",
            Self::WidenedBinary {
                format: F16,
                op: Subtract,
            } => "<fe2o3_device::F16 as core::ops::Sub>::sub",
            Self::WidenedBinary {
                format: F16,
                op: Multiply,
            } => "<fe2o3_device::F16 as core::ops::Mul>::mul",
            Self::WidenedBinary {
                format: F16,
                op: Divide,
            } => "<fe2o3_device::F16 as core::ops::Div>::div",
            Self::WidenedBinary {
                format: Bf16,
                op: Add,
            } => "<fe2o3_device::Bf16 as core::ops::Add>::add",
            Self::WidenedBinary {
                format: Bf16,
                op: Subtract,
            } => "<fe2o3_device::Bf16 as core::ops::Sub>::sub",
            Self::WidenedBinary {
                format: Bf16,
                op: Multiply,
            } => "<fe2o3_device::Bf16 as core::ops::Mul>::mul",
            Self::WidenedBinary {
                format: Bf16,
                op: Divide,
            } => "<fe2o3_device::Bf16 as core::ops::Div>::div",
            Self::Bf16x2FusedMultiplyAdd => "fe2o3_device::Bf16x2::mul_add_widened",
        }
    }
}

fn classify_half_operation(tcx: TyCtxt<'_>, def_id: DefId) -> Option<TrustedHalfOperation> {
    let associated = tcx.opt_associated_item(def_id)?;
    if !associated.is_fn() {
        return None;
    }
    let impl_id = tcx.impl_of_assoc(def_id)?;
    if impl_id.krate == LOCAL_CRATE {
        return None;
    }
    let self_ty = tcx.type_of(impl_id).instantiate_identity();
    let TyKind::Adt(adt, _) = self_ty.kind() else {
        return None;
    };
    let value = match classify(tcx, adt.did())? {
        TrustedDeviceItem::DeviceValue(value) => value,
        _ => return None,
    };
    let item_name = tcx.item_name(def_id);
    let name = item_name.as_str();

    if tcx.impl_is_of_trait(impl_id) {
        let trait_id = tcx.impl_trait_id(impl_id);
        let (lang_item, op) = match name {
            "add" => (LangItem::Add, WidenedFloatBinaryOp::Add),
            "sub" => (LangItem::Sub, WidenedFloatBinaryOp::Subtract),
            "mul" => (LangItem::Mul, WidenedFloatBinaryOp::Multiply),
            "div" => (LangItem::Div, WidenedFloatBinaryOp::Divide),
            _ => return None,
        };
        if tcx.lang_items().get(lang_item) != Some(trait_id) {
            return None;
        }
        return Some(TrustedHalfOperation::WidenedBinary {
            format: narrow_format(value)?,
            op,
        });
    }

    match (value, name) {
        (DeviceValueDiagnosticItem::F16, "from_f32") => {
            Some(TrustedHalfOperation::FromF32(NarrowFloatFormat::F16))
        }
        (DeviceValueDiagnosticItem::Bf16, "from_f32") => {
            Some(TrustedHalfOperation::FromF32(NarrowFloatFormat::Bf16))
        }
        (DeviceValueDiagnosticItem::F16, "to_f32") => {
            Some(TrustedHalfOperation::ToF32(NarrowFloatFormat::F16))
        }
        (DeviceValueDiagnosticItem::Bf16, "to_f32") => {
            Some(TrustedHalfOperation::ToF32(NarrowFloatFormat::Bf16))
        }
        (DeviceValueDiagnosticItem::Bf16x2, "mul_add_widened") => {
            Some(TrustedHalfOperation::Bf16x2FusedMultiplyAdd)
        }
        _ => None,
    }
}

const fn narrow_format(value: DeviceValueDiagnosticItem) -> Option<NarrowFloatFormat> {
    match value {
        DeviceValueDiagnosticItem::F16 => Some(NarrowFloatFormat::F16),
        DeviceValueDiagnosticItem::Bf16 => Some(NarrowFloatFormat::Bf16),
        DeviceValueDiagnosticItem::Bf16x2 => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        CompilerProviderObservationV1, GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1, HALF_MATH_DIAGNOSTIC_ITEMS,
        MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3, MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2,
        ProviderSemanticDefinitionExpectationV1, ProviderSemanticDefinitionRoleV1,
        ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
        ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1, ReviewedProviderSemanticDefinitionV1,
        ReviewedProviderSemanticProfileV1, TrustedAmdGpuDiagnosticOperation,
        TrustedAmdGpuInlineOperation, TrustedDeviceItem, TrustedGeneralGemmOperationV1,
        TrustedGeneralGemmSurfaceV1, WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1, canonical_compiler_definition_path,
        exact_provider_compiler_definition_path_v1, general_gemm_dependency_semantic_identity_v1,
        pinned_core_semantic_terminal_identity_v1, reviewed_provider_source_closure_identity,
        reviewed_provider_source_identity_from_path, reviewed_source_tree_identity,
        safe_execution_compiler_definition_path, safe_execution_provider_bound_item,
        structural_local_definition_component_v1, validate_compiled_provider_source_hash_v1,
        validate_ordered_provider_semantic_definitions_v1,
        validate_reviewed_fe2o3_device_provider_definition_v1,
        validate_reviewed_general_gemm_definition_source_v1,
        validate_reviewed_general_gemm_dependency_identity_v1,
        validate_reviewed_general_gemm_source_tree_v1,
        validate_reviewed_general_gemm_terminal_provider_v1,
    };
    use dialect_amdgcn::{DeviceMathDiagnosticItem, DeviceValueDiagnosticItem};
    use rustc_span::{SourceFileHash, SourceFileHashAlgorithm};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct ProviderPackageFixture {
        root: PathBuf,
    }

    impl ProviderPackageFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "fe2o3-provider-profile-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("src/nested")).unwrap();
            fs::write(root.join("Cargo.toml"), b"[package]\nname='fixture'\n").unwrap();
            fs::write(root.join("src/lib.rs"), b"pub mod nested;\n").unwrap();
            fs::write(root.join("src/nested/mod.rs"), b"pub fn value() {}\n").unwrap();
            Self { root }
        }

        fn source_root(&self) -> PathBuf {
            self.root.join("src")
        }

        fn definition(&self) -> PathBuf {
            self.source_root().join("lib.rs")
        }
    }

    impl Drop for ProviderPackageFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn digest(hex: &str) -> [u8; 32] {
        assert_eq!(hex.len(), 64);
        let mut output = [0_u8; 32];
        for (byte, pair) in output.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
        }
        output
    }

    fn semantic_definition(
        profile: ReviewedProviderSemanticProfileV1,
        path: &str,
        source_closure_identity: [u8; 32],
        definition_source_identity: [u8; 32],
    ) -> ReviewedProviderSemanticDefinitionV1 {
        ReviewedProviderSemanticDefinitionV1 {
            provider: CompilerProviderObservationV1 {
                crate_name: "fe2o3_device".into(),
                stable_crate_id: 7,
                crate_hash_observation: [3; 16],
            },
            profile,
            canonical_definition_path: format!("fe2o3_device::{path}"),
            structural_local_definition_component: structural_local_definition_component_v1(path)
                .unwrap(),
            cargo_metadata_build_observation: [4; 32],
            source_closure_identity,
            definition_source_identity,
        }
    }

    #[test]
    fn provider_semantic_identity_excludes_volatile_compilation_disambiguators() {
        fn identity(definition: &ReviewedProviderSemanticDefinitionV1) -> Result<[u8; 32], String> {
            definition.durable_semantic_identity(
                ProviderSemanticDefinitionRoleV1::SemanticTerminal,
                "fe2o3_device::thread::thread_idx_x",
            )
        }

        let definition = ReviewedProviderSemanticDefinitionV1 {
            provider: CompilerProviderObservationV1 {
                crate_name: "fe2o3_device".into(),
                stable_crate_id: 7,
                crate_hash_observation: [3; 16],
            },
            profile: ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            canonical_definition_path: "fe2o3_device::thread::thread_idx_x".into(),
            structural_local_definition_component: structural_local_definition_component_v1(
                "thread::thread_idx_x",
            )
            .unwrap(),
            cargo_metadata_build_observation: [4; 32],
            source_closure_identity: [5; 32],
            definition_source_identity: [6; 32],
        };
        let exact = identity(&definition).expect("complete provider semantic identity");
        assert_eq!(
            exact,
            digest("36349edbdabe77499ba36d983bf758f7c00e982d7fbd930397042192af1e7416")
        );

        let mut mutation = definition.clone();
        mutation.provider.stable_crate_id ^= 1;
        assert_ne!(mutation.provider, definition.provider);
        assert_eq!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.provider.crate_hash_observation[0] ^= 1;
        assert_ne!(mutation.provider, definition.provider);
        assert_eq!(identity(&mutation).unwrap(), exact);

        mutation = definition.clone();
        mutation.provider.crate_name = "fake_fe2o3_device".into();
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.canonical_definition_path = "fe2o3_device::thread::block_idx_x".into();
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.canonical_definition_path = "fe2o3_device::thread::block_idx_x".into();
        mutation.structural_local_definition_component =
            structural_local_definition_component_v1("thread::block_idx_x").unwrap();
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.structural_local_definition_component[0] ^= 1;
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.cargo_metadata_build_observation[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.source_closure_identity[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.definition_source_identity[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.source_closure_identity = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.definition_source_identity = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.cargo_metadata_build_observation = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.provider.stable_crate_id = 0;
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.provider.crate_hash_observation = [0; 16];
        assert!(identity(&mutation).is_err());
        assert_ne!(
            definition
                .durable_semantic_identity(
                    ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                    "fe2o3_device::thread::thread_idx_x",
                )
                .unwrap(),
            exact
        );
        assert!(
            definition
                .durable_semantic_identity(ProviderSemanticDefinitionRoleV1::SemanticTerminal, "",)
                .is_err()
        );
        assert!(
            definition
                .durable_semantic_identity_for_profile(
                    ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
                    ProviderSemanticDefinitionRoleV1::SemanticTerminal,
                    "fe2o3_device::thread::thread_idx_x",
                )
                .is_err()
        );
        assert_ne!(
            definition
                .durable_semantic_identity(
                    ProviderSemanticDefinitionRoleV1::SemanticTerminal,
                    "fe2o3_device::thread::block_idx_x",
                )
                .unwrap(),
            exact
        );
    }

    #[test]
    fn exact_device_provider_rejects_same_name_path_and_source_substitution() {
        let item = TrustedDeviceItem::ThreadIndexCheckedBlock;
        let structural = exact_provider_compiler_definition_path_v1(item).unwrap();
        let local = structural.strip_prefix("fe2o3_device::").unwrap();
        let exact = semantic_definition(
            ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            local,
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact)
            .expect("exact reviewed provider");

        let wrong_path = semantic_definition(
            ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            "thread::{impl#99}::checked_block",
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &wrong_path).is_err());
        let mut impostor = exact.clone();
        impostor.provider.crate_name = "same_name_impostor".into();
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &impostor).is_err());
        impostor = exact.clone();
        impostor.source_closure_identity[0] ^= 1;
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &impostor).is_err());
    }

    #[test]
    fn checked_view_capabilities_reject_type_constructor_and_load_lookalikes() {
        for item in [
            TrustedDeviceItem::StridedReadView2D,
            TrustedDeviceItem::StridedReadView2DError,
            TrustedDeviceItem::StridedReadView2DFromSharedSlice,
            TrustedDeviceItem::StridedReadView2DLoadOr,
            TrustedDeviceItem::Bf16MfmaMatrixView,
            TrustedDeviceItem::Bf16MfmaMatrixViewError,
            TrustedDeviceItem::Bf16MfmaMatrixARowMajor,
            TrustedDeviceItem::Bf16MfmaMatrixBRowMajor,
            TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2,
            TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2,
        ] {
            let structural = exact_provider_compiler_definition_path_v1(item).unwrap();
            let local = structural.strip_prefix("fe2o3_device::").unwrap();
            let exact = semantic_definition(
                ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
                local,
                super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
                [6; 32],
            );
            validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact)
                .expect("exact checked read-view capability");

            let lookalike = semantic_definition(
                ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
                &format!("lookalike::{local}"),
                super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
                [6; 32],
            );
            assert!(
                validate_reviewed_fe2o3_device_provider_definition_v1(item, &lookalike).is_err(),
                "same-signature lookalike authenticated as {item:?}"
            );

            let mut wrong_crate = exact.clone();
            wrong_crate.provider.crate_name = "fe2o3_device_lookalike".into();
            assert!(
                validate_reviewed_fe2o3_device_provider_definition_v1(item, &wrong_crate).is_err(),
                "wrong-crate lookalike authenticated as {item:?}"
            );
        }
    }

    #[test]
    fn every_production_safety_terminal_has_an_exact_structural_path() {
        for item in [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::StridedReadView2D,
            TrustedDeviceItem::StridedReadView2DError,
            TrustedDeviceItem::StridedReadView2DFromSharedSlice,
            TrustedDeviceItem::StridedReadView2DLoadOr,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::DisjointIndex,
            TrustedDeviceItem::ShiftedIndexSpace,
            TrustedDeviceItem::BlockedIndexSpace,
            TrustedDeviceItem::Tiled2DIndexSpace,
            TrustedDeviceItem::RowStriped2DIndexSpace,
            TrustedDeviceItem::GridExclusiveIndexSpace,
            TrustedDeviceItem::GridLeader,
            TrustedDeviceItem::DisjointBlock,
            TrustedDeviceItem::DisjointTile2D,
            TrustedDeviceItem::DisjointRowStripe2D,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexIntoDisjoint,
            TrustedDeviceItem::ThreadIndexCheckedShift,
            TrustedDeviceItem::ThreadIndexCheckedBlock,
            TrustedDeviceItem::ThreadIndexCheckedTiled2D,
            TrustedDeviceItem::ThreadIndexCheckedRowStriped2D,
            TrustedDeviceItem::DisjointIndexGet,
            TrustedDeviceItem::DisjointIndexCheckedShift,
            TrustedDeviceItem::DisjointBlockComponentIndex,
            TrustedDeviceItem::GridLeaderCurrent,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetDisjointMut,
            TrustedDeviceItem::DisjointSliceGetMutExclusive,
            TrustedDeviceItem::DisjointSliceGetBlockMut,
            TrustedDeviceItem::DisjointSliceGetTiled2DMut,
            TrustedDeviceItem::DisjointSliceGetRowStriped2DMut,
            TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
        ] {
            let path = exact_provider_compiler_definition_path_v1(item)
                .unwrap_or_else(|| panic!("{item:?} lacks exact provider authentication"));
            assert!(path.starts_with("fe2o3_device::"));
        }
    }

    #[test]
    fn safe_execution_source_closure_matches_the_reviewed_pin() {
        let closure = reviewed_provider_source_closure_identity(
            Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT),
            WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        assert_eq!(closure, super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1);

        let definition = semantic_definition(
            ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            "thread::thread_idx_x",
            [5; 32],
            [6; 32],
        );
        assert_eq!(
            definition
                .durable_semantic_identity_for_profile(
                    ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
                    ProviderSemanticDefinitionRoleV1::SemanticTerminal,
                    "fe2o3_device::thread::thread_idx_x",
                )
                .unwrap(),
            digest("36349edbdabe77499ba36d983bf758f7c00e982d7fbd930397042192af1e7416")
        );
    }

    #[test]
    fn safe_execution_provider_validation_rejects_source_substitution() {
        let exact = semantic_definition(
            ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            "wave::{impl#4}::current",
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        super::validate_safe_execution_provider_definition_v1(&exact).unwrap();

        let mut changed = exact.clone();
        changed.source_closure_identity[0] ^= 1;
        assert!(super::validate_safe_execution_provider_definition_v1(&changed).is_err());

        changed = exact;
        changed.profile = ReviewedProviderSemanticProfileV1::MatrixV3;
        assert!(super::validate_safe_execution_provider_definition_v1(&changed).is_err());
    }

    #[test]
    fn reviewed_general_gemm_companion_source_is_exactly_pinned() {
        let source_root = Path::new(super::REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT);
        let source_tree =
            reviewed_source_tree_identity(source_root, GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1)
                .unwrap();
        let typestate_definition = reviewed_provider_source_identity_from_path(
            source_root,
            &source_root.join("lib.rs"),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        let proof_definition = reviewed_provider_source_identity_from_path(
            source_root,
            &source_root.join("proof_sensitive_terminals.rs"),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        validate_reviewed_general_gemm_source_tree_v1(source_tree).unwrap();
        validate_reviewed_general_gemm_definition_source_v1(
            TrustedGeneralGemmSurfaceV1::Typestate,
            typestate_definition,
        )
        .unwrap();
        validate_reviewed_general_gemm_definition_source_v1(
            TrustedGeneralGemmSurfaceV1::ProofSensitive,
            proof_definition,
        )
        .unwrap();

        let modified = ProviderPackageFixture::new();
        fs::remove_dir_all(modified.source_root()).unwrap();
        fs::create_dir_all(modified.source_root()).unwrap();
        let mut changed = fs::read(source_root.join("lib.rs")).unwrap();
        changed.extend_from_slice(b"\n// semantic mutation\n");
        fs::write(modified.definition(), changed).unwrap();
        let changed_source_tree = reviewed_source_tree_identity(
            &modified.source_root(),
            GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1,
        )
        .unwrap();
        let changed_definition = reviewed_provider_source_identity_from_path(
            &modified.source_root(),
            &modified.definition(),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        assert!(
            validate_reviewed_general_gemm_source_tree_v1(changed_source_tree).is_err()
                || validate_reviewed_general_gemm_definition_source_v1(
                    TrustedGeneralGemmSurfaceV1::Typestate,
                    changed_definition,
                )
                .is_err()
        );
    }

    #[test]
    fn general_gemm_semantic_source_tree_excludes_manifest_provenance() {
        let first = ProviderPackageFixture::new();
        let second = ProviderPackageFixture::new();
        fs::write(
            second.root.join("Cargo.toml"),
            b"[package]\nname='alternate-manifest'\nversion='999.0.0'\n",
        )
        .unwrap();
        assert_eq!(
            reviewed_source_tree_identity(
                &first.source_root(),
                GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1,
            )
            .unwrap(),
            reviewed_source_tree_identity(
                &second.source_root(),
                GENERAL_GEMM_PROVIDER_SOURCE_TREE_DOMAIN_V1,
            )
            .unwrap()
        );
    }

    #[test]
    fn reviewed_general_gemm_compiler_observation_is_same_session_only() {
        let exact = CompilerProviderObservationV1 {
            crate_name: "fe2o3_gemm_device_v1".into(),
            stable_crate_id: 0x1234,
            crate_hash_observation: [0x56; 16],
        };
        validate_reviewed_general_gemm_terminal_provider_v1(&exact, &exact).unwrap();

        let mut changed = exact.clone();
        changed.stable_crate_id ^= 1;
        assert!(validate_reviewed_general_gemm_terminal_provider_v1(&exact, &changed).is_err());
        changed = exact.clone();
        changed.crate_hash_observation[0] ^= 1;
        assert!(validate_reviewed_general_gemm_terminal_provider_v1(&exact, &changed).is_err());
        changed = exact.clone();
        changed.crate_name = "same_name_impostor".into();
        assert!(validate_reviewed_general_gemm_terminal_provider_v1(&exact, &changed).is_err());
    }

    #[test]
    fn reviewed_general_gemm_dependency_identity_is_portable_and_exact() {
        let source_closure_identity = reviewed_provider_source_closure_identity(
            Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT),
            WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        let source_root = Path::new(super::REVIEWED_FE2O3_DEVICE_SOURCE_ROOT);
        let definition_source_identity = reviewed_provider_source_identity_from_path(
            source_root,
            &source_root.join("lib.rs"),
            WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        let definition = semantic_definition(
            ReviewedProviderSemanticProfileV1::WorkgroupFlashMoeV4,
            "DisjointSlice",
            source_closure_identity,
            definition_source_identity,
        );
        let identity = |definition: &ReviewedProviderSemanticDefinitionV1,
                        compiled_source: [u8; 32]| {
            general_gemm_dependency_semantic_identity_v1(
                definition,
                compiled_source,
                ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                "general-gemm-disjoint-slice-dependency-v1",
            )
        };
        let exact = identity(&definition, definition_source_identity).unwrap();
        assert_eq!(
            exact,
            super::REVIEWED_GENERAL_GEMM_DISJOINT_SLICE_DEPENDENCY_V1
        );
        validate_reviewed_general_gemm_dependency_identity_v1(exact).unwrap();

        let mut portable = definition.clone();
        portable.provider.stable_crate_id = 0;
        portable.provider.crate_hash_observation = [0; 16];
        portable.cargo_metadata_build_observation = [0; 32];
        assert_eq!(
            identity(&portable, definition_source_identity).unwrap(),
            exact
        );

        portable = definition.clone();
        portable.cargo_metadata_build_observation[0] ^= 1;
        portable.provider.stable_crate_id ^= 1;
        portable.provider.crate_hash_observation[0] ^= 1;
        assert_eq!(
            identity(&portable, definition_source_identity).unwrap(),
            exact
        );

        let mut changed_definition = definition.clone();
        changed_definition.canonical_definition_path = "fe2o3_device::Index1D".into();
        changed_definition.structural_local_definition_component =
            structural_local_definition_component_v1("Index1D").unwrap();
        assert_ne!(
            identity(&changed_definition, definition_source_identity).unwrap(),
            exact
        );
        changed_definition = definition.clone();
        changed_definition.source_closure_identity[0] ^= 1;
        assert_ne!(
            identity(&changed_definition, definition_source_identity).unwrap(),
            exact
        );
        changed_definition = definition.clone();
        changed_definition.definition_source_identity[0] ^= 1;
        assert!(identity(&changed_definition, definition_source_identity).is_err());
        changed_definition = definition.clone();
        changed_definition.provider.crate_name = "substituted_device".into();
        assert!(identity(&changed_definition, definition_source_identity).is_err());
        changed_definition = definition.clone();
        changed_definition.profile = ReviewedProviderSemanticProfileV1::MatrixV3;
        assert!(identity(&changed_definition, definition_source_identity).is_err());

        let mut changed = super::REVIEWED_GENERAL_GEMM_DISJOINT_SLICE_DEPENDENCY_V1;
        changed[0] ^= 1;
        assert!(validate_reviewed_general_gemm_dependency_identity_v1(changed).is_err());
    }

    #[test]
    fn compiled_source_hash_rejects_stale_rlib_after_source_restore() {
        let stale_compiled = SourceFileHash::new_in_memory(
            SourceFileHashAlgorithm::Sha256,
            "modified provider source",
        );
        validate_compiled_provider_source_hash_v1(
            &stale_compiled,
            "modified provider source",
            Path::new("provider.rs"),
        )
        .unwrap();
        assert!(
            validate_compiled_provider_source_hash_v1(
                &stale_compiled,
                "reviewed provider source restored on disk",
                Path::new("provider.rs"),
            )
            .is_err()
        );
    }

    #[test]
    fn row_and_matrix_profiles_bind_distinct_source_domains() {
        let fixture = ProviderPackageFixture::new();
        let workgroup_closure = reviewed_provider_source_closure_identity(
            &fixture.root,
            WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        let row_closure = reviewed_provider_source_closure_identity(
            &fixture.root,
            ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
        )
        .unwrap();
        let matrix_closure = reviewed_provider_source_closure_identity(
            &fixture.root,
            MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3,
        )
        .unwrap();
        assert_ne!(workgroup_closure, row_closure);
        assert_ne!(workgroup_closure, matrix_closure);
        assert_ne!(row_closure, matrix_closure);

        let workgroup_source = reviewed_provider_source_identity_from_path(
            &fixture.source_root(),
            &fixture.definition(),
            WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        let row_source = reviewed_provider_source_identity_from_path(
            &fixture.source_root(),
            &fixture.definition(),
            ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        let matrix_source = reviewed_provider_source_identity_from_path(
            &fixture.source_root(),
            &fixture.definition(),
            MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2,
        )
        .unwrap();
        assert_ne!(workgroup_source, row_source);
        assert_ne!(workgroup_source, matrix_source);
        assert_ne!(row_source, matrix_source);

        let row = semantic_definition(
            ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
            "thread::thread_idx_x",
            row_closure,
            row_source,
        );
        let matrix = semantic_definition(
            ReviewedProviderSemanticProfileV1::MatrixV3,
            "thread::thread_idx_x",
            matrix_closure,
            matrix_source,
        );
        let row_identity = row
            .durable_semantic_identity_for_profile(
                ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
                ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                "provider-thread-index",
            )
            .unwrap();
        let matrix_identity = matrix
            .durable_semantic_identity_for_profile(
                ReviewedProviderSemanticProfileV1::MatrixV3,
                ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                "provider-thread-index",
            )
            .unwrap();
        assert_ne!(row_identity, matrix_identity);
        assert!(
            row.durable_semantic_identity_for_profile(
                ReviewedProviderSemanticProfileV1::MatrixV3,
                ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                "provider-thread-index",
            )
            .is_err()
        );
        assert!(
            matrix
                .durable_semantic_identity_for_profile(
                    ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
                    ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                    "provider-thread-index",
                )
                .is_err()
        );
    }

    #[test]
    fn complete_source_closure_binds_manifest_build_script_and_nested_sources() {
        let fixture = ProviderPackageFixture::new();
        let identity = || {
            reviewed_provider_source_closure_identity(
                &fixture.root,
                ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
            )
            .unwrap()
        };
        let initial = identity();

        fs::write(
            fixture.root.join("Cargo.toml"),
            b"[package]\nname='changed'\n",
        )
        .unwrap();
        let changed_manifest = identity();
        assert_ne!(changed_manifest, initial);

        fs::write(fixture.root.join("build.rs"), b"fn main() {}\n").unwrap();
        let with_build_script = identity();
        assert_ne!(with_build_script, changed_manifest);

        fs::write(
            fixture.root.join("src/nested/mod.rs"),
            b"pub fn changed() {}\n",
        )
        .unwrap();
        let changed_nested_source = identity();
        assert_ne!(changed_nested_source, with_build_script);

        fs::write(
            fixture.root.join("src/extra.rs"),
            b"pub const EXTRA: u8 = 1;\n",
        )
        .unwrap();
        assert_ne!(identity(), changed_nested_source);
    }

    #[test]
    fn ordered_definition_validation_rejects_substitution_and_exposes_role_order() {
        let first = semantic_definition(
            ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
            "thread::thread_idx_x",
            [5; 32],
            [6; 32],
        );
        let second = semantic_definition(
            ReviewedProviderSemanticProfileV1::RowSoftmaxV2,
            "thread::block_idx_x",
            [5; 32],
            [7; 32],
        );
        let expectations = [
            ProviderSemanticDefinitionExpectationV1 {
                definition_role: ProviderSemanticDefinitionRoleV1::TrustedDefinition,
                canonical_role: "thread-index",
                canonical_definition_path: "fe2o3_device::thread::thread_idx_x",
            },
            ProviderSemanticDefinitionExpectationV1 {
                definition_role: ProviderSemanticDefinitionRoleV1::SemanticTerminal,
                canonical_role: "block-index",
                canonical_definition_path: "fe2o3_device::thread::block_idx_x",
            },
        ];
        let (provider, identities) = validate_ordered_provider_semantic_definitions_v1(
            &[first.clone(), second.clone()],
            &expectations,
        )
        .unwrap();
        assert_eq!(provider, first.provider);
        assert_eq!(identities.len(), 2);

        assert!(
            validate_ordered_provider_semantic_definitions_v1(
                &[second.clone(), first.clone()],
                &expectations,
            )
            .is_err()
        );
        assert!(
            validate_ordered_provider_semantic_definitions_v1(
                &[first.clone(), first.clone()],
                &expectations,
            )
            .is_err()
        );

        let mut duplicate_role = expectations;
        duplicate_role[1].canonical_role = duplicate_role[0].canonical_role;
        assert!(
            validate_ordered_provider_semantic_definitions_v1(
                &[first.clone(), second.clone()],
                &duplicate_role,
            )
            .is_err()
        );

        let mut reordered_roles = expectations;
        reordered_roles.swap(0, 1);
        reordered_roles[0].canonical_definition_path = "fe2o3_device::thread::thread_idx_x";
        reordered_roles[1].canonical_definition_path = "fe2o3_device::thread::block_idx_x";
        let (_, reordered_identities) = validate_ordered_provider_semantic_definitions_v1(
            &[first.clone(), second.clone()],
            &reordered_roles,
        )
        .unwrap();
        assert_ne!(reordered_identities, identities);

        let mut changed_provider = second.clone();
        changed_provider.provider.stable_crate_id ^= 1;
        assert!(
            validate_ordered_provider_semantic_definitions_v1(
                &[first.clone(), changed_provider],
                &expectations,
            )
            .is_err()
        );
        let mut changed_profile = second;
        changed_profile.profile = ReviewedProviderSemanticProfileV1::MatrixV3;
        assert!(
            validate_ordered_provider_semantic_definitions_v1(
                &[first, changed_profile],
                &expectations,
            )
            .is_err()
        );
    }

    #[test]
    fn source_closure_rejects_missing_inputs_and_out_of_root_definitions() {
        let missing_manifest = ProviderPackageFixture::new();
        fs::remove_file(missing_manifest.root.join("Cargo.toml")).unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &missing_manifest.root,
                MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3,
            )
            .is_err()
        );

        let missing_source_root = ProviderPackageFixture::new();
        fs::remove_dir_all(missing_source_root.source_root()).unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &missing_source_root.root,
                MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3,
            )
            .is_err()
        );

        let reviewed = ProviderPackageFixture::new();
        let outside = ProviderPackageFixture::new();
        let reviewed_root = fs::canonicalize(&reviewed.root).unwrap();
        assert!(super::reviewed_source_file(&reviewed_root, &outside.definition()).is_err());
        assert!(
            reviewed_provider_source_identity_from_path(
                &reviewed.source_root(),
                &outside.definition(),
                MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2,
            )
            .is_err()
        );
        assert!(reviewed_provider_source_closure_identity(&reviewed.root, b"").is_err());
        assert!(
            reviewed_provider_source_identity_from_path(
                &reviewed.source_root(),
                &reviewed.definition(),
                b"",
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_closure_rejects_symlinks_and_nonregular_files() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let manifest_link = ProviderPackageFixture::new();
        let manifest = manifest_link.root.join("Cargo.toml");
        let real_manifest = manifest_link.root.join("Cargo.real.toml");
        fs::rename(&manifest, &real_manifest).unwrap();
        symlink(&real_manifest, &manifest).unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &manifest_link.root,
                ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
            )
            .is_err()
        );

        let source_link = ProviderPackageFixture::new();
        symlink(
            source_link.definition(),
            source_link.source_root().join("alias.rs"),
        )
        .unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &source_link.root,
                ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
            )
            .is_err()
        );

        let source_root_link = ProviderPackageFixture::new();
        fs::rename(
            source_root_link.source_root(),
            source_root_link.root.join("real-src"),
        )
        .unwrap();
        symlink(
            source_root_link.root.join("real-src"),
            source_root_link.source_root(),
        )
        .unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &source_root_link.root,
                ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
            )
            .is_err()
        );

        let socket = ProviderPackageFixture::new();
        let _listener = UnixListener::bind(socket.source_root().join("provider.sock")).unwrap();
        assert!(
            reviewed_provider_source_closure_identity(
                &socket.root,
                ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
            )
            .is_err()
        );
    }

    #[test]
    fn pinned_core_terminal_identity_excludes_volatile_crate_disambiguators() {
        let core = CompilerProviderObservationV1 {
            crate_name: "core".into(),
            stable_crate_id: 9,
            crate_hash_observation: [8; 16],
        };
        let identity = |provider: &CompilerProviderObservationV1, role, path| {
            pinned_core_semantic_terminal_identity_v1(provider, role, path)
        };
        let exact = identity(
            &core,
            "core::intrinsics::atomic_xadd",
            "intrinsics::atomic_xadd",
        )
        .expect("complete core terminal identity");

        let mut mutation = core.clone();
        mutation.stable_crate_id ^= 1;
        assert_eq!(
            identity(
                &mutation,
                "core::intrinsics::atomic_xadd",
                "intrinsics::atomic_xadd",
            )
            .unwrap(),
            exact
        );
        mutation = core.clone();
        mutation.crate_hash_observation[0] ^= 1;
        assert_eq!(
            identity(
                &mutation,
                "core::intrinsics::atomic_xadd",
                "intrinsics::atomic_xadd",
            )
            .unwrap(),
            exact
        );
        assert_ne!(
            identity(
                &core,
                "core::intrinsics::atomic_xsub",
                "intrinsics::atomic_xadd",
            )
            .unwrap(),
            exact
        );
        assert_ne!(
            identity(
                &core,
                "core::intrinsics::atomic_xadd",
                "intrinsics::atomic_xsub",
            )
            .unwrap(),
            exact
        );
        mutation = core;
        mutation.crate_name = "impostor_core".into();
        assert!(
            identity(
                &mutation,
                "core::intrinsics::atomic_xadd",
                "intrinsics::atomic_xadd",
            )
            .is_err()
        );
    }

    #[test]
    fn structural_definition_paths_are_canonical_and_crate_qualified() {
        assert_eq!(
            canonical_compiler_definition_path(
                "fe2o3_device",
                "::__fe2o3_kernel_device::thread_idx_x",
            )
            .unwrap(),
            "fe2o3_device::__fe2o3_kernel_device::thread_idx_x"
        );
        assert!(canonical_compiler_definition_path("", "thread_idx_x").is_err());
        assert!(canonical_compiler_definition_path("fe2o3_device", "::").is_err());
    }

    #[test]
    fn semantic_registry_is_complete_and_unique() {
        let items = [
            TrustedDeviceItem::KernelError,
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::StridedReadView2D,
            TrustedDeviceItem::StridedReadView2DError,
            TrustedDeviceItem::StridedReadView2DFromSharedSlice,
            TrustedDeviceItem::StridedReadView2DLoadOr,
            TrustedDeviceItem::DeviceGlobalMutPtr,
            TrustedDeviceItem::WorkgroupLdsScope,
            TrustedDeviceItem::WorkgroupLdsScopeCurrent,
            TrustedDeviceItem::DynamicLdsExactCurrent,
            TrustedDeviceItem::Invocation3D,
            TrustedDeviceItem::Invocation3DCurrent,
            TrustedDeviceItem::ThreadIndexX,
            TrustedDeviceItem::ThreadIndexY,
            TrustedDeviceItem::ThreadIndexZ,
            TrustedDeviceItem::WorkgroupIndexX,
            TrustedDeviceItem::WorkgroupIndexY,
            TrustedDeviceItem::WorkgroupIndexZ,
            TrustedDeviceItem::WorkgroupDimensionX,
            TrustedDeviceItem::WorkgroupDimensionY,
            TrustedDeviceItem::WorkgroupDimensionZ,
            TrustedDeviceItem::GridDimensionX,
            TrustedDeviceItem::GridDimensionY,
            TrustedDeviceItem::GridDimensionZ,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::DisjointIndex,
            TrustedDeviceItem::ShiftedIndexSpace,
            TrustedDeviceItem::BlockedIndexSpace,
            TrustedDeviceItem::Tiled2DIndexSpace,
            TrustedDeviceItem::RowStriped2DIndexSpace,
            TrustedDeviceItem::GridExclusiveIndexSpace,
            TrustedDeviceItem::GridLeader,
            TrustedDeviceItem::DisjointBlock,
            TrustedDeviceItem::DisjointTile2D,
            TrustedDeviceItem::DisjointRowStripe2D,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexIntoDisjoint,
            TrustedDeviceItem::ThreadIndexCheckedShift,
            TrustedDeviceItem::ThreadIndexCheckedBlock,
            TrustedDeviceItem::ThreadIndexCheckedTiled2D,
            TrustedDeviceItem::ThreadIndexCheckedRowStriped2D,
            TrustedDeviceItem::DisjointIndexGet,
            TrustedDeviceItem::DisjointIndexCheckedShift,
            TrustedDeviceItem::DisjointBlockComponentIndex,
            TrustedDeviceItem::GridLeaderCurrent,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetDisjointMut,
            TrustedDeviceItem::DisjointSliceGetMutExclusive,
            TrustedDeviceItem::DisjointSliceGetBlockMut,
            TrustedDeviceItem::DisjointSliceGetTiled2DMut,
            TrustedDeviceItem::DisjointSliceGetRowStriped2DMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
            TrustedDeviceItem::DisjointSliceLen,
            TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
            TrustedDeviceItem::MemoryOffsetFrom,
            TrustedDeviceItem::MemoryVolatileLoad,
            TrustedDeviceItem::MemoryVolatileStore,
            TrustedDeviceItem::MemoryCopyNonOverlapping,
            TrustedDeviceItem::MemoryCopyOneNonOverlapping,
            TrustedDeviceItem::Gfx942CollectivesContext,
            TrustedDeviceItem::Gfx942CollectivesCurrent,
            TrustedDeviceItem::Gfx942SubgroupReduceSumF32,
            TrustedDeviceItem::Gfx942SubgroupReduceMaxF32,
            TrustedDeviceItem::Gfx942StaticLdsU32x256,
            TrustedDeviceItem::Gfx942StaticLdsU32x256Type,
            TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
            TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            TrustedDeviceItem::Gfx942Wave64InclusiveScanSum,
            TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum,
            TrustedDeviceItem::Gfx942WorkgroupReduceSum,
            TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum,
            TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum,
            TrustedDeviceItem::Gfx942BarrierArrive,
            TrustedDeviceItem::Gfx942BarrierWait,
            TrustedDeviceItem::WaveLane,
            TrustedDeviceItem::Wave64,
            TrustedDeviceItem::WaveLaneCurrent,
            TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
            TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16,
            TrustedDeviceItem::LdsTile16x16WriteMfmaBf16,
            TrustedDeviceItem::LdsTile16x16ReadMfmaBf16,
            TrustedDeviceItem::WorkgroupSyncthreads,
            TrustedDeviceItem::DeviceMatrix,
            TrustedDeviceItem::DeviceMatrixCurrent,
            TrustedDeviceItem::Bf16MfmaProfile,
            TrustedDeviceItem::MfmaOperandA,
            TrustedDeviceItem::MfmaOperandB,
            TrustedDeviceItem::MfmaRegisterTile16x16,
            TrustedDeviceItem::MfmaLdsXor4Storage,
            TrustedDeviceItem::MfmaAccumulatorRowMajor,
            TrustedDeviceItem::Bf16MfmaFragment,
            TrustedDeviceItem::F32AccumulatorFragment,
            TrustedDeviceItem::F32AccumulatorFragmentZero,
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
            TrustedDeviceItem::Bf16MfmaMatrixView,
            TrustedDeviceItem::Bf16MfmaMatrixViewError,
            TrustedDeviceItem::Bf16MfmaMatrixARowMajor,
            TrustedDeviceItem::Bf16MfmaMatrixBRowMajor,
            TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2,
            TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2,
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Acquire,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Stage,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Publish,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Mfma,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Reuse,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::Typestate,
                TrustedGeneralGemmOperationV1::Store,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Acquire,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Lane,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::WorkgroupX,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::WorkgroupY,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::LoadA,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::LoadB,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::LoadC,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Stage,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::StageValue,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::WaitStage,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::ReadStage,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Publish,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Mfma,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::MfmaValue,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Reuse,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Store,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::StoreEpilogue,
            ),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VMovB32),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAddU32),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VSubU32),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAndB32),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VOrB32),
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VXorB32),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print0),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print1),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print2),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::AssertFail),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Clock32),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::DebugTrap),
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::ProfilingMarker),
        ];

        let paths = items.map(TrustedDeviceItem::canonical_path);
        assert_eq!(paths.len(), super::TRUSTED_ITEMS.len());
        for (index, path) in paths.iter().enumerate() {
            assert!(!path.is_empty());
            assert!(!paths[..index].contains(path));
        }

        let markers = super::TRUSTED_ITEMS
            .iter()
            .map(|(_, marker, _)| *marker)
            .collect::<Vec<_>>();
        for (index, marker) in markers.iter().enumerate() {
            assert!(!marker.is_empty());
            assert!(!markers[..index].contains(marker));
        }

        let half_math_items = [
            TrustedDeviceItem::DeviceValue(DeviceValueDiagnosticItem::F16),
            TrustedDeviceItem::DeviceValue(DeviceValueDiagnosticItem::Bf16),
            TrustedDeviceItem::DeviceValue(DeviceValueDiagnosticItem::Bf16x2),
            TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::Context),
            TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::ContextFromCompiler),
        ];
        for item in half_math_items {
            assert!(!item.canonical_path().is_empty());
        }

        let markers = HALF_MATH_DIAGNOSTIC_ITEMS
            .iter()
            .map(|(marker, _)| *marker)
            .collect::<Vec<_>>();
        let paths = HALF_MATH_DIAGNOSTIC_ITEMS
            .iter()
            .map(|(_, path)| *path)
            .collect::<Vec<_>>();
        for index in 0..markers.len() {
            assert!(!markers[..index].contains(&markers[index]));
            assert!(!paths[..index].contains(&paths[index]));
        }
    }

    #[test]
    fn safe_execution_items_have_exact_structural_provider_paths() {
        let items = [
            TrustedDeviceItem::WorkgroupLdsScope,
            TrustedDeviceItem::WorkgroupLdsScopeCurrent,
            TrustedDeviceItem::DynamicLdsExactCurrent,
            TrustedDeviceItem::Invocation3D,
            TrustedDeviceItem::Invocation3DCurrent,
            TrustedDeviceItem::Gfx942CollectivesContext,
            TrustedDeviceItem::Gfx942CollectivesCurrent,
            TrustedDeviceItem::Gfx942SubgroupReduceSumF32,
            TrustedDeviceItem::Gfx942SubgroupReduceMaxF32,
            TrustedDeviceItem::Gfx942StaticLdsU32x256,
            TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
            TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            TrustedDeviceItem::Gfx942WorkgroupReduceSum,
            TrustedDeviceItem::WaveLaneCurrent,
            TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
            TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16,
            TrustedDeviceItem::LdsTile16x16WriteMfmaBf16,
            TrustedDeviceItem::LdsTile16x16ReadMfmaBf16,
            TrustedDeviceItem::WorkgroupSyncthreads,
            TrustedDeviceItem::DeviceMatrix,
            TrustedDeviceItem::DeviceMatrixCurrent,
            TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2,
            TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2,
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
        ];
        let mut paths = BTreeSet::new();
        for item in items {
            assert!(safe_execution_provider_bound_item(item));
            let path = safe_execution_compiler_definition_path(item);
            assert!(path.starts_with("fe2o3_device::"));
            assert!(
                path.contains("::collective::")
                    || path.contains("::lds::")
                    || path.contains("::sync::")
                    || path.contains("::tensor::")
                    || path.contains("::thread::")
                    || path.contains("::wave::"),
                "safe execution item retained only a public re-export path: {path}"
            );
            assert!(paths.insert(path), "duplicate provider DefPath: {path}");
        }
    }
}
