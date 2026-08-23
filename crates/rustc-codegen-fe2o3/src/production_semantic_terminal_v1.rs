//! Workload-neutral disposition of reviewed device semantic terminals.

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1;

use crate::trusted_device_items::{self, TrustedDeviceItem};

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
    DisjointIndexGet,
    DisjointIndexCheckedShift,
    DisjointSliceLen,
    DisjointSliceGetMut,
    DisjointSliceGetDisjointMut,
    GridLeaderCurrent,
    DisjointSliceGetMutExclusive,
    DisjointSliceGetBlockMut,
    DisjointSliceGetTiled2dMut,
    WorkgroupBarrier,
    MatrixContextCurrent,
    Bf16MatrixFragmentFromBits,
    F32MatrixAccumulatorFromValues,
    F32MatrixAccumulatorIntoValues,
    MatrixMultiplyAccumulate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSemanticTerminalRuleV1 {
    Expand(ProductionTerminalExpansionV1),
    Reject(TrustedDeviceItem),
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
            TrustedDeviceItem::WorkgroupSyncthreads => {
                Self::Expand(ProductionTerminalExpansionV1::WorkgroupBarrier)
            }
            TrustedDeviceItem::DeviceMatrixCurrent => {
                Self::Expand(ProductionTerminalExpansionV1::MatrixContextCurrent)
            }
            TrustedDeviceItem::Bf16MfmaFragmentFromBits => {
                Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixFragmentFromBits)
            }
            TrustedDeviceItem::F32AccumulatorFragmentFromValues => {
                Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorFromValues)
            }
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues => {
                Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues)
            }
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate => {
                Self::Expand(ProductionTerminalExpansionV1::MatrixMultiplyAccumulate)
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
            Self::Expand(ProductionTerminalExpansionV1::WorkgroupBarrier) => {
                TrustedDeviceItem::WorkgroupSyncthreads
            }
            Self::Expand(ProductionTerminalExpansionV1::MatrixContextCurrent) => {
                TrustedDeviceItem::DeviceMatrixCurrent
            }
            Self::Expand(ProductionTerminalExpansionV1::Bf16MatrixFragmentFromBits) => {
                TrustedDeviceItem::Bf16MfmaFragmentFromBits
            }
            Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorFromValues) => {
                TrustedDeviceItem::F32AccumulatorFragmentFromValues
            }
            Self::Expand(ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues) => {
                TrustedDeviceItem::F32AccumulatorFragmentIntoValues
            }
            Self::Expand(ProductionTerminalExpansionV1::MatrixMultiplyAccumulate) => {
                TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
            }
            Self::Reject(item) => item,
        }
    }
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<ProductionSemanticTerminalRuleV1> {
    trusted_device_items::classify(tcx, def_id)
        .map(ProductionSemanticTerminalRuleV1::from_trusted_device_item)
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
                TrustedDeviceItem::DeviceMatrixCurrent,
                ProductionTerminalExpansionV1::MatrixContextCurrent,
            ),
            (
                TrustedDeviceItem::Bf16MfmaFragmentFromBits,
                ProductionTerminalExpansionV1::Bf16MatrixFragmentFromBits,
            ),
            (
                TrustedDeviceItem::F32AccumulatorFragmentFromValues,
                ProductionTerminalExpansionV1::F32MatrixAccumulatorFromValues,
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
                TrustedDeviceItem::ThreadIndexCheckedTiled2D,
                ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetTiled2DMut,
                ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut,
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
            TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic,
            TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
        ] {
            let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
            assert_eq!(rule, ProductionSemanticTerminalRuleV1::Reject(item));
            assert_eq!(rule.trusted_device_item(), item);
        }
    }
}
