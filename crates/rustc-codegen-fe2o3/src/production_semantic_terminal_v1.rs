//! Workload-neutral disposition of reviewed device semantic terminals.

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

use dialect_amdgcn::DeviceMathDiagnosticItem;
use fe2o3_kernel_ir::{F32MathFunction, NarrowFloatFormat};
use fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1;

use crate::trusted_device_items::{
    self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem, TrustedHalfOperation,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionBf16ConversionV1 {
    FromBits,
    ToBits,
    FromF32RoundTiesEven,
    ToF32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionTerminalExpansionV1 {
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
    Bf16Conversion(ProductionBf16ConversionV1),
    CollectiveContextCurrent,
    WorkgroupReduceSum,
    SubgroupReduceSumF32,
    SubgroupReduceMaxF32,
    WaveLaneCurrent,
    MatrixContextCurrent,
    Bf16MatrixARowMajor,
    Bf16MatrixBRowMajor,
    Bf16MatrixALoadZeroFilledV2,
    Bf16MatrixBLoadZeroFilledV2,
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

pub(crate) const fn is_traversed_reviewed_helper_v1(item: TrustedDeviceItem) -> bool {
    matches!(
        item,
        TrustedDeviceItem::WorkgroupLdsScopeCurrent
            | TrustedDeviceItem::Invocation3DCurrent
            | TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic
    )
}

impl ProductionSemanticTerminalRuleV1 {
    pub(crate) const fn from_trusted_device_item(item: TrustedDeviceItem) -> Self {
        match item {
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
            unsupported => Self::Reject(unsupported),
        }
    }

    #[cfg(test)]
    const fn trusted_device_item(self) -> TrustedDeviceItem {
        match self {
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
            Self::Expand(ProductionTerminalExpansionV1::ColdPath) => {
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
mod tests {
    use super::*;

    #[test]
    fn fill_terminals_have_explicit_workload_neutral_expansions() {
        let cases = [
            (
                TrustedDeviceItem::ThreadIndexX,
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::X),
            ),
            (
                TrustedDeviceItem::ThreadIndexY,
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Y),
            ),
            (
                TrustedDeviceItem::ThreadIndexZ,
                ProductionTerminalExpansionV1::ThreadIndex(SemanticAxisV1::Z),
            ),
            (
                TrustedDeviceItem::WorkgroupIndexX,
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::X),
            ),
            (
                TrustedDeviceItem::WorkgroupIndexY,
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Y),
            ),
            (
                TrustedDeviceItem::WorkgroupIndexZ,
                ProductionTerminalExpansionV1::WorkgroupIndex(SemanticAxisV1::Z),
            ),
            (
                TrustedDeviceItem::WorkgroupDimensionX,
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::X),
            ),
            (
                TrustedDeviceItem::WorkgroupDimensionY,
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Y),
            ),
            (
                TrustedDeviceItem::WorkgroupDimensionZ,
                ProductionTerminalExpansionV1::WorkgroupDimension(SemanticAxisV1::Z),
            ),
            (
                TrustedDeviceItem::GridDimensionX,
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::X),
            ),
            (
                TrustedDeviceItem::GridDimensionY,
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Y),
            ),
            (
                TrustedDeviceItem::GridDimensionZ,
                ProductionTerminalExpansionV1::GridDimension(SemanticAxisV1::Z),
            ),
            (
                TrustedDeviceItem::ThreadIndex1d,
                ProductionTerminalExpansionV1::ThreadIndex1d,
            ),
            (
                TrustedDeviceItem::ThreadIndexGet,
                ProductionTerminalExpansionV1::ThreadIndexGet,
            ),
            (
                TrustedDeviceItem::ThreadIndexIntoDisjoint,
                ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint,
            ),
            (
                TrustedDeviceItem::ThreadIndexCheckedShift,
                ProductionTerminalExpansionV1::ThreadIndexCheckedShift,
            ),
            (
                TrustedDeviceItem::DisjointIndexGet,
                ProductionTerminalExpansionV1::DisjointIndexGet,
            ),
            (
                TrustedDeviceItem::DisjointIndexCheckedShift,
                ProductionTerminalExpansionV1::DisjointIndexCheckedShift,
            ),
            (
                TrustedDeviceItem::DisjointSliceLen,
                ProductionTerminalExpansionV1::DisjointSliceLen,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetMut,
                ProductionTerminalExpansionV1::DisjointSliceGetMut,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetDisjointMut,
                ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut,
            ),
            (
                TrustedDeviceItem::GridLeaderCurrent,
                ProductionTerminalExpansionV1::GridLeaderCurrent,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetMutExclusive,
                ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive,
            ),
            (
                TrustedDeviceItem::WorkgroupSyncthreads,
                ProductionTerminalExpansionV1::WorkgroupBarrier,
            ),
            (
                TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::ContextFromCompiler),
                ProductionTerminalExpansionV1::MathContextCurrent,
            ),
            (
                TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::F32(F32MathFunction::Exp)),
                ProductionTerminalExpansionV1::MathF32(F32MathFunction::Exp),
            ),
            (
                TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromBits(
                    NarrowFloatFormat::Bf16,
                )),
                ProductionTerminalExpansionV1::Bf16Conversion(ProductionBf16ConversionV1::FromBits),
            ),
            (
                TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToBits(
                    NarrowFloatFormat::Bf16,
                )),
                ProductionTerminalExpansionV1::Bf16Conversion(ProductionBf16ConversionV1::ToBits),
            ),
            (
                TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromF32(
                    NarrowFloatFormat::Bf16,
                )),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    ProductionBf16ConversionV1::FromF32RoundTiesEven,
                ),
            ),
            (
                TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToF32(
                    NarrowFloatFormat::Bf16,
                )),
                ProductionTerminalExpansionV1::Bf16Conversion(ProductionBf16ConversionV1::ToF32),
            ),
            (
                TrustedDeviceItem::Gfx942CollectivesCurrent,
                ProductionTerminalExpansionV1::CollectiveContextCurrent,
            ),
            (
                TrustedDeviceItem::Gfx942SubgroupReduceSumF32,
                ProductionTerminalExpansionV1::SubgroupReduceSumF32,
            ),
            (
                TrustedDeviceItem::Gfx942SubgroupReduceMaxF32,
                ProductionTerminalExpansionV1::SubgroupReduceMaxF32,
            ),
            (
                TrustedDeviceItem::WaveLaneCurrent,
                ProductionTerminalExpansionV1::WaveLaneCurrent,
            ),
            (
                TrustedDeviceItem::DeviceMatrixCurrent,
                ProductionTerminalExpansionV1::MatrixContextCurrent,
            ),
            (
                TrustedDeviceItem::Bf16MfmaMatrixARowMajor,
                ProductionTerminalExpansionV1::Bf16MatrixARowMajor,
            ),
            (
                TrustedDeviceItem::Bf16MfmaMatrixBRowMajor,
                ProductionTerminalExpansionV1::Bf16MatrixBRowMajor,
            ),
            (
                TrustedDeviceItem::Bf16MfmaMatrixALoadZeroFilledV2,
                ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2,
            ),
            (
                TrustedDeviceItem::Bf16MfmaMatrixBLoadZeroFilledV2,
                ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2,
            ),
            (
                TrustedDeviceItem::F32AccumulatorFragmentZero,
                ProductionTerminalExpansionV1::F32MatrixAccumulatorZero,
            ),
            (
                TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
                ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues,
            ),
            (
                TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
                ProductionTerminalExpansionV1::MatrixMultiplyAccumulate,
            ),
            (
                TrustedDeviceItem::Gfx950MatrixCurrent,
                ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixAFp4RowMajor,
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixBFp4RowMajor,
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixAFp4LoadM16K128,
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixBFp4LoadK128N16,
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16,
            ),
            (
                TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentZero,
                ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero,
            ),
            (
                TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentIntoValues,
                ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues,
            ),
            (
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4,
                ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate,
            ),
            (
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4Fp8,
                ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixARowMajor,
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixBRowMajor,
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixAFp8LoadM16K128,
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128,
            ),
            (
                TrustedDeviceItem::Gfx950MfmaMatrixBFp8LoadK128N16,
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16,
            ),
            (
                TrustedDeviceItem::Gfx950F32AccumulatorFragmentZero,
                ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero,
            ),
            (
                TrustedDeviceItem::Gfx950F32AccumulatorFragmentIntoValues,
                ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues,
            ),
            (
                TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp8,
                ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate,
            ),
            (
                TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
                ProductionTerminalExpansionV1::Trap,
            ),
            (
                TrustedDeviceItem::ThreadIndexCheckedTiled2D,
                ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetTiled2DMut,
                ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut,
            ),
            (
                TrustedDeviceItem::ThreadIndexCheckedRowStriped2D,
                ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetRowStriped2DMut,
                ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut,
            ),
        ];
        for (item, expansion) in cases {
            let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
            assert_eq!(rule, ProductionSemanticTerminalRuleV1::Expand(expansion));
            assert_eq!(rule.trusted_device_item(), item);
        }
    }

    #[test]
    fn every_unimplemented_terminal_is_retained_as_an_explicit_rejection() {
        for item in [
            TrustedDeviceItem::MemoryVolatileLoad,
            TrustedDeviceItem::MemoryVolatileStore,
            TrustedDeviceItem::MemoryCopyNonOverlapping,
            TrustedDeviceItem::MemoryCopyOneNonOverlapping,
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromBits(
                NarrowFloatFormat::F16,
            )),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToBits(NarrowFloatFormat::F16)),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::FromF32(NarrowFloatFormat::F16)),
            TrustedDeviceItem::HalfOperation(TrustedHalfOperation::ToF32(NarrowFloatFormat::F16)),
        ] {
            let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
            assert_eq!(rule, ProductionSemanticTerminalRuleV1::Reject(item));
            assert_eq!(rule.trusted_device_item(), item);
        }
    }

    #[test]
    fn reviewed_rust_helpers_are_traversed_instead_of_hidden_by_a_terminal() {
        for item in [
            TrustedDeviceItem::WorkgroupLdsScopeCurrent,
            TrustedDeviceItem::Invocation3DCurrent,
            TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
        ] {
            assert!(is_traversed_reviewed_helper_v1(item));
        }
    }
}
