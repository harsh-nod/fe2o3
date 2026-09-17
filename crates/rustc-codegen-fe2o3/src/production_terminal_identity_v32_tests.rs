use super::{ProductionBf16ConversionV1 as Bf16, ProductionTerminalExpansionV1 as E};
use fe2o3_kernel_ir::F32MathFunction as Math;
use fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1 as Axis;
use std::collections::BTreeMap;

pub(crate) fn assert_current_tags(tag: impl Fn(E) -> u8) {
    macro_rules! units {
        ($($unit:ident),+ $(,)?) => {{
            // This exhaustive match makes a newly added variant require an
            // explicit entry in the independently checked identity roster.
            let exhaustive = |value| match value {
                E::ThreadIndex(_) | E::WorkgroupIndex(_) | E::WorkgroupDimension(_)
                    | E::GridDimension(_) | E::MathF32(_) | E::Bf16Conversion(_)
                    $(| E::$unit)+ => (),
            };
            let values = vec![$(E::$unit),+];
            for value in &values { exhaustive(*value); }
            values
        }};
    }
    let mut values = units![
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
        RustcFabsF32,
        MemoryVolatileLoad,
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
        Trap,
        Realtime64,
        ColdPath,
    ];
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        values.extend([
            E::ThreadIndex(axis),
            E::WorkgroupIndex(axis),
            E::WorkgroupDimension(axis),
            E::GridDimension(axis),
        ]);
    }
    values.extend(
        [
            Math::Sqrt,
            Math::FusedMultiplyAdd,
            Math::Floor,
            Math::Ceil,
            Math::Truncate,
            Math::RoundTiesEven,
            Math::Sin,
            Math::Cos,
            Math::Exp,
            Math::Exp2,
            Math::Ln,
            Math::Log2,
            Math::Log10,
            Math::Abs,
        ]
        .map(E::MathF32),
    );
    values.extend(
        [
            Bf16::FromBits,
            Bf16::ToBits,
            Bf16::FromF32RoundTiesEven,
            Bf16::ToF32,
        ]
        .map(E::Bf16Conversion),
    );
    let mut tags = BTreeMap::new();
    for value in values {
        let number = tag(value);
        assert!(
            tags.insert(number, value).is_none(),
            "duplicate terminal tag {number}: {value:?}"
        );
    }
    assert_eq!(tag(E::Realtime64), 122);
    assert_eq!(tag(E::WorkgroupCollectiveContextCurrent), 111);
    assert_eq!(tag(E::Bf16MatrixBColumnMajorLoadZeroFilledV1), 121);
}
