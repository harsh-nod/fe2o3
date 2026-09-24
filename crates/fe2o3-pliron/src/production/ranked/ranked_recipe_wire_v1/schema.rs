use super::*;

enum_wire!(AccessKindAttr; 1 => Read, 2 => Write, 3 => AtomicRead, 4 => AtomicWrite, 5 => AtomicReadModifyWrite);
enum_wire!(MemorySpaceAttr; 1 => Private, 2 => Workgroup, 3 => Global);
enum_wire!(AtomicOrderingAttr; 1 => Relaxed, 2 => Acquire, 3 => Release, 4 => AcquireRelease, 5 => SequentiallyConsistent);
enum_wire!(AtomicScopeAttr; 1 => SingleThread, 2 => Workgroup, 3 => Agent, 4 => Device, 5 => System);
enum_wire!(OwnershipCoverageAttr; 1 => ExactView, 2 => ExactEffectDomain, 3 => TotalView, 4 => CollectiveContributions);
enum_wire!(OwnershipPartitionAttr; 1 => ExactSets, 2 => DenseRectangles);
enum_wire!(HierarchyAttr; 1 => Grid, 2 => Workgroup, 3 => Subgroup, 4 => Lane);
enum_wire!(MemoryScopeAttr; 1 => Subgroup, 2 => Workgroup, 3 => Device, 4 => System);
enum_wire!(AddressSpaceAttr; 1 => Private, 2 => Workgroup, 3 => Global, 4 => Constant, 5 => Generic);
enum_wire!(MemoryOrderAttr; 1 => Acquire, 2 => Release, 3 => AcquireRelease, 4 => SequentiallyConsistent);
enum_wire!(IndexBinaryKindAttr; 1 => Add, 2 => Multiply, 3 => Remainder, 4 => Divide);
enum_wire!(SemanticBinaryKindAttr; 1 => Add, 2 => Multiply);
enum_wire!(PipelineEventKindAttr; 1 => Stage, 2 => Commit, 3 => Wait, 4 => Consume, 5 => Discard, 6 => Release);
enum_wire!(TensorConvergenceAttr; 1 => UniformSubgroup, 2 => Divergent, 3 => UniformWorkgroup, 4 => Opaque);
enum_wire!(SemanticEvaluationOrderAttr; 1 => Ascending, 2 => Descending, 3 => Lexicographic, 4 => Explicit);
enum_wire!(SemanticCoverageBindingAttr; 1 => TotalView, 2 => CollectiveContributions);
enum_wire!(ProductionCollectiveSemanticKindV1; 1 => FiniteFold, 2 => FiniteRecurrence, 3 => PermutationGather);
enum_wire!(SafeReferenceKindV2; 1 => SourceAndMir, 2 => Mir);
enum_wire!(ProductionSemanticScalarTypeV2; 1 => Bool, 2 => Integer { signed: bool, bits: u16 }, 3 => Float { bits: u16 });
enum_wire!(ProductionIeeeRoundingModeV2; 1 => NearestTiesToEven, 2 => TowardZero, 3 => TowardPositive, 4 => TowardNegative);
enum_wire!(ProductionIeeeExceptionalValuePolicyV2; 1 => PreserveExactBits, 2 => CanonicalNan);
enum_wire!(ProductionNumericalContractV2;
    1 => ExactBitVectorOperatorCongruence,
    2 => ExactIeee754OperatorCongruence { rounding: ProductionIeeeRoundingModeV2, exceptional_values: ProductionIeeeExceptionalValuePolicyV2 },
    3 => Relaxed,
    4 => ErrorBounded { absolute_error_f64_bits: u64, relative_error_f64_bits: u64 },
);
enum_wire!(ProductionOverflowContractV2; 1 => Wrapping, 2 => Checked);
enum_wire!(ProductionSemanticUnaryOpV2; 1 => Not, 2 => Negate);
enum_wire!(ProductionSemanticBinaryOpV2; 1 => Add, 2 => Subtract, 3 => Multiply, 4 => Divide, 5 => Remainder, 6 => BitXor, 7 => BitAnd, 8 => BitOr, 9 => ShiftLeft, 10 => ShiftRight);
enum_wire!(ProductionSemanticComparisonV2; 1 => Equal, 2 => LessThan, 3 => LessOrEqual, 4 => NotEqual, 5 => GreaterOrEqual, 6 => GreaterThan);
enum_wire!(ProductionSemanticCastV2; 1 => Integer, 2 => IntegerToFloat, 3 => FloatToFloat, 4 => FloatToIntegerSaturating);

