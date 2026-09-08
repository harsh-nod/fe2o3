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

/// Closed source signatures for reviewed typed-global operations.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionCapabilityMemoryTerminalV1 {
    /// `(&KernelContext<K,T,L>, &[E]) -> Global<E, ReadOnly, Brand<K,T,L>>`.
    BindReadOnly,
    /// `(&KernelContext<K,T,L>, &mut [E]) -> Global<E, ExclusiveReadWrite, Brand<K,T,L>>`.
    BindExclusiveReadWrite,
    /// `(&KernelContext<K,T,L>, WriteOnlyDisjointSlice<E,I>) ->
    /// Global<E, DisjointWrite<I>, Brand<K,T,L>>`.
    BindDisjointWrite,
    /// `(&Global<E, ReadOnly, B>, usize) -> Option<E>` for an admitted scalar `E`.
    Load,
    /// `(&Global<E, ExclusiveReadWrite, B>, usize) -> Option<E>`.
    ExclusiveLoad,
    /// `(&mut Global<E, DisjointWrite<I>, B>, DisjointIndex<I>, E) -> bool`.
    Store,
    /// `(&mut Global<E, ExclusiveReadWrite, B>, usize, E) -> bool`.
    ExclusiveStore,
    /// `(&mut Global<E, DisjointWrite<Blocked<I,L,E>>, B>,
    /// &DisjointBlock<I,L,E,B>, usize, E) -> bool`.
    StoreBlock,
}

#[cfg(test)]
impl ProductionCapabilityMemoryTerminalV1 {
    pub(crate) const fn source_argument_count(self) -> usize {
        match self {
            Self::BindReadOnly
            | Self::BindExclusiveReadWrite
            | Self::BindDisjointWrite
            | Self::Load
            | Self::ExclusiveLoad => 2,
            Self::Store | Self::ExclusiveStore => 3,
            Self::StoreBlock => 4,
        }
    }
}

#[cfg(test)]
pub(crate) const fn capability_memory_terminal_contract_v1(
    item: TrustedDeviceItem,
) -> Option<ProductionCapabilityMemoryTerminalV1> {
    match item {
        TrustedDeviceItem::CapabilityGlobalBindReadOnly => {
            Some(ProductionCapabilityMemoryTerminalV1::BindReadOnly)
        }
        TrustedDeviceItem::CapabilityGlobalBindExclusiveReadWrite => {
            Some(ProductionCapabilityMemoryTerminalV1::BindExclusiveReadWrite)
        }
        TrustedDeviceItem::CapabilityGlobalBindDisjointWrite => {
            Some(ProductionCapabilityMemoryTerminalV1::BindDisjointWrite)
        }
        TrustedDeviceItem::CapabilityGlobalLoad => Some(ProductionCapabilityMemoryTerminalV1::Load),
        TrustedDeviceItem::CapabilityGlobalExclusiveLoad => {
            Some(ProductionCapabilityMemoryTerminalV1::ExclusiveLoad)
        }
        TrustedDeviceItem::CapabilityGlobalStore => {
            Some(ProductionCapabilityMemoryTerminalV1::Store)
        }
        TrustedDeviceItem::CapabilityGlobalExclusiveStore => {
            Some(ProductionCapabilityMemoryTerminalV1::ExclusiveStore)
        }
        TrustedDeviceItem::CapabilityGlobalStoreBlock => {
            Some(ProductionCapabilityMemoryTerminalV1::StoreBlock)
        }
        _ => None,
    }
}

/// Closed source terminals for the target-neutral execution capability surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionExecutionTerminalV1 {
    BindAtomicView,
    WorkgroupDerive,
    SubgroupDerive,
    LdsAllocate,
    WorkgroupBarrier,
    SubgroupBarrier,
    WorkgroupFence,
    LdsPublish,
    AsyncCopy,
    AsyncWait,
    WorkgroupReduceSum,
    WorkgroupInclusiveScanSum,
    WorkgroupExclusiveScanSum,
    BindGlobalAtomicLocation,
    AtomicLoad,
    AtomicStore,
    AtomicFetchAdd,
    AtomicCompareExchange,
    SubgroupFence,
    SubgroupReduceSum,
    SubgroupInclusiveScanSum,
    MatrixAccess,
    LdsInitializeByInvocation,
    LdsReadPublished,
    PrivateMemoryFromRawParts,
    WorkgroupMemoryFromRawParts,
    PrivateMemoryAllocate,
    WorkgroupMemoryIndex1D,
    WorkgroupMemoryAllocate,
    WorkgroupMemoryPublish,
    PrivateMemoryLoad,
    PrivateMemoryExclusiveLoad,
    PrivateMemoryExclusiveStore,
    PrivateMemoryDisjointStore,
    WorkgroupMemoryLoad,
    WorkgroupMemoryExclusiveLoad,
    WorkgroupMemoryExclusiveStore,
    WorkgroupMemoryDisjointStore,
    GlobalBindExclusiveReadWrite,
    GlobalExclusiveLoad,
    GlobalExclusiveStore,
    GlobalStoreBlock,
}

