//! Workload-neutral disposition of reviewed device semantic terminals.

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

use dialect_amdgcn::DeviceMathDiagnosticItem;
use fe2o3_kernel_ir::{F32MathFunction, NarrowFloatFormat};
use fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1;

use crate::trusted_device_items::{
    self, TrustedAmdGpuDiagnosticOperation, TrustedAmdGpuInlineOperation, TrustedDeviceItem,
    TrustedHalfOperation,
};

#[cfg(test)]
#[path = "production_semantic_terminal_v1/gfx942_inline_v30_tests.rs"]
mod gfx942_inline_v30_tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionBf16ConversionV1 {
    FromBits,
    ToBits,
    FromF32RoundTiesEven,
    ToF32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionTerminalExpansionV1 {
    Gfx942InlineU32(TrustedAmdGpuInlineOperation),
    Gfx942OrderedXorAddE32,
    Gfx942OrderedProgramE32,
    Gfx942CompleteBodyE32,
    Gfx942PhysicalEntryBegin,
    Gfx942PhysicalEntryLabel,
    Gfx942PhysicalEntryStep,
    Gfx942PhysicalGlobalCopyBegin,
    Gfx942PhysicalLdsExchangeBegin,
    Gfx942PhysicalGlobalCopyLabel,
    Gfx942PhysicalLdsExchangeLabel,
    Gfx942PhysicalGlobalCopyStep,
    Gfx942PhysicalLdsExchangeStep,
    ContextIssue,
    WorkgroupDerive,
    MaskedTileLoadU32,
    MaskedTileIntoFragmentU32,
    LaneFragmentIntoPartsU32,
    ThreadIndex(SemanticAxisV1),
    WorkgroupIndex(SemanticAxisV1),
    WorkgroupDimension(SemanticAxisV1),
    GridDimension(SemanticAxisV1),
    ThreadIndex1d,
    ThreadIndexGet,
    ThreadIndexIntoDisjoint,
    ThreadIndexCheckedShift,
    ThreadIndexCheckedBlock,
    ThreadIndexCheckedTiled2d,
    ThreadIndexCheckedRowStriped2d,
    DisjointIndexGet,
    DisjointBlockComponentIndex,
    DisjointIndexCheckedShift,
    DisjointSliceLen,
    DisjointSliceGetMut,
    DisjointSliceGetDisjointMut,
    GridLeaderCurrent,
    DisjointSliceGetMutExclusive,
    DisjointSliceGetBlockMut,
    DisjointSliceGetTiled2dMut,
    DisjointSliceGetRowStriped2dMut,
    WriteOnlyDisjointSliceLen,
    WriteOnlyDisjointSliceWrite,
    WriteOnlyDisjointSliceWriteDisjoint,
    WriteOnlyDisjointSliceWriteExclusive,
    WriteOnlyDisjointSliceWriteBlock,
    WriteOnlyDisjointSliceWriteTiled2d,
    WriteOnlyDisjointSliceWriteRowStriped2d,
    StridedReadView2DFromSharedSlice,
    StridedReadView2DLoadOr,
    WorkgroupLdsScopeCurrent,
    DynamicLdsExactCurrent,
    DynamicLdsIntoCollectiveRawParts,
    WorkgroupPipelineCurrent,
    WorkgroupPipelineStage,
    WorkgroupPipelineWrite,
    WorkgroupPipelineCommit,
    WorkgroupPipelineWait,
    WorkgroupPipelineConsume,
    WorkgroupPipelineRead,
    WorkgroupPipelineDiscard,
    WorkgroupPipelineRelease,
    WorkgroupBarrier,
    MathContextCurrent,
    MathF32(F32MathFunction),
    /// The exact rustc `core::intrinsics::fabs::<f32>` compiler intrinsic.
    RustcFabsF32,
    RustcSaturatingInteger(fe2o3_mir_model::semantic_mir_v1::SemanticSaturatingIntegerOpV1),
    Gfx942Wave64Shuffle(crate::trusted_device_items::Wave64ShuffleScalarV1),
    /// The exact checked `fe2o3_device::memory::volatile_load` provider.
    MemoryVolatileLoad,
    Bf16Conversion(ProductionBf16ConversionV1),
    WorkgroupCollectiveContextCurrent,
    NeutralWorkgroupReduceSum,
    NeutralWorkgroupInclusiveScanSum,
    NeutralWorkgroupExclusiveScanSum,
    CollectiveContextCurrent,
    WorkgroupReduceSum,
    SubgroupReduceSumF32,
    SubgroupReduceMaxF32,
    WaveLaneCurrent,
    MatrixContextCurrent,
    Bf16MatrixARowMajor,
    Bf16MatrixBRowMajor,
    Bf16MatrixBColumnMajor,
    Bf16MatrixALoadZeroFilledV2,
    Bf16MatrixBLoadZeroFilledV2,
    Bf16MatrixBColumnMajorLoadZeroFilledV1,
    F32MatrixAccumulatorZero,
    F32MatrixAccumulatorIntoValues,
    MatrixMultiplyAccumulate,
    Gfx950MatrixContextCurrent,
    Gfx950Fp4MatrixARowMajor,
    Gfx950Fp4MatrixBRowMajor,
    Gfx950Fp4MatrixALoadM16K128,
    Gfx950Fp4MatrixBLoadK128N16,
    Gfx950Fp4AccumulatorZero,
    Gfx950Fp4AccumulatorIntoValues,
    Gfx950Fp4MultiplyAccumulate,
    Gfx950Fp4Fp8MultiplyAccumulate,
    Gfx950Fp8MatrixARowMajor,
    Gfx950Fp8MatrixBRowMajor,
    Gfx950Fp8MatrixALoadM16K128,
    Gfx950Fp8MatrixBLoadK128N16,
    Gfx950Fp8AccumulatorZero,
    Gfx950Fp8AccumulatorIntoValues,
    Gfx950Fp8MultiplyAccumulate,
    Gfx950SubgroupCurrent,
    Gfx950SubgroupReduceMaxF32,
    Gfx950SubgroupReduceSumF32,
    Gfx950SubgroupBroadcastF32,
    Gfx950LdsTransposeTileCurrent,
    Gfx950LdsTransposeStageB4,
    Gfx950LdsTransposeStageB8,
    Gfx950LdsTransposePublish,
    Gfx950LdsTransposeReadB4,
    Gfx950LdsTransposeReadB8,
    /// Terminates the current lane by executing the target's canonical trap instruction.
    Trap,
    /// Rust's effect-free hint that the current path is unlikely to execute.
    ColdPath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSemanticTerminalRuleV1 {
    Expand(ProductionTerminalExpansionV1),
    Reject(TrustedDeviceItem),
}

pub(crate) const fn is_reserved_capability_type_v1(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::KernelContext
            | TrustedDeviceItem::ExecutionWorkgroupCapability
            | TrustedDeviceItem::MaskedTile1D
            | TrustedDeviceItem::LaneFragment1D
    )
}

pub(crate) const fn is_traversed_reviewed_helper_v1(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::Invocation3DCurrent
            | TrustedDeviceItem::ExecutionWithWorkgroup
            | TrustedDeviceItem::Gfx942Wave64ReduceSum
            | TrustedDeviceItem::Gfx942Wave64InclusiveScanSum
            | TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum
            | TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic
    )
}