impl Wire for ProductionRankedValueIdV1 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.get().emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        Ok(Self::new(u32::read(input)?))
    }
}
impl Wire for ProductionRankedValueV1 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        match self {
            Self::Argument(value) => {
                1u8.emit(out)?;
                value.emit(out)
            }
            Self::BlockArgument { block, argument } => {
                2u8.emit(out)?;
                block.emit(out)?;
                argument.emit(out)
            }
            Self::Local(value) => {
                3u8.emit(out)?;
                value.emit(out)
            }
        }
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        match u8::read(input)? {
            1 => Ok(Self::Argument(u32::read(input)?)),
            2 => Ok(Self::BlockArgument {
                block: u32::read(input)?,
                argument: u32::read(input)?,
            }),
            3 => Ok(Self::Local(ProductionRankedValueIdV1::read(input)?)),
            tag => Err(WireError::UnknownTag {
                field: "ranked value",
                tag,
            }
            .into()),
        }
    }
}

type Value = ProductionRankedValueV1;
type Id = ProductionRankedValueIdV1;
type Proof = ProductionReferenceProofV2;
type Subjects = FunctionalRefinementSubjectsV2;

enum_wire!(ProductionRankedOperationV1;
    1 => ExecutionLayout { grid_identity: u64, global_extents: [u64; 3], workgroup_extents: [u64; 3], subgroup_size: u64, full_physical_workgroups: bool },
    2 => View { result: Id, element_width: u32, writable: bool, shape: Vec<u64> => MAX_RANKED_MEMORY_RANK, dynamic_extents: Vec<Value> => MAX_RANKED_MEMORY_RANK, allocation_origin: u64, noalias_class: u64 },
    3 => ViewInSpace { result: Id, element_width: u32, writable: bool, shape: Vec<u64> => MAX_RANKED_MEMORY_RANK, dynamic_extents: Vec<Value> => MAX_RANKED_MEMORY_RANK, memory_space: MemorySpaceAttr, allocation_origin: u64, noalias_class: u64 },
    4 => PipelineCreate { result: Id, view: Value, buffers: u32, prefetch_distance: u32 },
    5 => PipelineEvent { pipeline: Value, epoch: Value, slot: Value, kind: PipelineEventKindAttr },
    6 => IndexConstant { result: Id, value: u64 },
    7 => IndexUnsignedCast { result: Id, source: Value, bit_width: u16 },
    8 => IndexUnknown { result: Id },
    9 => InvocationIndex { result: Id, dimension: u32, launch_extent: u64 },
    10 => IndexBinary { result: Id, kind: IndexBinaryKindAttr, lhs: Value, rhs: Value },
    11 => DeterministicJoin { result: Id, dependencies: Vec<Value> => MAX_DETERMINISTIC_JOIN_INPUTS_V1 },
    12 => CheckedTiledIndex2D { result: Id, invocation: Value, component: Value, rows: Value, columns: Value, row_stride: Value, lanes_per_tile: u64, tile_rows: u64, tile_columns: u64, elements_per_lane: u64 },
    13 => CheckedRowStripedIndex2D { result: Id, invocation: Value, component: Value, rows: Value, columns: Value, row_stride: Value, lanes_per_row: u64, elements_per_lane: u64 },
    14 => PredicatedCheckedTiledIndex2D { result: Id, success: Id, invocation: Value, component: Value, rows: Value, columns: Value, row_stride: Value, physical_extent: Value, lanes_per_tile: u64, tile_rows: u64, tile_columns: u64, elements_per_lane: u64 },
    15 => PredicatedCheckedRowStripedIndex2D { result: Id, success: Id, invocation: Value, component: Value, rows: Value, columns: Value, row_stride: Value, physical_extent: Value, lanes_per_row: u64, elements_per_lane: u64 },
    16 => Dimension { result: Id, view: Value, dimension: u32 },
    17 => Access { kind: AccessKindAttr, view: Value, indices: Vec<Value> => MAX_RANKED_MEMORY_RANK },
    18 => PredicatedAccess { kind: AccessKindAttr, view: Value, index: Value, success: Value },
    19 => ValueAccess { kind: AccessKindAttr, view: Value, indices: Vec<Value> => MAX_RANKED_MEMORY_RANK, value: Value },
    20 => AtomicAccess { kind: AccessKindAttr, ordering: AtomicOrderingAttr, scope: AtomicScopeAttr, view: Value, indices: Vec<Value> => MAX_RANKED_MEMORY_RANK },
    21 => AtomicValueAccess { kind: AccessKindAttr, ordering: AtomicOrderingAttr, scope: AtomicScopeAttr, view: Value, indices: Vec<Value> => MAX_RANKED_MEMORY_RANK, value: Value },
    22 => OwnershipContract { view: Value, coverage: OwnershipCoverageAttr, partition: OwnershipPartitionAttr },
    23 => AllocationEffect { kind: AccessKindAttr, memory_space: MemorySpaceAttr, allocation_origin: u64, noalias_class: u64 },
    24 => Barrier { execution_scope: HierarchyAttr, memory_scope: MemoryScopeAttr, address_space: AddressSpaceAttr, order: MemoryOrderAttr },
    25 => Fence { memory_scope: MemoryScopeAttr, address_space: AddressSpaceAttr, order: MemoryOrderAttr },
    26 => TensorLayout { contract: TensorLayoutContractV1, convergence: TensorConvergenceAttr, active_lanes: u32, binding: Option<ProductionCooperativeTensorBindingV1> },
    27 => TensorResultComponent { result: Id, tensor_result_root: DigestV1, component: u16, scalar: ProductionSemanticScalarTypeV2, numerical_contract: ProductionNumericalContractV2 },
    28 => SemanticSymbol { result: Id, symbol: u32 },
    29 => SemanticConstant { result: Id, value: u64 },
    30 => SemanticBinary { result: Id, kind: SemanticBinaryKindAttr, lhs: Value, rhs: Value },
    31 => SemanticExpression { result: Id, expression: ProductionSemanticExpressionV2, numerical_contract: ProductionNumericalContractV2 },
    32 => CollectiveSemantics { contract: ProductionCollectiveSemanticContractV1, view: Value, actual: Value, expected: Value, witness0: Value, witness1: Value },
    33 => RequireEquivalent { actual: Value, expected: Value },
    34 => RequireAuthenticatedReferenceEquivalent { actual: Value, expected: Value, proof: Proof },
    35 => RequestAuthenticatedReferenceEquivalent { actual: Value, expected: Value, subjects: Subjects },
    36 => RequireEffectRefinement { contract: ProductionEffectRefinementContractV2, proof: Proof },
    37 => RequestEffectRefinement { contract: ProductionEffectRefinementContractV2, subjects: Subjects },
    38 => RequireNumericalRefinement { contract: ProductionNumericalRefinementContractV2, proof: Proof },
    39 => RequestNumericalRefinement { contract: ProductionNumericalRefinementContractV2, subjects: Subjects },
    40 => RequireTensorRefinement { contract: ProductionTensorRefinementContractV1, proof: Proof },
    41 => RequestTensorRefinement { contract: ProductionTensorRefinementContractV1, subjects: Subjects },
);

enum_wire!(ProductionRankedTerminatorV1;
    1 => IndexLessThan { lhs: Value, rhs: Value, true_block: u32, false_block: u32 },
    2 => IndexLessThanArgs { lhs: Value, rhs: Value, true_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, false_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, true_block: u32, false_block: u32 },
    3 => IndexEqual { lhs: Value, rhs: Value, true_block: u32, false_block: u32 },
    4 => IndexEqualArgs { lhs: Value, rhs: Value, true_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, false_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, true_block: u32, false_block: u32 },
    5 => AnalysisSplit { control_dependencies: Vec<Value>, first_block: u32, second_block: u32 },
    6 => AnalysisSplitArgs { control_dependencies: Vec<Value>, first_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, second_arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, first_block: u32, second_block: u32 },
    7 => Branch { target: u32 },
    8 => BranchArgs { arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, target: u32 },
    9 => BranchArgsAdd { value: Value, step: Value, target: u32 },
    10 => BranchArgsAddAt { arguments: Vec<Value> => HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, add_argument: u32, step: Value, target: u32 },
    11 => Return,
    12 => Trap,
);