impl ProductionExecutionTerminalV1 {
    pub(crate) const fn is_typed_global_memory(self) -> bool {
        matches!(
            self,
            Self::GlobalBindExclusiveReadWrite
                | Self::GlobalExclusiveLoad
                | Self::GlobalExclusiveStore
                | Self::GlobalStoreBlock
        )
    }

    #[cfg(test)]
    const ALL: [Self; 42] = [
        Self::BindAtomicView,
        Self::WorkgroupDerive,
        Self::SubgroupDerive,
        Self::LdsAllocate,
        Self::WorkgroupBarrier,
        Self::SubgroupBarrier,
        Self::WorkgroupFence,
        Self::LdsPublish,
        Self::AsyncCopy,
        Self::AsyncWait,
        Self::WorkgroupReduceSum,
        Self::WorkgroupInclusiveScanSum,
        Self::WorkgroupExclusiveScanSum,
        Self::BindGlobalAtomicLocation,
        Self::AtomicLoad,
        Self::AtomicStore,
        Self::AtomicFetchAdd,
        Self::AtomicCompareExchange,
        Self::SubgroupFence,
        Self::SubgroupReduceSum,
        Self::SubgroupInclusiveScanSum,
        Self::MatrixAccess,
        Self::LdsInitializeByInvocation,
        Self::LdsReadPublished,
        Self::PrivateMemoryFromRawParts,
        Self::WorkgroupMemoryFromRawParts,
        Self::PrivateMemoryAllocate,
        Self::WorkgroupMemoryIndex1D,
        Self::WorkgroupMemoryAllocate,
        Self::WorkgroupMemoryPublish,
        Self::PrivateMemoryLoad,
        Self::PrivateMemoryExclusiveLoad,
        Self::PrivateMemoryExclusiveStore,
        Self::PrivateMemoryDisjointStore,
        Self::WorkgroupMemoryLoad,
        Self::WorkgroupMemoryExclusiveLoad,
        Self::WorkgroupMemoryExclusiveStore,
        Self::WorkgroupMemoryDisjointStore,
        Self::GlobalBindExclusiveReadWrite,
        Self::GlobalExclusiveLoad,
        Self::GlobalExclusiveStore,
        Self::GlobalStoreBlock,
    ];

    pub(crate) const fn identity_tag(self) -> u8 {
        match self {
            Self::BindAtomicView => 0,
            Self::WorkgroupDerive => 1,
            Self::SubgroupDerive => 2,
            Self::LdsAllocate => 3,
            Self::WorkgroupBarrier => 4,
            Self::SubgroupBarrier => 5,
            Self::WorkgroupFence => 6,
            Self::LdsPublish => 7,
            Self::AsyncCopy => 8,
            Self::AsyncWait => 9,
            Self::WorkgroupReduceSum => 10,
            Self::WorkgroupInclusiveScanSum => 11,
            Self::WorkgroupExclusiveScanSum => 12,
            Self::BindGlobalAtomicLocation => 13,
            Self::AtomicLoad => 14,
            Self::AtomicStore => 15,
            Self::AtomicFetchAdd => 16,
            Self::AtomicCompareExchange => 17,
            Self::SubgroupFence => 18,
            Self::SubgroupReduceSum => 19,
            Self::SubgroupInclusiveScanSum => 20,
            Self::MatrixAccess => 21,
            Self::LdsInitializeByInvocation => 22,
            Self::LdsReadPublished => 23,
            Self::PrivateMemoryFromRawParts => 24,
            Self::WorkgroupMemoryFromRawParts => 25,
            Self::PrivateMemoryAllocate => 26,
            Self::WorkgroupMemoryIndex1D => 27,
            Self::WorkgroupMemoryAllocate => 28,
            Self::WorkgroupMemoryPublish => 29,
            Self::PrivateMemoryLoad => 30,
            Self::PrivateMemoryExclusiveLoad => 31,
            Self::PrivateMemoryExclusiveStore => 32,
            Self::PrivateMemoryDisjointStore => 33,
            Self::WorkgroupMemoryLoad => 34,
            Self::WorkgroupMemoryExclusiveLoad => 35,
            Self::WorkgroupMemoryExclusiveStore => 36,
            Self::WorkgroupMemoryDisjointStore => 37,
            Self::GlobalBindExclusiveReadWrite => 38,
            Self::GlobalExclusiveLoad => 39,
            Self::GlobalExclusiveStore => 40,
            Self::GlobalStoreBlock => 41,
        }
    }