impl ProductionSemanticTerminalRuleV1 {
    pub(crate) const fn from_trusted_device_item(item: TrustedDeviceItem) -> Self {
        match item {
            TrustedDeviceItem::AmdGpuOrderedXorAddE32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32)
            }
            TrustedDeviceItem::AmdGpuOrderedProgramE32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942OrderedProgramE32)
            }
            TrustedDeviceItem::AmdGpuPhysicalEntryBeginGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryBegin)
            }
            TrustedDeviceItem::AmdGpuPhysicalGlobalCopyBeginGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyBegin)
            }
            TrustedDeviceItem::AmdGpuPhysicalLdsExchangeBeginGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeBegin)
            }
            TrustedDeviceItem::AmdGpuPhysicalEntryLabelGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryLabel)
            }
            TrustedDeviceItem::AmdGpuPhysicalGlobalCopyLabelGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyLabel)
            }
            TrustedDeviceItem::AmdGpuPhysicalLdsExchangeLabelGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeLabel)
            }
            TrustedDeviceItem::AmdGpuPhysicalEntryStepGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryStep)
            }
            TrustedDeviceItem::AmdGpuPhysicalGlobalCopyStepGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyStep)
            }
            TrustedDeviceItem::AmdGpuPhysicalLdsExchangeStepGfx942 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeStep)
            }
            TrustedDeviceItem::AmdGpuCompleteBodyE32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942CompleteBodyE32)
            }
            TrustedDeviceItem::AmdGpuInline(operation) => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942InlineU32(operation))
            }
            TrustedDeviceItem::KernelContextIssue => {
                Self::Expand(ProductionTerminalExpansionV1::ContextIssue)
            }
            TrustedDeviceItem::ExecutionWorkgroupCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupDerive)
            }
            TrustedDeviceItem::MaskedTile1DLoadMasked => {
                Self::Expand(ProductionTerminalExpansionV1::MaskedTileLoadU32)
            }
            TrustedDeviceItem::MaskedTile1DIntoFragment => {
                Self::Expand(ProductionTerminalExpansionV1::MaskedTileIntoFragmentU32)
            }
            TrustedDeviceItem::LaneFragment1DIntoParts => {
                Self::Expand(ProductionTerminalExpansionV1::LaneFragmentIntoPartsU32)
            }
            TrustedDeviceItem::ThreadIndexX => Self::Expand(
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::X),
            ),
            TrustedDeviceItem::ThreadIndexY => Self::Expand(
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Y),
            ),
            TrustedDeviceItem::ThreadIndexZ => Self::Expand(
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Z),
            ),
            TrustedDeviceItem::WorkgroupIndexX => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::X),
            ),
            TrustedDeviceItem::WorkgroupIndexY => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Y),
            ),
            TrustedDeviceItem::WorkgroupIndexZ => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Z),
            ),
            TrustedDeviceItem::WorkgroupDimensionX => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::X),
            ),
            TrustedDeviceItem::WorkgroupDimensionY => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Y),
            ),
            TrustedDeviceItem::WorkgroupDimensionZ => Self::Expand(
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Z),
            ),
            TrustedDeviceItem::GridDimensionX => Self::Expand(
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::X),
            ),
            TrustedDeviceItem::GridDimensionY => Self::Expand(
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Y),
            ),
            TrustedDeviceItem::GridDimensionZ => Self::Expand(
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Z),
            ),
            TrustedDeviceItem::ThreadIndex1d => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndex1d)
            }
            TrustedDeviceItem::ThreadIndexGet => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexGet)
            }
            TrustedDeviceItem::ThreadIndexIntoDisjoint => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint)
            }
            TrustedDeviceItem::ThreadIndexCheckedShift => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedShift)
            }
            TrustedDeviceItem::ThreadIndexCheckedBlock => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedBlock)
            }
            TrustedDeviceItem::ThreadIndexCheckedTiled2D => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d)
            }
            TrustedDeviceItem::ThreadIndexCheckedRowStriped2D => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d)
            }
            TrustedDeviceItem::DisjointIndexGet => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointIndexGet)
            }
            TrustedDeviceItem::DisjointBlockComponentIndex => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointBlockComponentIndex)
            }
            TrustedDeviceItem::DisjointIndexCheckedShift => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointIndexCheckedShift)
            }
            TrustedDeviceItem::DisjointSliceLen => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceLen)
            }
            TrustedDeviceItem::DisjointSliceGetMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMut)
            }
            TrustedDeviceItem::DisjointSliceGetDisjointMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut)
            }
            TrustedDeviceItem::GridLeaderCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::GridLeaderCurrent)
            }
            TrustedDeviceItem::DisjointSliceGetMutExclusive => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive)
            }
            TrustedDeviceItem::DisjointSliceGetBlockMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetBlockMut)
            }
            TrustedDeviceItem::DisjointSliceGetTiled2DMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut)
            }
            TrustedDeviceItem::DisjointSliceGetRowStriped2DMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceLen => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWrite => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWriteDisjoint => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWriteExclusive => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWriteBlock => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWriteTiled2D => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d)
            }
            TrustedDeviceItem::WriteOnlyDisjointSliceWriteRowStriped2D => {
                Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d)
            }
            TrustedDeviceItem::StridedReadView2DFromSharedSlice => {
                Self::Expand(ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice)
            }
            TrustedDeviceItem::StridedReadView2DLoadOr => {
                Self::Expand(ProductionTerminalExpansionV1::StridedReadView2DLoadOr)
            }
            TrustedDeviceItem::WorkgroupLdsScopeCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent)
            }
            TrustedDeviceItem::DynamicLdsExactCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::DynamicLdsExactCurrent)
            }
            TrustedDeviceItem::DynamicLdsIntoCollectiveRawParts => {
                Self::Expand(ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts)
            }
            TrustedDeviceItem::WorkgroupPipelineCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineCurrent)
            }
            TrustedDeviceItem::WorkgroupPipelineStage => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineStage)
            }
            TrustedDeviceItem::WorkgroupPipelineWrite => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineWrite)
            }
            TrustedDeviceItem::WorkgroupPipelineCommit => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineCommit)
            }
            TrustedDeviceItem::WorkgroupPipelineWait => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineWait)
            }
            TrustedDeviceItem::WorkgroupPipelineConsume => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineConsume)
            }
            TrustedDeviceItem::WorkgroupPipelineRead => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineRead)
            }
            TrustedDeviceItem::WorkgroupPipelineDiscard => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineDiscard)
            }
            TrustedDeviceItem::WorkgroupPipelineRelease => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineRelease)
            }
            TrustedDeviceItem::WorkgroupSyncthreads => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupBarrier)
            }
            TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::ContextFromCompiler) => {
                Self::Expand(ProductionTerminalExpansionV1::MathContextCurrent)
            }
            TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::F32(function)) => {
                Self::Expand(ProductionTerminalExpansionV1::MathF32(function))
            }
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromBits(
                NarrowFloatFormat::Bf16,
            )) => Self::Expand(ProductionTerminalExpansionV1::Bf16Conversion(
                ProductionBf16ConversionV1::FromBits,
            )),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToBits(
                NarrowFloatFormat::Bf16,
            )) => Self::Expand(ProductionTerminalExpansionV1::Bf16Conversion(
                ProductionBf16ConversionV1::ToBits,
            )),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromF32(
                NarrowFloatFormat::Bf16,
            )) => Self::Expand(ProductionTerminalExpansionV1::Bf16Conversion(
                ProductionBf16ConversionV1::FromF32RoundTiesEven,
            )),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToF32(
                NarrowFloatFormat::Bf16,
            )) => Self::Expand(ProductionTerminalExpansionV1::Bf16Conversion(
                ProductionBf16ConversionV1::ToF32,
            )),
            TrustedDeviceItem::Gfx942CollectivesCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::CollectiveContextCurrent)
            }
            TrustedDeviceItem::Gfx942Wave64Shuffle(scalar) => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx942Wave64Shuffle(scalar))
            }
            TrustedDeviceItem::WorkgroupCollectivesCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent)
            }
            TrustedDeviceItem::WorkgroupReduceSum => {
                Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum)
            }
            TrustedDeviceItem::WorkgroupInclusiveScanSum => {
                Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum)
            }
            TrustedDeviceItem::WorkgroupExclusiveScanSum => {
                Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum)
            }
            TrustedDeviceItem::Gfx942WorkgroupReduceSum => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupReduceSum)
            }
            TrustedDeviceItem::Gfx942SubgroupReduceSumF32 => {
                Self::Expand(ProductionTerminalExpansionV1::SubgroupReduceSumF32)
            }
            TrustedDeviceItem::Gfx942SubgroupReduceMaxF32 => {
                Self::Expand(ProductionTerminalExpansionV1::SubgroupReduceMaxF32)
            }
            TrustedDeviceItem::WaveLaneCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::WaveLaneCurrent)
            }
            TrustedDeviceItem::DeviceMatrixCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::MatrixContextCurrent)
            }
            TrustedDeviceItem::Bf16MfmaMatrixARowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixARowMajor)
            }
            TrustedDeviceItem::Bf16MfmaMatrixBRowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBRowMajor)
            }
            TrustedDeviceItem::Bf16MfmaBColumnMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBColumnMajor)
            }
            TrustedDeviceItem::Bf16MfmaBColumnMajorLoadZeroFilledV1 => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBColumnMajorLoadZeroFilledV1)
            }
            TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2 => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2)
            }
            TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2 => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2)
            }
            TrustedDeviceItem::F32AccumulatorFragmentZero => {
                Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorZero)
            }
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues => {
                Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues)
            }
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate => {
                Self::Expand(ProductionTerminalExpansionV1::MatrixMultiplyAccumulate)
            }
            TrustedDeviceItem::Gfx950MatrixCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixAFp4RowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixBFp4RowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixAFp4LoadM16K128 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixBFp4LoadK128N16 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16)
            }
            TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentZero => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero)
            }
            TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentIntoValues => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues)
            }
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate)
            }
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4Fp8 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixARowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixBRowMajor => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixAFp8LoadM16K128 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128)
            }
            TrustedDeviceItem::Gfx950MfmaMatrixBFp8LoadK128N16 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16)
            }
            TrustedDeviceItem::Gfx950F32AccumulatorFragmentZero => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero)
            }
            TrustedDeviceItem::Gfx950F32AccumulatorFragmentIntoValues => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues)
            }
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp8 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate)
            }
            TrustedDeviceItem::Gfx950SubgroupCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupCurrent)
            }
            TrustedDeviceItem::Gfx950SubgroupReduceMaxF32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32)
            }
            TrustedDeviceItem::Gfx950SubgroupReduceSumF32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32)
            }
            TrustedDeviceItem::Gfx950SubgroupBroadcastF32 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32)
            }
            TrustedDeviceItem::Gfx950LdsTransposeTileCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent)
            }
            TrustedDeviceItem::Gfx950LdsTransposeStageB4 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4)
            }
            TrustedDeviceItem::Gfx950LdsTransposeStageB8 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8)
            }
            TrustedDeviceItem::Gfx950LdsTransposePublish => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposePublish)
            }
            TrustedDeviceItem::Gfx950LdsTransposeReadB4 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4)
            }
            TrustedDeviceItem::Gfx950LdsTransposeReadB8 => {
                Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8)
            }
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap) => {
                Self::Expand(ProductionTerminalExpansionV1::Trap)
            }
            TrustedDeviceItem::MemoryVolatileLoad => {
                Self::Expand(ProductionTerminalExpansionV1::MemoryVolatileLoad)
            }
            unsupported => Self::Reject(unsupported),
        }
    }

    #[cfg(test)]
    const fn trusted_device_item(self) -> TrustedDeviceItem {
        match self {
            Self::Expand(ProductionTerminalExpansionV1::ContextIssue) => {
                TrustedDeviceItem::KernelContextIssue
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupDerive) => {
                TrustedDeviceItem::ExecutionWorkgroupCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::MaskedTileLoadU32) => {
                TrustedDeviceItem::MaskedTile1DLoadMasked
            }
            Self::Expand(ProductionTerminalExpansionV1::MaskedTileIntoFragmentU32) => {
                TrustedDeviceItem::MaskedTile1DIntoFragment
            }
            Self::Expand(ProductionTerminalExpansionV1::LaneFragmentIntoPartsU32) => {
                TrustedDeviceItem::LaneFragment1DIntoParts
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::X)) => {
                TrustedDeviceItem::ThreadIndexX
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Y)) => {
                TrustedDeviceItem::ThreadIndexY
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Z)) => {
                TrustedDeviceItem::ThreadIndexZ
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::X)) => {
                TrustedDeviceItem::WorkgroupIndexX
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Y)) => {
                TrustedDeviceItem::WorkgroupIndexY
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Z)) => {
                TrustedDeviceItem::WorkgroupIndexZ
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::X)) => {
                TrustedDeviceItem::WorkgroupDimensionX
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Y)) => {
                TrustedDeviceItem::WorkgroupDimensionY
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Z)) => {
                TrustedDeviceItem::WorkgroupDimensionZ
            }
            Self::Expand(ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::X)) => {
                TrustedDeviceItem::GridDimensionX
            }
            Self::Expand(ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Y)) => {
                TrustedDeviceItem::GridDimensionY
            }
            Self::Expand(ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Z)) => {
                TrustedDeviceItem::GridDimensionZ
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndex1d) => {
                TrustedDeviceItem::ThreadIndex1d
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexGet) => {
                TrustedDeviceItem::ThreadIndexGet
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint) => {
                TrustedDeviceItem::ThreadIndexIntoDisjoint
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedShift) => {
                TrustedDeviceItem::ThreadIndexCheckedShift
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedBlock) => {
                TrustedDeviceItem::ThreadIndexCheckedBlock
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d) => {
                TrustedDeviceItem::ThreadIndexCheckedTiled2D
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d) => {
                TrustedDeviceItem::ThreadIndexCheckedRowStriped2D
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointIndexGet) => {
                TrustedDeviceItem::DisjointIndexGet
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointBlockComponentIndex) => {
                TrustedDeviceItem::DisjointBlockComponentIndex
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointIndexCheckedShift) => {
                TrustedDeviceItem::DisjointIndexCheckedShift
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceLen) => {
                TrustedDeviceItem::DisjointSliceLen
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMut) => {
                TrustedDeviceItem::DisjointSliceGetMut
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut) => {
                TrustedDeviceItem::DisjointSliceGetDisjointMut
            }
            Self::Expand(ProductionTerminalExpansionV1::GridLeaderCurrent) => {
                TrustedDeviceItem::GridLeaderCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive) => {
                TrustedDeviceItem::DisjointSliceGetMutExclusive
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetBlockMut) => {
                TrustedDeviceItem::DisjointSliceGetBlockMut
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut) => {
                TrustedDeviceItem::DisjointSliceGetTiled2DMut
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut) => {
                TrustedDeviceItem::DisjointSliceGetRowStriped2DMut
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceLen
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceWrite
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceWriteDisjoint
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceWriteExclusive
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceWriteBlock
            }
            Self::Expand(ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d) => {
                TrustedDeviceItem::WriteOnlyDisjointSliceWriteTiled2D
            }
            Self::Expand(
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d,
            ) => TrustedDeviceItem::WriteOnlyDisjointSliceWriteRowStriped2D,
            Self::Expand(ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice) => {
                TrustedDeviceItem::StridedReadView2DFromSharedSlice
            }
            Self::Expand(ProductionTerminalExpansionV1::StridedReadView2DLoadOr) => {
                TrustedDeviceItem::StridedReadView2DLoadOr
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent) => {
                TrustedDeviceItem::WorkgroupLdsScopeCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::DynamicLdsExactCurrent) => {
                TrustedDeviceItem::DynamicLdsExactCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts) => {
                TrustedDeviceItem::DynamicLdsIntoCollectiveRawParts
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineCurrent) => {
                TrustedDeviceItem::WorkgroupPipelineCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineStage) => {
                TrustedDeviceItem::WorkgroupPipelineStage
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineWrite) => {
                TrustedDeviceItem::WorkgroupPipelineWrite
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineCommit) => {
                TrustedDeviceItem::WorkgroupPipelineCommit
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineWait) => {
                TrustedDeviceItem::WorkgroupPipelineWait
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineConsume) => {
                TrustedDeviceItem::WorkgroupPipelineConsume
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineRead) => {
                TrustedDeviceItem::WorkgroupPipelineRead
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineDiscard) => {
                TrustedDeviceItem::WorkgroupPipelineDiscard
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupPipelineRelease) => {
                TrustedDeviceItem::WorkgroupPipelineRelease
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupBarrier) => {
                TrustedDeviceItem::WorkgroupSyncthreads
            }
            Self::Expand(ProductionTerminalExpansionV1::MathContextCurrent) => {
                TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::ContextFromCompiler)
            }
            Self::Expand(ProductionTerminalExpansionV1::MathF32(function)) => {
                TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::F32(function))
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16Conversion(conversion)) => {
                TrustedDeviceItem::HalfOperation(match conversion {
                    ProductionBf16ConversionV1::FromBits => {
                        TrustedHalfOperation::FromBits(NarrowFloatFormat::Bf16)
                    }
                    ProductionBf16ConversionV1::ToBits => {
                        TrustedHalfOperation::ToBits(NarrowFloatFormat::Bf16)
                    }
                    ProductionBf16ConversionV1::FromF32RoundTiesEven => {
                        TrustedHalfOperation::FromF32(NarrowFloatFormat::Bf16)
                    }
                    ProductionBf16ConversionV1::ToF32 => {
                        TrustedHalfOperation::ToF32(NarrowFloatFormat::Bf16)
                    }
                })
            }
            Self::Expand(ProductionTerminalExpansionV1::CollectiveContextCurrent) => {
                TrustedDeviceItem::Gfx942CollectivesCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent) => {
                TrustedDeviceItem::WorkgroupCollectivesCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum) => {
                TrustedDeviceItem::WorkgroupReduceSum
            }
            Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum) => {
                TrustedDeviceItem::WorkgroupInclusiveScanSum
            }
            Self::Expand(ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum) => {
                TrustedDeviceItem::WorkgroupExclusiveScanSum
            }
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupReduceSum) => {
                TrustedDeviceItem::Gfx942WorkgroupReduceSum
            }
            Self::Expand(ProductionTerminalExpansionV1::SubgroupReduceSumF32) => {
                TrustedDeviceItem::Gfx942SubgroupReduceSumF32
            }
            Self::Expand(ProductionTerminalExpansionV1::SubgroupReduceMaxF32) => {
                TrustedDeviceItem::Gfx942SubgroupReduceMaxF32
            }
            Self::Expand(ProductionTerminalExpansionV1::WaveLaneCurrent) => {
                TrustedDeviceItem::WaveLaneCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::MatrixContextCurrent) => {
                TrustedDeviceItem::DeviceMatrixCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixARowMajor) => {
                TrustedDeviceItem::Bf16MfmaMatrixARowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBRowMajor) => {
                TrustedDeviceItem::Bf16MfmaMatrixBRowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBColumnMajor) => {
                TrustedDeviceItem::Bf16MfmaBColumnMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBColumnMajorLoadZeroFilledV1) => {
                TrustedDeviceItem::Bf16MfmaBColumnMajorLoadZeroFilledV1
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2) => {
                TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2) => {
                TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2
            }
            Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorZero) => {
                TrustedDeviceItem::F32AccumulatorFragmentZero
            }
            Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues) => {
                TrustedDeviceItem::F32AccumulatorFragmentIntoValues
            }
            Self::Expand(ProductionTerminalExpansionV1::MatrixMultiplyAccumulate) => {
                TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent) => {
                TrustedDeviceItem::Gfx950MatrixCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor) => {
                TrustedDeviceItem::Gfx950MfmaMatrixAFp4RowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor) => {
                TrustedDeviceItem::Gfx950MfmaMatrixBFp4RowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128) => {
                TrustedDeviceItem::Gfx950MfmaMatrixAFp4LoadM16K128
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16) => {
                TrustedDeviceItem::Gfx950MfmaMatrixBFp4LoadK128N16
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero) => {
                TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentZero
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues) => {
                TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentIntoValues
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate) => {
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate) => {
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4Fp8
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor) => {
                TrustedDeviceItem::Gfx950MfmaMatrixARowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor) => {
                TrustedDeviceItem::Gfx950MfmaMatrixBRowMajor
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128) => {
                TrustedDeviceItem::Gfx950MfmaMatrixAFp8LoadM16K128
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16) => {
                TrustedDeviceItem::Gfx950MfmaMatrixBFp8LoadK128N16
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero) => {
                TrustedDeviceItem::Gfx950F32AccumulatorFragmentZero
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues) => {
                TrustedDeviceItem::Gfx950F32AccumulatorFragmentIntoValues
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate) => {
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp8
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupCurrent) => {
                TrustedDeviceItem::Gfx950SubgroupCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32) => {
                TrustedDeviceItem::Gfx950SubgroupReduceMaxF32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32) => {
                TrustedDeviceItem::Gfx950SubgroupReduceSumF32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32) => {
                TrustedDeviceItem::Gfx950SubgroupBroadcastF32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent) => {
                TrustedDeviceItem::Gfx950LdsTransposeTileCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4) => {
                TrustedDeviceItem::Gfx950LdsTransposeStageB4
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8) => {
                TrustedDeviceItem::Gfx950LdsTransposeStageB8
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposePublish) => {
                TrustedDeviceItem::Gfx950LdsTransposePublish
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4) => {
                TrustedDeviceItem::Gfx950LdsTransposeReadB4
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8) => {
                TrustedDeviceItem::Gfx950LdsTransposeReadB8
            }
            Self::Expand(ProductionTerminalExpansionV1::Trap) => {
                TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap)
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32) => {
                TrustedDeviceItem::AmdGpuOrderedXorAddE32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942OrderedProgramE32) => {
                TrustedDeviceItem::AmdGpuOrderedProgramE32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryBegin) => {
                TrustedDeviceItem::AmdGpuPhysicalEntryBeginGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyBegin) => {
                TrustedDeviceItem::AmdGpuPhysicalGlobalCopyBeginGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeBegin) => {
                TrustedDeviceItem::AmdGpuPhysicalLdsExchangeBeginGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryLabel) => {
                TrustedDeviceItem::AmdGpuPhysicalEntryLabelGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyLabel) => {
                TrustedDeviceItem::AmdGpuPhysicalGlobalCopyLabelGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeLabel) => {
                TrustedDeviceItem::AmdGpuPhysicalLdsExchangeLabelGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalEntryStep) => {
                TrustedDeviceItem::AmdGpuPhysicalEntryStepGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyStep) => {
                TrustedDeviceItem::AmdGpuPhysicalGlobalCopyStepGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeStep) => {
                TrustedDeviceItem::AmdGpuPhysicalLdsExchangeStepGfx942
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942CompleteBodyE32) => {
                TrustedDeviceItem::AmdGpuCompleteBodyE32
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942InlineU32(operation)) => {
                TrustedDeviceItem::AmdGpuInline(operation)
            }
            Self::Expand(ProductionTerminalExpansionV1::MemoryVolatileLoad) => {
                TrustedDeviceItem::MemoryVolatileLoad
            }
            Self::Expand(ProductionTerminalExpansionV1::Gfx942Wave64Shuffle(scalar)) => {
                TrustedDeviceItem::Gfx942Wave64Shuffle(scalar)
            }
            Self::Expand(
                ProductionTerminalExpansionV1::ColdPath
                | ProductionTerminalExpansionV1::RustcFabsF32
                | ProductionTerminalExpansionV1::RustcSaturatingInteger(_),
            ) => {
                panic!("core compiler intrinsics are not trusted device items")
            }
            Self::Reject(item) => item,
        }
    }
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<ProductionSemanticTerminalRuleV1> {
    trusted_device_items::classify(tcx, def_id)
        .and_then(|item| {
            (!is_traversed_reviewed_helper_v1(item))
                .then(|| ProductionSemanticTerminalRuleV1::from_trusted_device_item(item))
        })
        .or_else(|| {
            let intrinsic = tcx.intrinsic(def_id)?;
            (intrinsic.name == sym::cold_path).then_some(ProductionSemanticTerminalRuleV1::Expand(
                ProductionTerminalExpansionV1::ColdPath,
            ))
        })
}

#[cfg(test)]
#[path = "production_semantic_terminal_v1_tests.rs"]
mod tests;
