// This classifies the archived source call before querying a defined-body
// instance. Non-body callables do not acquire a fabricated invocation or result.
fn source_helper_defined_callee_v34(
    callable: Option<&SemanticCallableDeclV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SemanticFunctionIdV1> {
    budget.charge_work(1)?;
    match callable {
        Some(SemanticCallableDeclV1::Defined { function }) => Ok(*function),
        Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => {
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                source_helper_intrinsic_refusal_v34(operation),
            ))
        }
        Some(SemanticCallableDeclV1::DeviceFfiImport { .. }) => {
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper DeviceFfiImport computation is not interpreted",
            ))
        }
        None => Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source helper original callable is absent",
        )),
    }
}

fn source_helper_intrinsic_refusal_v34(
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> &'static str {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    macro_rules! name {
        ($variant:ident) => {
            concat!(
                "source helper compiler intrinsic ",
                stringify!($variant),
                " is not interpreted"
            )
        };
    }
    match operation {
        Op::Execution(..) => name!(Execution),
        Op::Gfx942InlineU32(..) => name!(Gfx942InlineU32),
        Op::Gfx942OrderedRegion(..) => name!(Gfx942OrderedRegion),
        Op::Gfx942OrderedProgram(..) => name!(Gfx942OrderedProgram),
        Op::Gfx942CompleteBody(..) => name!(Gfx942CompleteBody),
        Op::Gfx942PhysicalEntryBegin => name!(Gfx942PhysicalEntryBegin),
        Op::Gfx942PhysicalEntryLabel(..) => name!(Gfx942PhysicalEntryLabel),
        Op::Gfx942PhysicalEntryStep(..) => name!(Gfx942PhysicalEntryStep),
        Op::Gfx942PhysicalGlobalCopyBegin => name!(Gfx942PhysicalGlobalCopyBegin),
        Op::Gfx942PhysicalGlobalCopyLabel(..) => name!(Gfx942PhysicalGlobalCopyLabel),
        Op::Gfx942PhysicalGlobalCopyStep(..) => name!(Gfx942PhysicalGlobalCopyStep),
        Op::Gfx942PhysicalLdsExchangeBegin(..) => name!(Gfx942PhysicalLdsExchangeBegin),
        Op::Gfx942PhysicalLdsExchangeLabel(..) => name!(Gfx942PhysicalLdsExchangeLabel),
        Op::Gfx942PhysicalLdsExchangeStep(..) => name!(Gfx942PhysicalLdsExchangeStep),
        Op::ThreadIndex(..) => name!(ThreadIndex),
        Op::WorkgroupIndex(..) => name!(WorkgroupIndex),
        Op::WorkgroupDimension(..) => name!(WorkgroupDimension),
        Op::GridDimension(..) => name!(GridDimension),
        Op::Trap => name!(Trap),
        Op::WorkgroupLdsScopeCurrent { .. } => name!(WorkgroupLdsScopeCurrent),
        Op::DynamicLdsExactCurrent { .. } => name!(DynamicLdsExactCurrent),
        Op::DynamicLdsIntoCollectiveRawParts { .. } => name!(DynamicLdsIntoCollectiveRawParts),
        Op::WorkgroupPipelineCreate { .. } => name!(WorkgroupPipelineCreate),
        Op::WorkgroupPipelineEvent { .. } => name!(WorkgroupPipelineEvent),
        Op::WorkgroupPipelineWrite { .. } => name!(WorkgroupPipelineWrite),
        Op::WorkgroupPipelineRead { .. } => name!(WorkgroupPipelineRead),
        Op::WorkgroupBarrier => name!(WorkgroupBarrier),
        Op::WaveBarrier => name!(WaveBarrier),
        Op::FabsF32 => name!(FabsF32),
        Op::SaturatingInteger(..) => name!(SaturatingInteger),
        Op::Gfx942Wave64ShuffleIndex { .. } => name!(Gfx942Wave64ShuffleIndex),
        Op::MemoryVolatileLoad { .. } => name!(MemoryVolatileLoad),
        Op::MathContextCurrent { .. } => name!(MathContextCurrent),
        Op::MathF32 { .. } => name!(MathF32),
        Op::Bf16Conversion { .. } => name!(Bf16Conversion),
        Op::CollectiveContextCurrent { .. } => name!(CollectiveContextCurrent),
        Op::WorkgroupReduceSum { .. } => name!(WorkgroupReduceSum),
        Op::NeutralWorkgroupReduceSum { .. } => name!(NeutralWorkgroupReduceSum),
        Op::NeutralWorkgroupScanSum { .. } => name!(NeutralWorkgroupScanSum),
        Op::SubgroupReduceF32 { .. } => name!(SubgroupReduceF32),
        Op::Gfx950SubgroupContextCurrent { .. } => name!(Gfx950SubgroupContextCurrent),
        Op::Gfx950SubgroupReduceF32 { .. } => name!(Gfx950SubgroupReduceF32),
        Op::SubgroupBroadcastF32 { .. } => name!(SubgroupBroadcastF32),
        Op::MatrixContextCurrent { .. } => name!(MatrixContextCurrent),
        Op::WaveLaneCurrent { .. } => name!(WaveLaneCurrent),
        Op::Bf16MatrixViewRowMajor { .. } => name!(Bf16MatrixViewRowMajor),
        Op::Bf16MatrixViewColumnMajor { .. } => name!(Bf16MatrixViewColumnMajor),
        Op::Bf16MatrixLoad { .. } => name!(Bf16MatrixLoad),
        Op::Bf16MatrixLoadZeroFilledV2 { .. } => name!(Bf16MatrixLoadZeroFilledV2),
        Op::Gfx950Fp4MatrixViewRowMajor { .. } => name!(Gfx950Fp4MatrixViewRowMajor),
        Op::Gfx950Fp4MatrixLoadM16K128 { .. } => name!(Gfx950Fp4MatrixLoadM16K128),
        Op::Gfx950Fp8MatrixViewRowMajor { .. } => name!(Gfx950Fp8MatrixViewRowMajor),
        Op::Gfx950Fp8MatrixLoadM16K128 { .. } => name!(Gfx950Fp8MatrixLoadM16K128),
        Op::Gfx950LdsTransposeCurrent { .. } => name!(Gfx950LdsTransposeCurrent),
        Op::Gfx950LdsTransposeStage { .. } => name!(Gfx950LdsTransposeStage),
        Op::Gfx950LdsTransposePublish { .. } => name!(Gfx950LdsTransposePublish),
        Op::Gfx950LdsTransposeRead { .. } => name!(Gfx950LdsTransposeRead),
        Op::StridedReadView2DFromSharedSlice { .. } => name!(StridedReadView2DFromSharedSlice),
        Op::StridedReadView2DLoadOr { .. } => name!(StridedReadView2DLoadOr),
        Op::F32MatrixAccumulatorZero { .. } => name!(F32MatrixAccumulatorZero),
        Op::F32MatrixAccumulatorIntoValues { .. } => name!(F32MatrixAccumulatorIntoValues),
        Op::MatrixMultiplyAccumulate { .. } => name!(MatrixMultiplyAccumulate),
        Op::ThreadIndex1d { .. } => name!(ThreadIndex1d),
        Op::ThreadIndexGet { .. } => name!(ThreadIndexGet),
        Op::ThreadIndexIntoDisjoint { .. } => name!(ThreadIndexIntoDisjoint),
        Op::ThreadIndexCheckedShift { .. } => name!(ThreadIndexCheckedShift),
        Op::ThreadIndexCheckedBlock { .. } => name!(ThreadIndexCheckedBlock),
        Op::ThreadIndexCheckedTiled2d { .. } => name!(ThreadIndexCheckedTiled2d),
        Op::ThreadIndexCheckedRowStriped2d { .. } => name!(ThreadIndexCheckedRowStriped2d),
        Op::DisjointIndexGet { .. } => name!(DisjointIndexGet),
        Op::DisjointBlockComponentIndex { .. } => name!(DisjointBlockComponentIndex),
        Op::DisjointIndexCheckedShift { .. } => name!(DisjointIndexCheckedShift),
        Op::DisjointSliceLen { .. } => name!(DisjointSliceLen),
        Op::WriteOnlyDisjointSliceLen { .. } => name!(WriteOnlyDisjointSliceLen),
        Op::DisjointSliceGetMut { .. } => name!(DisjointSliceGetMut),
        Op::DisjointSliceGetDisjointMut { .. } => name!(DisjointSliceGetDisjointMut),
        Op::GridLeaderCurrent { .. } => name!(GridLeaderCurrent),
        Op::DisjointSliceGetMutExclusive { .. } => name!(DisjointSliceGetMutExclusive),
        Op::DisjointSliceGetBlockMut { .. } => name!(DisjointSliceGetBlockMut),
        Op::DisjointSliceGetTiled2dMut { .. } => name!(DisjointSliceGetTiled2dMut),
        Op::DisjointSliceGetRowStriped2dMut { .. } => name!(DisjointSliceGetRowStriped2dMut),
        Op::WriteOnlyDisjointSliceWrite { .. } => name!(WriteOnlyDisjointSliceWrite),
        Op::ColdPath => name!(ColdPath),
    }
}

impl OriginalEntryIndexV20<'_, '_> {
    fn helper_for_call_v34(
        &self,
        root: usize,
        parent: usize,
        block: u32,
        call: &SemanticDirectCallV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<OriginalHelperCallV33> {
        let semantic = self.source.source.source_semantic(budget)?;
        let function = source_helper_defined_callee_v34(
            semantic.callables().get(call.callee().index() as usize),
            budget,
        )?;
        let child = self.helper_child_v33(root, parent, block, budget)?;
        budget.charge_work(1)?;
        if child.function != function {
            return self
                .source
                .source
                .missing("source helper defined child names another callable");
        }
        Ok(child)
    }
}