    pub(crate) const fn source_argument_count(self) -> usize {
        match self {
            Self::WorkgroupDerive
            | Self::SubgroupDerive
            | Self::LdsAllocate
            | Self::WorkgroupBarrier
            | Self::PrivateMemoryAllocate
            | Self::WorkgroupMemoryIndex1D
            | Self::WorkgroupMemoryAllocate => 1,
            Self::BindAtomicView
            | Self::SubgroupBarrier
            | Self::WorkgroupFence
            | Self::LdsPublish
            | Self::AsyncWait
            | Self::AtomicLoad
            | Self::SubgroupFence
            | Self::MatrixAccess
            | Self::WorkgroupMemoryPublish
            | Self::PrivateMemoryLoad
            | Self::PrivateMemoryExclusiveLoad
            | Self::GlobalBindExclusiveReadWrite
            | Self::GlobalExclusiveLoad => 2,
            Self::WorkgroupReduceSum
            | Self::WorkgroupInclusiveScanSum
            | Self::WorkgroupExclusiveScanSum
            | Self::BindGlobalAtomicLocation
            | Self::AtomicStore
            | Self::AtomicFetchAdd
            | Self::SubgroupReduceSum
            | Self::SubgroupInclusiveScanSum
            | Self::LdsInitializeByInvocation
            | Self::LdsReadPublished
            | Self::PrivateMemoryExclusiveStore
            | Self::PrivateMemoryDisjointStore
            | Self::WorkgroupMemoryLoad
            | Self::WorkgroupMemoryExclusiveLoad
            | Self::GlobalExclusiveStore => 3,
            Self::AsyncCopy
            | Self::AtomicCompareExchange
            | Self::PrivateMemoryFromRawParts
            | Self::WorkgroupMemoryFromRawParts
            | Self::WorkgroupMemoryExclusiveStore
            | Self::WorkgroupMemoryDisjointStore
            | Self::GlobalStoreBlock => 4,
        }
    }

