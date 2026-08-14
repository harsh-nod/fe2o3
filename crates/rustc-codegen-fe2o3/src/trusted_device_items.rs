//! Semantic identities recognized by device lowering.
//!
//! Recognition starts from a rustc [`DefId`]. Diagnostic-item equality is only
//! accepted after the provider definition is anchored to the reviewed sibling
//! `fe2o3-device` source tree used to build this backend. The imported matrix
//! and row-softmax records bind that source identity, rustc's observed stable
//! crate ID and full crate hash, and the managed Cargo metadata observation.
//!
//! This remains a compiler build-observation boundary, not cryptographic
//! package authentication. A publisher signature or transparency-log identity
//! must be checked before the managed build when that stronger claim is needed.

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
const REVIEWED_FE2O3_DEVICE_SOURCE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../fe2o3-device/src");

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
pub(crate) struct RejectedTrustedProvider {
    pub(crate) marker: &'static str,
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
pub(crate) enum TrustedDeviceItem {
    DisjointSlice,
    ThreadIndex,
    ThreadIndex1d,
    ThreadIndexGet,
    ThreadIndexOffset,
    ThreadIndexOffsetSigned,
    ThreadIndexStride,
    ThreadIndexStrideOffset,
    DisjointSliceGetMut,
    DisjointSliceGetMutAt,
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
    DeviceMatrix,
    DeviceMatrixFromCompiler,
    Bf16MfmaFragment,
    Bf16MfmaFragmentFromBits,
    F32AccumulatorFragment,
    F32AccumulatorFragmentFromValues,
    F32AccumulatorFragmentIntoValues,
    DeviceMatrixMultiplyAccumulate,
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
        .map(|reason| RejectedTrustedProvider { marker, reason })
}

fn provider_rule(tcx: TyCtxt<'_>, def_id: DefId, item: TrustedDeviceItem) -> Result<(), String> {
    if matrix_provider_bound_item(item) {
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
    if crate_num == LOCAL_CRATE {
        return Err("provider is the local compilation crate".to_owned());
    }
    let crate_name = tcx.crate_name(crate_num).as_str().to_owned();
    if crate_name != "fe2o3_device" {
        return Err(format!("provider crate name is `{crate_name}`"));
    }
    Ok(crate_name)
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

fn reviewed_matrix_source_identity(tcx: TyCtxt<'_>, def_id: DefId) -> Result<[u8; 32], String> {
    reviewed_provider_source_identity(tcx, def_id, MATRIX_PROVIDER_SOURCE_IDENTITY_DOMAIN_V2)
}

fn reviewed_provider_source_identity(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    domain: &[u8],
) -> Result<[u8; 32], String> {
    let file_name = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(def_id))
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let source = std::fs::canonicalize(&file_name).map_err(|error| {
        format!("provider source file `{file_name}` is unavailable to the managed build: {error}")
    })?;
    let reviewed_root =
        std::fs::canonicalize(REVIEWED_FE2O3_DEVICE_SOURCE_ROOT).map_err(|error| {
            format!(
                "reviewed fe2o3-device source root is unavailable to the managed build: {error}"
            )
        })?;
    let relative = source.strip_prefix(&reviewed_root).map_err(|_| {
        format!(
            "provider source file `{}` is outside the reviewed fe2o3-device source root",
            source.display()
        )
    })?;
    let source_bytes = std::fs::read(&source).map_err(|error| {
        format!(
            "provider source file `{}` cannot be observed by the managed build: {error}",
            source.display()
        )
    })?;
    let relative = relative.to_string_lossy();
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
    use super::{
        HALF_MATH_DIAGNOSTIC_ITEMS, TrustedAmdGpuDiagnosticOperation, TrustedAmdGpuInlineOperation,
        TrustedDeviceItem,
    };
    use dialect_amdgcn::{DeviceMathDiagnosticItem, DeviceValueDiagnosticItem};

    #[test]
    fn semantic_registry_is_complete_and_unique() {
        let items = [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
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
            TrustedDeviceItem::DeviceMatrix,
            TrustedDeviceItem::DeviceMatrixFromCompiler,
            TrustedDeviceItem::Bf16MfmaFragment,
            TrustedDeviceItem::F32AccumulatorFragment,
            TrustedDeviceItem::Bf16MfmaFragmentFromBits,
            TrustedDeviceItem::F32AccumulatorFragmentFromValues,
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
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
