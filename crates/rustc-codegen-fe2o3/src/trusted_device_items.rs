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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::lang_items::LangItem;
use rustc_middle::ty::{TyCtxt, TyKind};
use rustc_span::Symbol;
use sha2::{Digest as _, Sha256};

use dialect_amdgcn::{
    DeviceMathDiagnosticItem, DeviceValueDiagnosticItem, Fe2o3DeviceDiagnosticItem,
};
use fe2o3_kernel_ir::{F32MathFunction, NarrowFloatFormat, WidenedFloatBinaryOp};

const CARGO_METADATA_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2";

const MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/MATRIX-PROVIDER-SOURCE-IDENTITY/V2\0";
const ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX-PROVIDER-SOURCE-IDENTITY/V1\0";
const WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKGROUP-SYNC-PROVIDER-SOURCE-IDENTITY/V1\0";
const WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKGROUP-SYNC-PROVIDER-SOURCE-CLOSURE/V1\0";
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
const GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM-PROVIDER-SOURCE-CLOSURE/V1\0";
// Updated only after review of the complete standalone companion package.
const REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_V1: [u8; 32] = [
    0x70, 0x88, 0x14, 0x5d, 0x4b, 0xdc, 0xf9, 0x7b, 0x6e, 0x44, 0x3c, 0xfe, 0xbe, 0x8e, 0x2e, 0x37,
    0xea, 0xff, 0xd3, 0xc4, 0xff, 0x97, 0xa6, 0x72, 0x9f, 0x4d, 0xb5, 0x49, 0x49, 0x87, 0x26, 0x32,
];
// All V1 terminals are defined in the companion's exact `src/lib.rs`.
const REVIEWED_GENERAL_GEMM_PROVIDER_DEFINITION_SOURCE_V1: [u8; 32] = [
    0xa3, 0x53, 0x50, 0x48, 0x6e, 0x60, 0x21, 0xef, 0xf9, 0x74, 0x62, 0x8d, 0x4b, 0xa4, 0xa5, 0x16,
    0x11, 0x73, 0xe8, 0x31, 0xb4, 0xe6, 0x85, 0xc1, 0x86, 0x4a, 0xce, 0x12, 0xf8, 0x63, 0x2a, 0xd1,
];
const PROVIDER_SEMANTIC_DEFINITION_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/PROVIDER-SEMANTIC-DEFINITION-TRANSCRIPT/V1\0";
const PINNED_CORE_SEMANTIC_TERMINAL_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/PINNED-CORE-SEMANTIC-TERMINAL-TRANSCRIPT/V1\0";
const STRUCTURAL_LOCAL_DEFINITION_COMPONENT_DOMAIN_V1: &[u8] =
    b"FE2O3/STRUCTURAL-LOCAL-DEFINITION-COMPONENT/V1\0";
const REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../fe2o3-device");
const REVIEWED_FE2O3_DEVICE_SOURCE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../fe2o3-device/src");
const REVIEWED_GENERAL_GEMM_PROVIDER_PACKAGE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/tiled_gemm_general_v1/device-api"
);
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
static GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_V1: OnceLock<Result<[u8; 32], String>> =
    OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewedMatrixProviderObservationV2 {
    pub(crate) crate_name: String,
    pub(crate) stable_crate_id: u64,
    pub(crate) crate_hash: [u8; 16],
    pub(crate) cargo_metadata_build_observation: [u8; 32],
    pub(crate) source_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    Stage,
    Publish,
    Mfma,
    Reuse,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedDeviceItem {
    DisjointSlice,
    DeviceGlobalMutPtr,
    WorkgroupLdsScope,
    DynamicLdsExactFromCompiler,
    ThreadIndex,
    ThreadIndex1d,
    ThreadIndexGet,
    ThreadIndexOffset,
    ThreadIndexOffsetSigned,
    ThreadIndexStride,
    ThreadIndexStrideOffset,
    DisjointSliceGetMut,
    DisjointSliceGetMutAt,
    DisjointSliceLen,
    MemoryOffsetFrom,
    MemoryVolatileLoad,
    MemoryVolatileStore,
    MemoryCopyNonOverlapping,
    Gfx942CollectivesContext,
    Gfx942CollectivesFromCompiler,
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
    WaveLaneFromRaw,
    Gfx942LdsBf16TilePairM16x16,
    LdsTile16x16AssumeInit,
    LdsTile16x16WriteMfmaBf16,
    LdsTile16x16ReadMfmaBf16,
    WorkgroupSyncthreads,
    DeviceMatrix,
    DeviceMatrixFromCompiler,
    Bf16MfmaFragment,
    Bf16MfmaFragmentFromBits,
    F32AccumulatorFragment,
    F32AccumulatorFragmentFromValues,
    F32AccumulatorFragmentIntoValues,
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
        TrustedDeviceItem::DisjointSlice,
        "fe2o3_device_disjoint_slice",
        "fe2o3_device::DisjointSlice",
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
        TrustedDeviceItem::DynamicLdsExactFromCompiler,
        "fe2o3_device_dynamic_lds_exact_from_compiler_v1",
        "fe2o3_device::DynamicLds::<T>::exact_from_compiler",
    ),
    (
        TrustedDeviceItem::ThreadIndex,
        "fe2o3_device_thread_index",
        "fe2o3_device::ThreadIndex",
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
        "fe2o3_device::memory::copy_nonoverlapping",
    ),
    (
        TrustedDeviceItem::Gfx942CollectivesContext,
        "fe2o3_device_gfx942_collectives_context_v1",
        "fe2o3_device::Gfx942Collectives",
    ),
    (
        TrustedDeviceItem::Gfx942CollectivesFromCompiler,
        "fe2o3_device_gfx942_collectives_from_compiler_v1",
        "fe2o3_device::Gfx942Collectives::from_compiler",
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
        TrustedDeviceItem::WaveLaneFromRaw,
        "fe2o3_device_wave_lane_from_raw",
        "fe2o3_device::WaveLane::from_raw",
    ),
    (
        TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
        "fe2o3_device_gfx942_lds_bf16_tile_pair_m16x16_v1",
        "fe2o3_device::gfx942_lds_bf16_tile_pair_m16x16_v1",
    ),
    (
        TrustedDeviceItem::LdsTile16x16AssumeInit,
        "fe2o3_device_lds_tile16x16_assume_init_v1",
        "fe2o3_device::LdsTile16x16::assume_init",
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
        TrustedDeviceItem::DeviceMatrixFromCompiler,
        "fe2o3_device_matrix_context_from_compiler_v1",
        "fe2o3_device::DeviceMatrix::from_compiler",
    ),
    (
        TrustedDeviceItem::Bf16MfmaFragment,
        "fe2o3_device_bf16_mfma_fragment_v1",
        "fe2o3_device::Bf16MfmaFragment",
    ),
    (
        TrustedDeviceItem::Bf16MfmaFragmentFromBits,
        "fe2o3_device_bf16_mfma_fragment_from_bits_v1",
        "fe2o3_device::Bf16MfmaFragment::from_bits",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragment,
        "fe2o3_device_f32_accumulator_fragment_v1",
        "fe2o3_device::F32AccumulatorFragment",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragmentFromValues,
        "fe2o3_device_f32_accumulator_fragment_from_values_v1",
        "fe2o3_device::F32AccumulatorFragment::from_values",
    ),
    (
        TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
        "fe2o3_device_f32_accumulator_fragment_into_values_v1",
        "fe2o3_device::F32AccumulatorFragment::into_values",
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
            TrustedGeneralGemmOperationV1::Stage,
        ),
        "fe2o3_device_general_tiled_gemm_proof_stage_v1",
        "fe2o3_gemm_device_v1::proof_stage_gfx942_tiled_gemm_wave64_v1",
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
        "fe2o3_device::DeviceMath::from_compiler",
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
    } else if matrix_provider_bound_item(item) {
        reviewed_matrix_provider_observation(tcx, def_id).map(|_| ())
    } else if row_softmax_provider_bound_item(item) {
        reviewed_row_softmax_provider_definition(tcx, def_id).map(|_| ())
    } else {
        named_external_provider(tcx, def_id.krate).map(|_| ())
    }
}