    pub(crate) const fn trusted_device_item(self) -> TrustedDeviceItem {
        match self {
            Self::BindAtomicView => TrustedDeviceItem::CapabilityGlobalBindAtomic,
            Self::WorkgroupDerive => TrustedDeviceItem::ExecutionWorkgroupCurrent,
            Self::SubgroupDerive => TrustedDeviceItem::ExecutionSubgroupCurrent,
            Self::LdsAllocate => TrustedDeviceItem::ExecutionLdsAllocate,
            Self::WorkgroupBarrier => TrustedDeviceItem::ExecutionWorkgroupBarrier,
            Self::SubgroupBarrier => TrustedDeviceItem::ExecutionSubgroupBarrier,
            Self::WorkgroupFence => TrustedDeviceItem::ExecutionWorkgroupFence,
            Self::LdsPublish => TrustedDeviceItem::ExecutionLdsPublish,
            Self::AsyncCopy => TrustedDeviceItem::ExecutionAsyncCopy,
            Self::AsyncWait => TrustedDeviceItem::ExecutionAsyncWait,
            Self::WorkgroupReduceSum => TrustedDeviceItem::ExecutionWorkgroupReduceSum,
            Self::WorkgroupInclusiveScanSum => {
                TrustedDeviceItem::ExecutionWorkgroupInclusiveScanSum
            }
            Self::WorkgroupExclusiveScanSum => {
                TrustedDeviceItem::ExecutionWorkgroupExclusiveScanSum
            }
            Self::BindGlobalAtomicLocation => TrustedDeviceItem::ExecutionGlobalAtomic,
            Self::AtomicLoad => TrustedDeviceItem::ExecutionAtomicLoad,
            Self::AtomicStore => TrustedDeviceItem::ExecutionAtomicStore,
            Self::AtomicFetchAdd => TrustedDeviceItem::ExecutionAtomicFetchAdd,
            Self::AtomicCompareExchange => TrustedDeviceItem::ExecutionAtomicCompareExchange,
            Self::SubgroupFence => TrustedDeviceItem::ExecutionSubgroupFence,
            Self::SubgroupReduceSum => TrustedDeviceItem::ExecutionSubgroupReduceSum,
            Self::SubgroupInclusiveScanSum => TrustedDeviceItem::ExecutionSubgroupInclusiveScanSum,
            Self::MatrixAccess => TrustedDeviceItem::ExecutionMatrixAccess,
            Self::LdsInitializeByInvocation => {
                TrustedDeviceItem::ExecutionLdsInitializeByInvocation
            }
            Self::LdsReadPublished => TrustedDeviceItem::ExecutionLdsReadPublished,
            Self::PrivateMemoryFromRawParts => TrustedDeviceItem::PrivateMemoryFromRawParts,
            Self::WorkgroupMemoryFromRawParts => TrustedDeviceItem::WorkgroupMemoryFromRawParts,
            Self::PrivateMemoryAllocate => TrustedDeviceItem::PrivateMemoryAllocate,
            Self::WorkgroupMemoryIndex1D => TrustedDeviceItem::WorkgroupMemoryIndex1D,
            Self::WorkgroupMemoryAllocate => TrustedDeviceItem::WorkgroupMemoryAllocate,
            Self::WorkgroupMemoryPublish => TrustedDeviceItem::WorkgroupMemoryPublish,
            Self::PrivateMemoryLoad => TrustedDeviceItem::PrivateMemoryLoad,
            Self::PrivateMemoryExclusiveLoad => TrustedDeviceItem::PrivateMemoryExclusiveLoad,
            Self::PrivateMemoryExclusiveStore => TrustedDeviceItem::PrivateMemoryExclusiveStore,
            Self::PrivateMemoryDisjointStore => TrustedDeviceItem::PrivateMemoryDisjointStore,
            Self::WorkgroupMemoryLoad => TrustedDeviceItem::WorkgroupMemoryLoad,
            Self::WorkgroupMemoryExclusiveLoad => TrustedDeviceItem::WorkgroupMemoryExclusiveLoad,
            Self::WorkgroupMemoryExclusiveStore => TrustedDeviceItem::WorkgroupMemoryExclusiveStore,
            Self::WorkgroupMemoryDisjointStore => TrustedDeviceItem::WorkgroupMemoryDisjointStore,
            Self::GlobalBindExclusiveReadWrite => {
                TrustedDeviceItem::CapabilityGlobalBindExclusiveReadWrite
            }
            Self::GlobalExclusiveLoad => TrustedDeviceItem::CapabilityGlobalExclusiveLoad,
            Self::GlobalExclusiveStore => TrustedDeviceItem::CapabilityGlobalExclusiveStore,
            Self::GlobalStoreBlock => TrustedDeviceItem::CapabilityGlobalStoreBlock,
        }
    }
}

