#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum IndexExpr {
    Constant(u64),
    Dimension { view: Value, dimension: usize },
    Value(Value),
}

impl IndexExpr {
    fn describe(self, context: &Context) -> String {
        match self {
            Self::Constant(value) => value.to_string(),
            Self::Dimension { view, dimension } => {
                use std::fmt::Write as _;

                let mut text = String::with_capacity(NUMERIC_BOUNDS_DIAGNOSTIC_BYTES_V1);
                write!(text, "{}.dim<{dimension}>()", view.id(context))
                    .expect("writing a numeric diagnostic to String cannot fail");
                text
            }
            Self::Value(value) => value.id(context).into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct LessThanFact {
    lhs: IndexExpr,
    rhs: IndexExpr,
}

#[derive(Clone, Copy)]
struct PredecessorEdge {
    block: usize,
    successor: usize,
    guard_fact: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RankedOperationKind {
    NativeData,
    RankedView,
    IndexConstant,
    IndexUnsignedCast,
    IndexUnknown,
    InvocationIndex,
    IndexBinary,
    DeterministicJoin,
    CheckedTiledIndex2D,
    CheckedRowStripedIndex2D,
    Dimension,
    RankedAccess,
    OwnershipContract,
    AllocationEffect,
    PipelineCreate,
    PipelineEvent,
    IndexLessThanBranch,
    IndexLessThanBranchArgs,
    IndexEqualBranch,
    IndexEqualBranchArgs,
    BooleanBranchArgs,
    AnalysisSplit,
    Branch,
    BranchArgs,
    Return,
    Trap,
    Barrier,
    ExecutionLayout,
    Fence,
    SemanticSymbol,
    SemanticConstant,
    SemanticBinary,
    SemanticExpressionCommitment,
    FiniteFoldContract,
    FiniteRecurrenceContract,
    PermutationGatherContract,
    TypedSemantic,
    RequireEquivalent,
    ProofObligation,
    ProofEvidence,
    RequireRefinement,
    RequireEffectRefinement,
    TensorLayout,
}

impl RankedOperationKind {
    const fn is_terminator(self) -> bool {
        matches!(
            self,
            Self::IndexLessThanBranch
                | Self::IndexLessThanBranchArgs
                | Self::IndexEqualBranch
                | Self::IndexEqualBranchArgs
                | Self::BooleanBranchArgs
                | Self::AnalysisSplit
                | Self::Branch
                | Self::BranchArgs
                | Self::Return
                | Self::Trap
        )
    }
}

fn ranked_operation_kind(operation: &dyn Op) -> Option<RankedOperationKind> {
    if is_native_data_operation_v3(operation) {
        Some(RankedOperationKind::NativeData)
    } else if operation.downcast_ref::<RankedViewOp>().is_some() {
        Some(RankedOperationKind::RankedView)
    } else if operation.downcast_ref::<PipelineCreateOp>().is_some() {
        Some(RankedOperationKind::PipelineCreate)
    } else if operation.downcast_ref::<PipelineEventOp>().is_some() {
        Some(RankedOperationKind::PipelineEvent)
    } else if operation.downcast_ref::<IndexConstantOp>().is_some() {
        Some(RankedOperationKind::IndexConstant)
    } else if operation.downcast_ref::<IndexUnsignedCastOp>().is_some() {
        Some(RankedOperationKind::IndexUnsignedCast)
    } else if operation.downcast_ref::<IndexUnknownOp>().is_some() {
        Some(RankedOperationKind::IndexUnknown)
    } else if operation.downcast_ref::<InvocationIndexOp>().is_some() {
        Some(RankedOperationKind::InvocationIndex)
    } else if operation.downcast_ref::<IndexBinaryOp>().is_some() {
        Some(RankedOperationKind::IndexBinary)
    } else if operation.downcast_ref::<DeterministicJoinOp>().is_some() {
        Some(RankedOperationKind::DeterministicJoin)
    } else if operation.downcast_ref::<CheckedTiledIndex2DOp>().is_some() {
        Some(RankedOperationKind::CheckedTiledIndex2D)
    } else if operation
        .downcast_ref::<CheckedRowStripedIndex2DOp>()
        .is_some()
    {
        Some(RankedOperationKind::CheckedRowStripedIndex2D)
    } else if operation.downcast_ref::<DimensionOp>().is_some() {
        Some(RankedOperationKind::Dimension)
    } else if operation.downcast_ref::<RankedAccessOp>().is_some() {
        Some(RankedOperationKind::RankedAccess)
    } else if operation.downcast_ref::<OwnershipContractOp>().is_some() {
        Some(RankedOperationKind::OwnershipContract)
    } else if operation.downcast_ref::<AllocationEffectOp>().is_some() {
        Some(RankedOperationKind::AllocationEffect)
    } else if operation.downcast_ref::<IndexLessThanBranchOp>().is_some() {
        Some(RankedOperationKind::IndexLessThanBranch)
    } else if operation
        .downcast_ref::<IndexLessThanBranchArgsOp>()
        .is_some()
    {
        Some(RankedOperationKind::IndexLessThanBranchArgs)
    } else if operation.downcast_ref::<IndexEqualBranchOp>().is_some() {
        Some(RankedOperationKind::IndexEqualBranch)
    } else if operation.downcast_ref::<IndexEqualBranchArgsOp>().is_some() {
        Some(RankedOperationKind::IndexEqualBranchArgs)
    } else if operation
        .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
        .is_some()
    {
        Some(RankedOperationKind::BooleanBranchArgs)
    } else if operation.downcast_ref::<AnalysisSplitOp>().is_some() {
        Some(RankedOperationKind::AnalysisSplit)
    } else if operation.downcast_ref::<BranchOp>().is_some() {
        Some(RankedOperationKind::Branch)
    } else if operation.downcast_ref::<BranchArgsOp>().is_some() {
        Some(RankedOperationKind::BranchArgs)
    } else if operation.downcast_ref::<ReturnOp>().is_some() {
        Some(RankedOperationKind::Return)
    } else if operation.downcast_ref::<TrapOp>().is_some() {
        Some(RankedOperationKind::Trap)
    } else if operation.downcast_ref::<BarrierOp>().is_some() {
        Some(RankedOperationKind::Barrier)
    } else if operation.downcast_ref::<ExecutionLayoutOp>().is_some() {
        Some(RankedOperationKind::ExecutionLayout)
    } else if operation.downcast_ref::<FenceOp>().is_some() {
        Some(RankedOperationKind::Fence)
    } else if operation.downcast_ref::<SemanticSymbolOp>().is_some() {
        Some(RankedOperationKind::SemanticSymbol)
    } else if operation.downcast_ref::<SemanticConstantOp>().is_some() {
        Some(RankedOperationKind::SemanticConstant)
    } else if operation.downcast_ref::<SemanticBinaryOp>().is_some() {
        Some(RankedOperationKind::SemanticBinary)
    } else if operation
        .downcast_ref::<SemanticExpressionCommitmentOp>()
        .is_some()
    {
        Some(RankedOperationKind::SemanticExpressionCommitment)
    } else if operation.downcast_ref::<SemanticTypedSymbolOp>().is_some()
        || operation
            .downcast_ref::<TensorResultComponentOp>()
            .is_some()
        || operation
            .downcast_ref::<SemanticTypedConstantOp>()
            .is_some()
        || operation.downcast_ref::<SemanticTypedUnaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedBinaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCompareOp>().is_some()
        || operation.downcast_ref::<SemanticTypedSelectOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCastOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedExpressionRootOp>()
            .is_some()
    {
        Some(RankedOperationKind::TypedSemantic)
    } else if operation.downcast_ref::<RequireEquivalentOp>().is_some() {
        Some(RankedOperationKind::RequireEquivalent)
    } else if operation.downcast_ref::<RequireFiniteFoldOp>().is_some() {
        Some(RankedOperationKind::FiniteFoldContract)
    } else if operation
        .downcast_ref::<RequireFiniteRecurrenceOp>()
        .is_some()
    {
        Some(RankedOperationKind::FiniteRecurrenceContract)
    } else if operation
        .downcast_ref::<RequirePermutationGatherOp>()
        .is_some()
    {
        Some(RankedOperationKind::PermutationGatherContract)
    } else if operation.downcast_ref::<ObligationOp>().is_some() {
        Some(RankedOperationKind::ProofObligation)
    } else if operation.downcast_ref::<EvidenceRefOp>().is_some() {
        Some(RankedOperationKind::ProofEvidence)
    } else if operation.downcast_ref::<RequireRefinementOp>().is_some()
        || operation
            .downcast_ref::<RequireNumericalRefinementOp>()
            .is_some()
        || operation
            .downcast_ref::<RequireTensorRefinementOp>()
            .is_some()
    {
        Some(RankedOperationKind::RequireRefinement)
    } else if operation
        .downcast_ref::<RequireEffectRefinementOp>()
        .is_some()
    {
        Some(RankedOperationKind::RequireEffectRefinement)
    } else if operation.downcast_ref::<TensorLayoutOp>().is_some() {
        Some(RankedOperationKind::TensorLayout)
    } else {
        None
    }
}

// These concrete native families are effect-free; a generic GPU operation,
// preserved leaf, call or memory operation must not enter through this list.
fn is_native_data_operation_v3(operation: &dyn Op) -> bool {
    use dialect_gpu::optimization_v1 as gpu;
    operation.downcast_ref::<gpu::ConstantOp>().is_some()
        || operation.downcast_ref::<gpu::UnaryOp>().is_some()
        || operation.downcast_ref::<gpu::BinaryOp>().is_some()
        || operation.downcast_ref::<gpu::CompareOp>().is_some()
        || operation.downcast_ref::<gpu::CastOp>().is_some()
        || operation.downcast_ref::<gpu::SelectOp>().is_some()
        || operation.downcast_ref::<gpu::SliceLengthOp>().is_some()
        || operation.downcast_ref::<gpu::SliceDataOp>().is_some()
        || operation
            .downcast_ref::<gpu::GetElementPointerOp>()
            .is_some()
}

pub(crate) fn is_production_ranked_operation_v1(operation: &dyn Op) -> bool {
    ranked_operation_kind(operation).is_some()
}

#[derive(Clone, Copy)]
enum RankedBoundsResource {
    Blocks,
    Operations,
    Edges,
    Facts,
    OperationItems,
    Findings,
    StorageItems,
    WorkUnits,
}

impl RankedBoundsResource {
    const fn description(self) -> &'static str {
        match self {
            Self::Blocks => "basic block",
            Self::Operations => "operation",
            Self::Edges => "CFG edge",
            Self::Facts => "guard fact",
            Self::OperationItems => "operation component",
            Self::Findings => "finding",
            Self::StorageItems => "analysis storage item",
            Self::WorkUnits => "analysis work unit",
        }
    }

    const fn limit(self) -> usize {
        match self {
            Self::Blocks => MAX_RANKED_BOUNDS_BLOCKS,
            Self::Operations => MAX_RANKED_BOUNDS_OPERATIONS,
            Self::Edges => MAX_RANKED_BOUNDS_EDGES,
            Self::Facts => MAX_RANKED_BOUNDS_FACTS,
            Self::OperationItems => MAX_RANKED_BOUNDS_OPERATION_ITEMS,
            Self::Findings => MAX_RANKED_BOUNDS_FINDINGS,
            Self::StorageItems => MAX_RANKED_BOUNDS_STORAGE_ITEMS,
            Self::WorkUnits => MAX_RANKED_BOUNDS_WORK_UNITS,
        }
    }
}

#[derive(Default)]
struct RankedBoundsBudget {
    blocks: usize,
    operations: usize,
    edges: usize,
    facts: usize,
    operation_items: usize,
    findings: usize,
    storage_items: usize,
    work_units: usize,
    #[cfg(feature = "internal-bounds-cost-trace")]
    work_trace: RankedBoundsWorkTraceV1,
}

impl RankedBoundsBudget {
    #[cfg_attr(feature = "internal-bounds-cost-trace", track_caller)]
    fn reserve(
        &mut self,
        resource: RankedBoundsResource,
        amount: usize,
    ) -> Result<(), RankedBoundsFindingV1> {
        let current = match resource {
            RankedBoundsResource::Blocks => self.blocks,
            RankedBoundsResource::Operations => self.operations,
            RankedBoundsResource::Edges => self.edges,
            RankedBoundsResource::Facts => self.facts,
            RankedBoundsResource::OperationItems => self.operation_items,
            RankedBoundsResource::Findings => self.findings,
            RankedBoundsResource::StorageItems => self.storage_items,
            RankedBoundsResource::WorkUnits => self.work_units,
        };
        let actual = current.saturating_add(amount);
        #[cfg(feature = "internal-bounds-cost-trace")]
        if matches!(resource, RankedBoundsResource::WorkUnits) {
            self.work_trace.record(
                RankedBoundsWorkSiteV1::caller(core::panic::Location::caller()),
                current,
                amount,
                actual <= resource.limit(),
            );
            if actual > resource.limit() {
                self.work_trace
                    .report_denial(current, amount, actual, resource.limit());
            }
        }
        if actual > resource.limit() {
            return Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: resource.description(),
                limit: resource.limit(),
                actual,
            });
        }
        match resource {
            RankedBoundsResource::Blocks => self.blocks = actual,
            RankedBoundsResource::Operations => self.operations = actual,
            RankedBoundsResource::Edges => self.edges = actual,
            RankedBoundsResource::Facts => self.facts = actual,
            RankedBoundsResource::OperationItems => self.operation_items = actual,
            RankedBoundsResource::Findings => self.findings = actual,
            RankedBoundsResource::StorageItems => self.storage_items = actual,
            RankedBoundsResource::WorkUnits => self.work_units = actual,
        }
        Ok(())
    }

    fn storage(&mut self, amount: usize) -> Result<(), RankedBoundsFindingV1> {
        self.reserve(RankedBoundsResource::StorageItems, amount)
    }

    #[cfg_attr(feature = "internal-bounds-cost-trace", track_caller)]
    fn work(&mut self, amount: usize) -> Result<(), RankedBoundsFindingV1> {
        self.reserve(RankedBoundsResource::WorkUnits, amount)
    }
}

#[cfg(feature = "internal-bounds-cost-trace")]
include!("work_trace_v1.rs");

#[derive(Clone, Debug, Eq, PartialEq)]
struct FactSet {
    words: Vec<u64>,
}

impl FactSet {
    fn empty(fact_count: usize) -> Self {
        Self {
            words: vec![0; fact_count.div_ceil(u64::BITS as usize)],
        }
    }

    fn full(fact_count: usize) -> Self {
        let mut words = vec![u64::MAX; fact_count.div_ceil(u64::BITS as usize)];
        if let Some(last) = words.last_mut() {
            let used = fact_count % u64::BITS as usize;
            if used != 0 {
                *last = (1_u64 << used) - 1;
            }
        }
        Self { words }
    }

    fn insert(&mut self, fact: usize) {
        self.words[fact / u64::BITS as usize] |= 1_u64 << (fact % u64::BITS as usize);
    }

    fn contains(&self, fact: usize) -> bool {
        self.words[fact / u64::BITS as usize] & (1_u64 << (fact % u64::BITS as usize)) != 0
    }

    fn intersect_edge(&mut self, source: &Self, guard_fact: Option<usize>) {
        for (word_index, word) in self.words.iter_mut().enumerate() {
            let mut source_word = source.words[word_index];
            if let Some(fact) = guard_fact
                && fact / u64::BITS as usize == word_index
            {
                source_word |= 1_u64 << (fact % u64::BITS as usize);
            }
            *word &= source_word;
        }
    }
}