const fn row_softmax_provider_bound_item(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::DisjointSlice
            | TrustedDeviceItem::ThreadIndex
            | TrustedDeviceItem::ThreadIndex1d
            | TrustedDeviceItem::ThreadIndexGet
            | TrustedDeviceItem::DisjointSliceGetMutAt
            | TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::Context)
            | TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::ContextFromCompiler)
            | TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::F32(F32MathFunction::Exp))
    )
}

const fn matrix_provider_bound_item(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::DeviceMatrix
            | TrustedDeviceItem::DeviceMatrixFromCompiler
            | TrustedDeviceItem::Bf16MfmaFragment
            | TrustedDeviceItem::Bf16MfmaFragmentFromBits
            | TrustedDeviceItem::F32AccumulatorFragment
            | TrustedDeviceItem::F32AccumulatorFragmentFromValues
            | TrustedDeviceItem::F32AccumulatorFragmentIntoValues
            | TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
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
    let source_identity = reviewed_provider_source_identity_at_root(
        tcx,
        provider_definition,
        Path::new(REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT),
        GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
    )?;
    let source_closure = GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_V1
        .get_or_init(|| {
            reviewed_provider_source_closure_identity(
                Path::new(REVIEWED_GENERAL_GEMM_PROVIDER_PACKAGE_ROOT),
                GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
            )
        })
        .clone()?;
    let cargo_metadata = decode_sha256_environment(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)?;
    let provider = compiler_provider_observation_v1(tcx, provider_definition.krate);
    if provider.crate_name != crate_name
        || provider.stable_crate_id == 0
        || provider.crate_hash_observation == [0; 16]
        || source_identity == [0; 32]
        || source_closure == [0; 32]
        || cargo_metadata == [0; 32]
    {
        return Err("reviewed general-GEMM provider observation is incomplete".to_owned());
    }
    validate_reviewed_general_gemm_source_v1(source_closure, source_identity)?;
    Ok(())
}

fn validate_reviewed_general_gemm_source_v1(
    source_closure: [u8; 32],
    definition_source: [u8; 32],
) -> Result<(), String> {
    if source_closure != REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_V1 {
        return Err(format!(
            "general-GEMM provider source closure does not match the reviewed V1 identity: {source_closure:02x?}"
        ));
    }
    if definition_source != REVIEWED_GENERAL_GEMM_PROVIDER_DEFINITION_SOURCE_V1 {
        return Err(format!(
            "general-GEMM provider definition source does not match the reviewed V1 identity: {definition_source:02x?}"
        ));
    }
    Ok(())
}

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
    let definition_source_identity =
        reviewed_provider_source_identity(tcx, provider_definition, definition_source_domain)?;
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