pub(crate) const fn execution_terminal_contract_v1(
    item: TrustedDeviceItem,
) -> Option<ProductionExecutionTerminalV1> {
    use ProductionExecutionTerminalV1 as Terminal;
    Some(match item {
        TrustedDeviceItem::CapabilityGlobalBindAtomic => Terminal::BindAtomicView,
        TrustedDeviceItem::ExecutionWorkgroupCurrent => Terminal::WorkgroupDerive,
        TrustedDeviceItem::ExecutionSubgroupCurrent => Terminal::SubgroupDerive,
        TrustedDeviceItem::ExecutionLdsAllocate => Terminal::LdsAllocate,
        TrustedDeviceItem::ExecutionWorkgroupBarrier => Terminal::WorkgroupBarrier,
        TrustedDeviceItem::ExecutionSubgroupBarrier => Terminal::SubgroupBarrier,
        TrustedDeviceItem::ExecutionWorkgroupFence => Terminal::WorkgroupFence,
        TrustedDeviceItem::ExecutionLdsPublish => Terminal::LdsPublish,
        TrustedDeviceItem::ExecutionAsyncCopy => Terminal::AsyncCopy,
        TrustedDeviceItem::ExecutionAsyncWait => Terminal::AsyncWait,
        TrustedDeviceItem::ExecutionWorkgroupReduceSum => Terminal::WorkgroupReduceSum,
        TrustedDeviceItem::ExecutionWorkgroupInclusiveScanSum => {
            Terminal::WorkgroupInclusiveScanSum
        }
        TrustedDeviceItem::ExecutionWorkgroupExclusiveScanSum => {
            Terminal::WorkgroupExclusiveScanSum
        }
        TrustedDeviceItem::ExecutionGlobalAtomic => Terminal::BindGlobalAtomicLocation,
        TrustedDeviceItem::ExecutionAtomicLoad => Terminal::AtomicLoad,
        TrustedDeviceItem::ExecutionAtomicStore => Terminal::AtomicStore,
        TrustedDeviceItem::ExecutionAtomicFetchAdd => Terminal::AtomicFetchAdd,
        TrustedDeviceItem::ExecutionAtomicCompareExchange => Terminal::AtomicCompareExchange,
        TrustedDeviceItem::ExecutionSubgroupFence => Terminal::SubgroupFence,
        TrustedDeviceItem::ExecutionSubgroupReduceSum => Terminal::SubgroupReduceSum,
        TrustedDeviceItem::ExecutionSubgroupInclusiveScanSum => Terminal::SubgroupInclusiveScanSum,
        TrustedDeviceItem::ExecutionMatrixAccess => Terminal::MatrixAccess,
        TrustedDeviceItem::ExecutionLdsInitializeByInvocation => {
            Terminal::LdsInitializeByInvocation
        }
        TrustedDeviceItem::ExecutionLdsReadPublished => Terminal::LdsReadPublished,
        TrustedDeviceItem::PrivateMemoryFromRawParts => Terminal::PrivateMemoryFromRawParts,
        TrustedDeviceItem::WorkgroupMemoryFromRawParts => Terminal::WorkgroupMemoryFromRawParts,
        TrustedDeviceItem::PrivateMemoryAllocate => Terminal::PrivateMemoryAllocate,
        TrustedDeviceItem::WorkgroupMemoryIndex1D => Terminal::WorkgroupMemoryIndex1D,
        TrustedDeviceItem::WorkgroupMemoryAllocate => Terminal::WorkgroupMemoryAllocate,
        TrustedDeviceItem::WorkgroupMemoryPublish => Terminal::WorkgroupMemoryPublish,
        TrustedDeviceItem::PrivateMemoryLoad => Terminal::PrivateMemoryLoad,
        TrustedDeviceItem::PrivateMemoryExclusiveLoad => Terminal::PrivateMemoryExclusiveLoad,
        TrustedDeviceItem::PrivateMemoryExclusiveStore => Terminal::PrivateMemoryExclusiveStore,
        TrustedDeviceItem::PrivateMemoryDisjointStore => Terminal::PrivateMemoryDisjointStore,
        TrustedDeviceItem::WorkgroupMemoryLoad => Terminal::WorkgroupMemoryLoad,
        TrustedDeviceItem::WorkgroupMemoryExclusiveLoad => Terminal::WorkgroupMemoryExclusiveLoad,
        TrustedDeviceItem::WorkgroupMemoryExclusiveStore => Terminal::WorkgroupMemoryExclusiveStore,
        TrustedDeviceItem::WorkgroupMemoryDisjointStore => Terminal::WorkgroupMemoryDisjointStore,
        TrustedDeviceItem::CapabilityGlobalBindExclusiveReadWrite => {
            Terminal::GlobalBindExclusiveReadWrite
        }
        TrustedDeviceItem::CapabilityGlobalExclusiveLoad => Terminal::GlobalExclusiveLoad,
        TrustedDeviceItem::CapabilityGlobalExclusiveStore => Terminal::GlobalExclusiveStore,
        TrustedDeviceItem::CapabilityGlobalStoreBlock => Terminal::GlobalStoreBlock,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductionTerminalExpansionV1 {
    KernelContextIssue,
    CapabilityGlobalBindReadOnly,
    CapabilityGlobalBindDisjointWrite,
    CapabilityGlobalLoad,
    CapabilityGlobalStore,
    Execution(ProductionExecutionTerminalV1),
    ThreadIndex(SemanticAxisV1),
    WorkgroupIndex(SemanticAxisV1),
    WorkgroupDimension(SemanticAxisV1),
    GridDimension(SemanticAxisV1),
    ThreadIndex1d,
    Invocation3DIndex1D,
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
        TrustedDeviceItem::Invocation3DCurrent
            | TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic
            | TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic
    )
}

impl ProductionSemanticTerminalRuleV1 {
    pub(crate) const fn from_trusted_device_item(item: TrustedDeviceItem) -> Self {
        if let Some(terminal) = execution_terminal_contract_v1(item) {
            return Self::Expand(ProductionTerminalExpansionV1::Execution(terminal));
        }
        match item {
            TrustedDeviceItem::KernelContextIssue => {
                Self::Expand(ProductionTerminalExpansionV1::KernelContextIssue)
            }
            TrustedDeviceItem::CapabilityGlobalBindReadOnly => {
                Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalBindReadOnly)
            }
            TrustedDeviceItem::CapabilityGlobalBindDisjointWrite => {
                Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalBindDisjointWrite)
            }
            TrustedDeviceItem::CapabilityGlobalLoad => {
                Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalLoad)
            }
            TrustedDeviceItem::CapabilityGlobalStore => {
                Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalStore)
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
            TrustedDeviceItem::Invocation3DIndex1D => {
                Self::Expand(ProductionTerminalExpansionV1::Invocation3DIndex1D)
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
            Self::Expand(ProductionTerminalExpansionV1::KernelContextIssue) => {
                TrustedDeviceItem::KernelContextIssue
            }
            Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalBindReadOnly) => {
                TrustedDeviceItem::CapabilityGlobalBindReadOnly
            }
            Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalBindDisjointWrite) => {
                TrustedDeviceItem::CapabilityGlobalBindDisjointWrite
            }
            Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalLoad) => {
                TrustedDeviceItem::CapabilityGlobalLoad
            }
            Self::Expand(ProductionTerminalExpansionV1::CapabilityGlobalStore) => {
                TrustedDeviceItem::CapabilityGlobalStore
            }
            Self::Expand(ProductionTerminalExpansionV1::Execution(terminal)) => {
                terminal.trusted_device_item()
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
            Self::Expand(ProductionTerminalExpansionV1::Invocation3DIndex1D) => {
                TrustedDeviceItem::Invocation3DIndex1D
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
            Self::Expand(ProductionTerminalExpansionV1::MemoryVolatileLoad) => {
                TrustedDeviceItem::MemoryVolatileLoad
            }
            Self::Expand(
                ProductionTerminalExpansionV1::ColdPath
                | ProductionTerminalExpansionV1::RustcFabsF32,
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
mod tests {
    use super::*;

    #[test]
    fn execution_terminal_roster_is_closed_unique_and_fully_expanded() {
        let mut tags = std::collections::BTreeSet::new();
        let mut items = Vec::new();
        for terminal in ProductionExecutionTerminalV1::ALL {
            assert!(tags.insert(terminal.identity_tag()));
            let item = terminal.trusted_device_item();
            assert!(!items.contains(&item));
            items.push(item);
            assert_eq!(execution_terminal_contract_v1(item), Some(terminal));
            assert_eq!(
                ProductionSemanticTerminalRuleV1::from_trusted_device_item(item),
                ProductionSemanticTerminalRuleV1::Expand(ProductionTerminalExpansionV1::Execution(
                    terminal
                ),),
            );
            assert!((1..=4).contains(&terminal.source_argument_count()));
        }
        assert_eq!(
            tags.into_iter().collect::<Vec<_>>(),
            (0_u8..42).collect::<Vec<_>>()
        );
    }

    #[test]
    fn typed_global_contract_is_closed_and_has_production_expansions() {
        let cases = [
            (
                TrustedDeviceItem::CapabilityGlobalBindReadOnly,
                ProductionCapabilityMemoryTerminalV1::BindReadOnly,
                2,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalBindDisjointWrite,
                ProductionCapabilityMemoryTerminalV1::BindDisjointWrite,
                2,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalBindExclusiveReadWrite,
                ProductionCapabilityMemoryTerminalV1::BindExclusiveReadWrite,
                2,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalLoad,
                ProductionCapabilityMemoryTerminalV1::Load,
                2,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalStore,
                ProductionCapabilityMemoryTerminalV1::Store,
                3,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalExclusiveLoad,
                ProductionCapabilityMemoryTerminalV1::ExclusiveLoad,
                2,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalExclusiveStore,
                ProductionCapabilityMemoryTerminalV1::ExclusiveStore,
                3,
            ),
            (
                TrustedDeviceItem::CapabilityGlobalStoreBlock,
                ProductionCapabilityMemoryTerminalV1::StoreBlock,
                4,
            ),
        ];
        for (item, expected, arity) in cases {
            let contract = capability_memory_terminal_contract_v1(item).unwrap();
            assert_eq!(contract, expected);
            assert_eq!(contract.source_argument_count(), arity);
            let ProductionSemanticTerminalRuleV1::Expand(expansion) =
                ProductionSemanticTerminalRuleV1::from_trusted_device_item(item)
            else {
                panic!("typed global operation remained fail-closed")
            };
            assert_eq!(
                expansion,
                match expected {
                    ProductionCapabilityMemoryTerminalV1::BindReadOnly => {
                        ProductionTerminalExpansionV1::CapabilityGlobalBindReadOnly
                    }
                    ProductionCapabilityMemoryTerminalV1::BindExclusiveReadWrite => {
                        ProductionTerminalExpansionV1::Execution(
                            ProductionExecutionTerminalV1::GlobalBindExclusiveReadWrite,
                        )
                    }
                    ProductionCapabilityMemoryTerminalV1::BindDisjointWrite => {
                        ProductionTerminalExpansionV1::CapabilityGlobalBindDisjointWrite
                    }
                    ProductionCapabilityMemoryTerminalV1::Load => {
                        ProductionTerminalExpansionV1::CapabilityGlobalLoad
                    }
                    ProductionCapabilityMemoryTerminalV1::ExclusiveLoad => {
                        ProductionTerminalExpansionV1::Execution(
                            ProductionExecutionTerminalV1::GlobalExclusiveLoad,
                        )
                    }
                    ProductionCapabilityMemoryTerminalV1::Store => {
                        ProductionTerminalExpansionV1::CapabilityGlobalStore
                    }
                    ProductionCapabilityMemoryTerminalV1::ExclusiveStore => {
                        ProductionTerminalExpansionV1::Execution(
                            ProductionExecutionTerminalV1::GlobalExclusiveStore,
                        )
                    }
                    ProductionCapabilityMemoryTerminalV1::StoreBlock => {
                        ProductionTerminalExpansionV1::Execution(
                            ProductionExecutionTerminalV1::GlobalStoreBlock,
                        )
                    }
                }
            );
        }
    }

    #[test]
    fn fill_terminals_have_explicit_workload_neutral_expansions() {
        let cases = [
            (
                TrustedDeviceItem::KernelContextIssue,
                ProductionTerminalExpansionV1::KernelContextIssue,
            ),
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
                TrustedDeviceItem::Invocation3DIndex1D,
                ProductionTerminalExpansionV1::Invocation3DIndex1D,
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
                TrustedDeviceItem::DisjointBlockComponentIndex,
                ProductionTerminalExpansionV1::DisjointBlockComponentIndex,
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
                TrustedDeviceItem::WorkgroupCollectivesCurrent,
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
            ),
            (
                TrustedDeviceItem::WorkgroupReduceSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
            ),
            (
                TrustedDeviceItem::WorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
            ),
            (
                TrustedDeviceItem::WorkgroupExclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
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
            (
                TrustedDeviceItem::MemoryVolatileLoad,
                ProductionTerminalExpansionV1::MemoryVolatileLoad,
            ),
            (
                TrustedDeviceItem::WorkgroupLdsScopeCurrent,
                ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent,
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
        assert!(
            !is_traversed_reviewed_helper_v1(TrustedDeviceItem::KernelContextIssue),
            "kernel context issuance must remain an authenticated semantic terminal",
        );
        for item in [
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