fn reviewed_matrix_source_identity(tcx: TyCtxt<'_>, def_id: DefId) -> Result<[u8; 32], String> {
    reviewed_provider_source_identity(tcx, def_id, MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2)
}

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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        CompilerProviderObservationV1, GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1, HALF_MATH_DIAGNOSTIC_ITEMS,
        MATRIX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V3, MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2,
        ProviderSemanticDefinitionExpectationV1, ProviderSemanticDefinitionRoleV1,
        ROW_SOFTMAX_PROVIDER_SOURCE_CLOSURE_DOMAIN_V2,
        ROW_SOFTMAX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1, ReviewedProviderSemanticDefinitionV1,
        ReviewedProviderSemanticProfileV1, TrustedAmdGpuDiagnosticOperation,
        TrustedAmdGpuInlineOperation, TrustedDeviceItem, TrustedGeneralGemmOperationV1,
        TrustedGeneralGemmSurfaceV1, WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1, canonical_compiler_definition_path,
        pinned_core_semantic_terminal_identity_v1, reviewed_provider_source_closure_identity,
        reviewed_provider_source_identity_from_path, structural_local_definition_component_v1,
        validate_ordered_provider_semantic_definitions_v1,
        validate_reviewed_general_gemm_source_v1,
    };
    use dialect_amdgcn::{DeviceMathDiagnosticItem, DeviceValueDiagnosticItem};

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
    fn existing_flash_moe_and_workgroup_profile_bytes_match_the_parent() {
        let closure = reviewed_provider_source_closure_identity(
            Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT),
            WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        assert_eq!(
            closure,
            digest("2b9c60625eb166fc28b949bf64c06eeafc393172bd46adaefc5daa71934dc3e7")
        );

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
    fn reviewed_general_gemm_companion_source_is_exactly_pinned() {
        let package_root = Path::new(super::REVIEWED_GENERAL_GEMM_PROVIDER_PACKAGE_ROOT);
        let source_root = Path::new(super::REVIEWED_GENERAL_GEMM_PROVIDER_SOURCE_ROOT);
        let closure = reviewed_provider_source_closure_identity(
            package_root,
            GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        let definition = reviewed_provider_source_identity_from_path(
            source_root,
            &source_root.join("lib.rs"),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        validate_reviewed_general_gemm_source_v1(closure, definition).unwrap();

        let modified = ProviderPackageFixture::new();
        fs::remove_dir_all(modified.source_root()).unwrap();
        fs::create_dir_all(modified.source_root()).unwrap();
        fs::copy(
            package_root.join("Cargo.toml"),
            modified.root.join("Cargo.toml"),
        )
        .unwrap();
        let mut changed = fs::read(source_root.join("lib.rs")).unwrap();
        changed.extend_from_slice(b"\n// semantic mutation\n");
        fs::write(modified.definition(), changed).unwrap();
        let changed_closure = reviewed_provider_source_closure_identity(
            &modified.root,
            GENERAL_GEMM_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        )
        .unwrap();
        let changed_definition = reviewed_provider_source_identity_from_path(
            &modified.source_root(),
            &modified.definition(),
            GENERAL_GEMM_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        )
        .unwrap();
        assert!(
            validate_reviewed_general_gemm_source_v1(changed_closure, changed_definition).is_err()
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
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::DeviceGlobalMutPtr,
            TrustedDeviceItem::WorkgroupLdsScope,
            TrustedDeviceItem::DynamicLdsExactFromCompiler,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
            TrustedDeviceItem::DisjointSliceLen,
            TrustedDeviceItem::MemoryOffsetFrom,
            TrustedDeviceItem::MemoryVolatileLoad,
            TrustedDeviceItem::MemoryVolatileStore,
            TrustedDeviceItem::MemoryCopyNonOverlapping,
            TrustedDeviceItem::Gfx942CollectivesContext,
            TrustedDeviceItem::Gfx942CollectivesFromCompiler,
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
            TrustedDeviceItem::WaveLaneFromRaw,
            TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
            TrustedDeviceItem::LdsTile16x16AssumeInit,
            TrustedDeviceItem::LdsTile16x16WriteMfmaBf16,
            TrustedDeviceItem::LdsTile16x16ReadMfmaBf16,
            TrustedDeviceItem::WorkgroupSyncthreads,
            TrustedDeviceItem::DeviceMatrix,
            TrustedDeviceItem::DeviceMatrixFromCompiler,
            TrustedDeviceItem::Bf16MfmaFragment,
            TrustedDeviceItem::F32AccumulatorFragment,
            TrustedDeviceItem::Bf16MfmaFragmentFromBits,
            TrustedDeviceItem::F32AccumulatorFragmentFromValues,
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
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
                TrustedGeneralGemmOperationV1::Stage,
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
                TrustedGeneralGemmOperationV1::Reuse,
            ),
            TrustedDeviceItem::GeneralGemm(
                TrustedGeneralGemmSurfaceV1::ProofSensitive,
                TrustedGeneralGemmOperationV1::Store,
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
}
