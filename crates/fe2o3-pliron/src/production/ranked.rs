use std::{
    collections::HashMap,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionLayoutOp, FenceOp, HierarchyAttr, MemoryOrderAttr,
    MemoryScopeAttr,
};
use dialect_kernel::{
    AccessKindAttr, AnalysisSplitOp, AtomicOrderingAttr, AtomicScopeAttr, BranchArgsOp, BranchOp,
    CheckedTiledIndex2DOp, DYNAMIC_EXTENT, DeterministicJoinOp, DimensionOp, IndexBinaryKindAttr,
    IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, IndexType, InvocationIndexOp,
    MAX_DETERMINISTIC_JOIN_INPUTS_V1, MAX_RANKED_MEMORY_RANK, MemorySpaceAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, RequireEquivalentOp, ReturnOp, SUPPORTED_ELEMENT_WIDTHS,
    SemanticBinaryKindAttr, SemanticBinaryOp, SemanticConstantOp, SemanticSymbolOp,
    TensorConvergenceAttr, TensorLayoutOp,
};
use fe2o3_kernel_analysis::{
    GeneralPlironKernelCheckErrorV1, MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_OPERATIONS,
    PlironAtomicLegalityReportV1, PlironBarrierReportV1, PlironSemanticRefinementReportV1,
    PlironTensorLayoutReportV1, PlironWorkgroupMemoryReportV1, ProductionPlironPreloweringErrorV1,
    ProductionPlironPreloweringReportV1, RankedBoundsReportV1, RankedRaceReportV1,
    require_production_pliron_checks_before_lowering_v1,
};
use fe2o3_kernel_ir::TensorLayoutContractV1;
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        op_interfaces::{OneRegionInterface, SingleBlockRegionInterface},
        ops::{FuncOp, ModuleOp},
        types::FunctionType,
    },
    context::Ptr,
    identifier::Identifier,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

use super::{
    ConstructedGraphStageV1, KernelChecksVerifiedGraphStageV1, ProductionConstructionKindV1,
    ProductionConstructionV1, ProductionPlironSessionV1, ProductionRootHandleV1,
    ProductionSessionErrorV1, ProductionStageHandleV1, RootIdentityV1,
};
use crate::{
    ContextBuildError, HARD_MAX_SESSION_OPERATION_TREE_ITEMS, NameError, NameKind, OperationHandle,
    OperationHandleError, ProductionSessionLimitsV1, validate_name,
};

pub const HARD_MAX_PRODUCTION_RANKED_ARGUMENTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionRankedValueIdV1(u32);

impl ProductionRankedValueIdV1 {
    pub const fn new(identity: u32) -> Self {
        Self(identity)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionRankedValueV1 {
    Argument(u32),
    BlockArgument { block: u32, argument: u32 },
    Local(ProductionRankedValueIdV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedOperationV1 {
    /// Retained linear invocation-to-scope mapping used by mandatory
    /// concurrency analysis. It carries no launch authority.
    ExecutionLayout {
        grid_identity: u64,
        global_extents: [u64; 3],
        workgroup_extents: [u64; 3],
        subgroup_size: u64,
    },
    View {
        result: ProductionRankedValueIdV1,
        element_width: u32,
        writable: bool,
        shape: Vec<u64>,
        dynamic_extents: Vec<ProductionRankedValueV1>,
        allocation_origin: u64,
        noalias_class: u64,
    },
    ViewInSpace {
        result: ProductionRankedValueIdV1,
        element_width: u32,
        writable: bool,
        shape: Vec<u64>,
        dynamic_extents: Vec<ProductionRankedValueV1>,
        memory_space: MemorySpaceAttr,
        allocation_origin: u64,
        noalias_class: u64,
    },
    IndexConstant {
        result: ProductionRankedValueIdV1,
        value: u64,
    },
    InvocationIndex {
        result: ProductionRankedValueIdV1,
        dimension: u32,
        launch_extent: u64,
    },
    IndexBinary {
        result: ProductionRankedValueIdV1,
        kind: IndexBinaryKindAttr,
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
    },
    /// Abstract result of a source-authenticated total deterministic operation.
    ///
    /// The recipe retains dependencies only. It does not itself authenticate
    /// source semantics or grant compiler, artifact, or launch authority.
    DeterministicJoin {
        result: ProductionRankedValueIdV1,
        dependencies: Vec<ProductionRankedValueV1>,
    },
    CheckedTiledIndex2D {
        result: ProductionRankedValueIdV1,
        invocation: ProductionRankedValueV1,
        component: ProductionRankedValueV1,
        rows: ProductionRankedValueV1,
        columns: ProductionRankedValueV1,
        row_stride: ProductionRankedValueV1,
        lanes_per_tile: u64,
        tile_rows: u64,
        tile_columns: u64,
        elements_per_lane: u64,
    },
    Dimension {
        result: ProductionRankedValueIdV1,
        view: ProductionRankedValueV1,
        dimension: u32,
    },
    Access {
        kind: AccessKindAttr,
        view: ProductionRankedValueV1,
        indices: Vec<ProductionRankedValueV1>,
    },
    AtomicAccess {
        kind: AccessKindAttr,
        ordering: AtomicOrderingAttr,
        scope: AtomicScopeAttr,
        view: ProductionRankedValueV1,
        indices: Vec<ProductionRankedValueV1>,
    },
    Barrier {
        execution_scope: HierarchyAttr,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    Fence {
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    /// One tensor-instruction site, not a free-standing proof annotation.
    ///
    /// The source projector must derive this declaration from an authenticated
    /// semantic terminal and its dominating operand producers. Merely adding
    /// this recipe operation never grants source-refinement or artifact authority.
    TensorLayout {
        contract: TensorLayoutContractV1,
        convergence: TensorConvergenceAttr,
        active_lanes: u32,
    },
    SemanticSymbol {
        result: ProductionRankedValueIdV1,
        symbol: u32,
    },
    SemanticConstant {
        result: ProductionRankedValueIdV1,
        value: u64,
    },
    SemanticBinary {
        result: ProductionRankedValueIdV1,
        kind: SemanticBinaryKindAttr,
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
    },
    RequireEquivalent {
        actual: ProductionRankedValueV1,
        expected: ProductionRankedValueV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedTerminatorV1 {
    IndexLessThan {
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        true_block: u32,
        false_block: u32,
    },
    IndexLessThanArgs {
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        true_arguments: Vec<ProductionRankedValueV1>,
        false_arguments: Vec<ProductionRankedValueV1>,
        true_block: u32,
        false_block: u32,
    },
    IndexEqual {
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        true_block: u32,
        false_block: u32,
    },
    IndexEqualArgs {
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        true_arguments: Vec<ProductionRankedValueV1>,
        false_arguments: Vec<ProductionRankedValueV1>,
        true_block: u32,
        false_block: u32,
    },
    AnalysisSplit {
        first_block: u32,
        second_block: u32,
    },
    Branch {
        target: u32,
    },
    BranchArgs {
        arguments: Vec<ProductionRankedValueV1>,
        target: u32,
    },
    BranchArgsAdd {
        value: ProductionRankedValueV1,
        step: ProductionRankedValueV1,
        target: u32,
    },
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRankedBlockV1 {
    index_argument_count: u32,
    operations: Vec<ProductionRankedOperationV1>,
    terminator: ProductionRankedTerminatorV1,
}

impl ProductionRankedBlockV1 {
    pub fn new(
        operations: Vec<ProductionRankedOperationV1>,
        terminator: ProductionRankedTerminatorV1,
    ) -> Self {
        Self {
            index_argument_count: 0,
            operations,
            terminator,
        }
    }

    pub fn with_index_arguments(
        index_argument_count: u32,
        operations: Vec<ProductionRankedOperationV1>,
        terminator: ProductionRankedTerminatorV1,
    ) -> Self {
        Self {
            index_argument_count,
            operations,
            terminator,
        }
    }

    pub const fn index_argument_count(&self) -> u32 {
        self.index_argument_count
    }

    pub fn operations(&self) -> &[ProductionRankedOperationV1] {
        &self.operations
    }

    pub const fn terminator(&self) -> &ProductionRankedTerminatorV1 {
        &self.terminator
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRankedKernelV1 {
    function_name: String,
    argument_count: usize,
    blocks: Vec<ProductionRankedBlockV1>,
    tree_work: usize,
}

impl ProductionRankedKernelV1 {
    pub fn new(
        function_name: &str,
        argument_count: usize,
        blocks: Vec<ProductionRankedBlockV1>,
    ) -> Result<Self, ProductionRankedKernelErrorV1> {
        validate_name(function_name, NameKind::Dialect)
            .map_err(ProductionRankedKernelErrorV1::InvalidFunctionName)?;
        let mut kernel = Self {
            function_name: function_name.to_owned(),
            argument_count,
            blocks,
            tree_work: 0,
        };
        kernel.tree_work = kernel.validate()?;
        Ok(kernel)
    }

    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    pub const fn argument_count(&self) -> usize {
        self.argument_count
    }

    pub fn blocks(&self) -> &[ProductionRankedBlockV1] {
        &self.blocks
    }

    fn validate(&self) -> Result<usize, ProductionRankedKernelErrorV1> {
        if self.argument_count > HARD_MAX_PRODUCTION_RANKED_ARGUMENTS {
            return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "function argument",
                limit: HARD_MAX_PRODUCTION_RANKED_ARGUMENTS,
                actual: self.argument_count,
            });
        }
        if self.blocks.is_empty() || self.blocks.len() > MAX_RANKED_BOUNDS_BLOCKS {
            return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "basic block",
                limit: MAX_RANKED_BOUNDS_BLOCKS,
                actual: self.blocks.len(),
            });
        }
        let operation_count = self.blocks.iter().try_fold(0_usize, |total, block| {
            total
                .checked_add(block.operations.len() + 1)?
                .checked_add(usize::from(matches!(
                    block.terminator,
                    ProductionRankedTerminatorV1::BranchArgsAdd { .. }
                )))
        });
        let Some(operation_count) = operation_count else {
            return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "operation",
                limit: MAX_RANKED_BOUNDS_OPERATIONS,
                actual: usize::MAX,
            });
        };
        if operation_count > MAX_RANKED_BOUNDS_OPERATIONS {
            return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "operation",
                limit: MAX_RANKED_BOUNDS_OPERATIONS,
                actual: operation_count,
            });
        }
        let tree_work = ranked_tree_work(self.blocks.len(), operation_count).ok_or(
            ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "operation tree work",
                limit: HARD_MAX_SESSION_OPERATION_TREE_ITEMS,
                actual: usize::MAX,
            },
        )?;
        if tree_work > HARD_MAX_SESSION_OPERATION_TREE_ITEMS {
            return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "operation tree work",
                limit: HARD_MAX_SESSION_OPERATION_TREE_ITEMS,
                actual: tree_work,
            });
        }

        let mut locals = Vec::new();
        let mut saw_execution_layout = false;
        let mut allocation_classes = HashMap::new();
        let mut total_block_arguments = 0_usize;
        for (block_index, block) in self.blocks.iter().enumerate() {
            if block_index == 0 && block.index_argument_count != 0
                || block.index_argument_count as usize > HARD_MAX_PRODUCTION_RANKED_ARGUMENTS
            {
                return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                    resource: "block argument",
                    limit: HARD_MAX_PRODUCTION_RANKED_ARGUMENTS,
                    actual: block.index_argument_count as usize,
                });
            }
            total_block_arguments = total_block_arguments
                .checked_add(block.index_argument_count as usize)
                .ok_or(ProductionRankedKernelErrorV1::ResourceLimit {
                    resource: "total block argument",
                    limit: MAX_RANKED_BOUNDS_OPERATIONS,
                    actual: usize::MAX,
                })?;
            if total_block_arguments > MAX_RANKED_BOUNDS_OPERATIONS {
                return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                    resource: "total block argument",
                    limit: MAX_RANKED_BOUNDS_OPERATIONS,
                    actual: total_block_arguments,
                });
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                validate_block_argument_values_v1(operation, block_index, &self.blocks)?;
                if let ProductionRankedOperationV1::ExecutionLayout {
                    workgroup_extents,
                    subgroup_size,
                    ..
                } = operation
                {
                    let workgroup_size = workgroup_extents
                        .iter()
                        .try_fold(1_u64, |volume, extent| volume.checked_mul(*extent));
                    if block_index != 0
                        || operation_index != 0
                        || saw_execution_layout
                        || workgroup_extents.contains(&0)
                        || workgroup_size.is_none()
                        || *subgroup_size == 0
                        || workgroup_size.is_some_and(|size| *subgroup_size > size)
                        || workgroup_size.is_some_and(|size| !size.is_multiple_of(*subgroup_size))
                    {
                        return Err(ProductionRankedKernelErrorV1::InvalidExecutionLayout);
                    }
                    saw_execution_layout = true;
                }
                if let ProductionRankedOperationV1::View {
                    allocation_origin,
                    noalias_class,
                    ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    allocation_origin,
                    noalias_class,
                    ..
                } = operation
                {
                    if *noalias_class != 0 && *allocation_origin == 0
                        || *allocation_origin != 0
                            && allocation_classes
                                .insert(*allocation_origin, *noalias_class)
                                .is_some_and(|previous| previous != *noalias_class)
                    {
                        return Err(ProductionRankedKernelErrorV1::InvalidAllocationContract);
                    }
                }
                let result = validate_operation(operation, self.argument_count, &locals)?;
                if let Some((identity, kind)) = result {
                    if block_index != 0 {
                        return Err(ProductionRankedKernelErrorV1::NonEntryDefinition {
                            block: block_index,
                        });
                    }
                    let expected = u32::try_from(locals.len()).map_err(|_| {
                        ProductionRankedKernelErrorV1::ResourceLimit {
                            resource: "local value",
                            limit: MAX_RANKED_BOUNDS_OPERATIONS,
                            actual: locals.len(),
                        }
                    })?;
                    if identity.get() != expected {
                        return Err(ProductionRankedKernelErrorV1::NonCanonicalValueId {
                            expected,
                            actual: identity.get(),
                        });
                    }
                    locals.push(kind);
                }
            }
            validate_terminator(
                &block.terminator,
                self.argument_count,
                &locals,
                &self.blocks,
                block_index,
            )?;
        }
        Ok(tree_work)
    }
}

fn ranked_tree_work(block_count: usize, operation_count: usize) -> Option<usize> {
    // Module: root + region + block + function edge. Function: root + region,
    // blocks, and operation edges. Each child operation contributes its root.
    6_usize
        .checked_add(block_count)?
        .checked_add(operation_count.checked_mul(2)?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedKernelErrorV1 {
    InvalidFunctionName(NameError),
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    InvalidShape,
    InvalidExecutionLayout,
    InvalidAllocationContract,
    UnsupportedElementWidth(u32),
    DynamicExtentCountMismatch {
        expected: usize,
        actual: usize,
    },
    UndefinedValue(ProductionRankedValueV1),
    NonCanonicalValueId {
        expected: u32,
        actual: u32,
    },
    ExpectedIndex(ProductionRankedValueV1),
    ExpectedSemantic(ProductionRankedValueV1),
    ExpectedView(ProductionRankedValueV1),
    DimensionOutOfBounds {
        dimension: u32,
        rank: usize,
    },
    AccessRankMismatch {
        expected: usize,
        actual: usize,
    },
    WriteThroughReadOnlyView,
    AtomicContractRequired,
    NonAtomicKindForAtomicAccess,
    InvalidBlockTarget(u32),
    NonEntryDefinition {
        block: usize,
    },
    MissingKernelDialect,
    MissingGpuDialect,
    Materialization(&'static str),
}

impl fmt::Display for ProductionRankedKernelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFunctionName(_) => {
                formatter.write_str("invalid ranked-kernel function name")
            }
            Self::ResourceLimit {
                resource,
                limit,
                actual,
            } => {
                write!(
                    formatter,
                    "ranked-kernel {resource} count {actual} exceeds {limit}"
                )
            }
            Self::InvalidShape => write!(
                formatter,
                "ranked view rank must be within 1..={MAX_RANKED_MEMORY_RANK}"
            ),
            Self::InvalidExecutionLayout => formatter.write_str(
                "ranked execution layout must be the unique first entry operation with nonzero workgroup axes and an integral subgroup width",
            ),
            Self::InvalidAllocationContract => formatter.write_str(
                "ranked views require consistent allocation origins and no-alias classes",
            ),
            Self::UnsupportedElementWidth(width) => write!(
                formatter,
                "ranked view element width {width} is not one of {SUPPORTED_ELEMENT_WIDTHS:?}"
            ),
            Self::DynamicExtentCountMismatch { expected, actual } => write!(
                formatter,
                "ranked view requires {expected} dynamic extents but has {actual}"
            ),
            Self::UndefinedValue(value) => write!(
                formatter,
                "ranked recipe references undefined value {value:?}"
            ),
            Self::NonCanonicalValueId { expected, actual } => write!(
                formatter,
                "ranked recipe local value ID {actual} is noncanonical; expected {expected}"
            ),
            Self::ExpectedIndex(value) => write!(
                formatter,
                "ranked recipe expected index value, found {value:?}"
            ),
            Self::ExpectedSemantic(value) => write!(
                formatter,
                "ranked recipe expected semantic scalar value, found {value:?}"
            ),
            Self::ExpectedView(value) => write!(
                formatter,
                "ranked recipe expected view value, found {value:?}"
            ),
            Self::DimensionOutOfBounds { dimension, rank } => write!(
                formatter,
                "ranked dimension {dimension} is outside rank {rank}"
            ),
            Self::AccessRankMismatch { expected, actual } => write!(
                formatter,
                "ranked access requires {expected} indices but has {actual}"
            ),
            Self::WriteThroughReadOnlyView => {
                formatter.write_str("ranked write targets a read-only view")
            }
            Self::AtomicContractRequired => formatter
                .write_str("atomic ranked access requires the explicit AtomicAccess recipe"),
            Self::NonAtomicKindForAtomicAccess => {
                formatter.write_str("AtomicAccess recipe requires an atomic access kind")
            }
            Self::InvalidBlockTarget(target) => {
                write!(formatter, "ranked terminator targets absent block {target}")
            }
            Self::NonEntryDefinition { block } => write!(
                formatter,
                "ranked recipe block {block} defines a value; closed production SSA definitions must be in entry block"
            ),
            Self::MissingKernelDialect => formatter.write_str(
                "production ranked construction requires the kernel dialect registration",
            ),
            Self::MissingGpuDialect => formatter.write_str(
                "production ranked barrier construction requires the gpu dialect registration",
            ),
            Self::Materialization(message) => formatter.write_str(message),
        }
    }
}

impl Error for ProductionRankedKernelErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecipeValueKindV1 {
    Index,
    Semantic,
    View { rank: usize, writable: bool },
}

fn require_value(
    value: ProductionRankedValueV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<RecipeValueKindV1, ProductionRankedKernelErrorV1> {
    match value {
        ProductionRankedValueV1::Argument(argument)
            if usize::try_from(argument)
                .ok()
                .is_some_and(|argument| argument < argument_count) =>
        {
            Ok(RecipeValueKindV1::Index)
        }
        ProductionRankedValueV1::Local(identity) => locals
            .get(identity.get() as usize)
            .copied()
            .ok_or(ProductionRankedKernelErrorV1::UndefinedValue(value)),
        ProductionRankedValueV1::BlockArgument { .. } => Ok(RecipeValueKindV1::Index),
        ProductionRankedValueV1::Argument(_) => {
            Err(ProductionRankedKernelErrorV1::UndefinedValue(value))
        }
    }
}

fn require_index(
    value: ProductionRankedValueV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    if matches!(
        require_value(value, argument_count, locals)?,
        RecipeValueKindV1::Index
    ) {
        Ok(())
    } else {
        Err(ProductionRankedKernelErrorV1::ExpectedIndex(value))
    }
}

fn require_view(
    value: ProductionRankedValueV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<(usize, bool), ProductionRankedKernelErrorV1> {
    match require_value(value, argument_count, locals)? {
        RecipeValueKindV1::View { rank, writable } => Ok((rank, writable)),
        RecipeValueKindV1::Index | RecipeValueKindV1::Semantic => {
            Err(ProductionRankedKernelErrorV1::ExpectedView(value))
        }
    }
}

fn require_semantic(
    value: ProductionRankedValueV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    if matches!(
        require_value(value, argument_count, locals)?,
        RecipeValueKindV1::Semantic
    ) {
        Ok(())
    } else {
        Err(ProductionRankedKernelErrorV1::ExpectedSemantic(value))
    }
}

fn validate_operation(
    operation: &ProductionRankedOperationV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<Option<(ProductionRankedValueIdV1, RecipeValueKindV1)>, ProductionRankedKernelErrorV1> {
    match operation {
        ProductionRankedOperationV1::ExecutionLayout { .. } => Ok(None),
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            ..
        }
        | ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            ..
        } => {
            if !(1..=MAX_RANKED_MEMORY_RANK).contains(&shape.len()) {
                return Err(ProductionRankedKernelErrorV1::InvalidShape);
            }
            if !SUPPORTED_ELEMENT_WIDTHS.contains(element_width) {
                return Err(ProductionRankedKernelErrorV1::UnsupportedElementWidth(
                    *element_width,
                ));
            }
            let expected = shape
                .iter()
                .filter(|extent| **extent == DYNAMIC_EXTENT)
                .count();
            if dynamic_extents.len() != expected {
                return Err(ProductionRankedKernelErrorV1::DynamicExtentCountMismatch {
                    expected,
                    actual: dynamic_extents.len(),
                });
            }
            for extent in dynamic_extents {
                require_index(*extent, argument_count, locals)?;
            }
            Ok(Some((
                *result,
                RecipeValueKindV1::View {
                    rank: shape.len(),
                    writable: *writable,
                },
            )))
        }
        ProductionRankedOperationV1::IndexConstant { result, .. } => {
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::InvocationIndex {
            result, dimension, ..
        } => {
            if usize::try_from(*dimension)
                .ok()
                .is_none_or(|dimension| dimension >= MAX_RANKED_MEMORY_RANK)
            {
                return Err(ProductionRankedKernelErrorV1::DimensionOutOfBounds {
                    dimension: *dimension,
                    rank: MAX_RANKED_MEMORY_RANK,
                });
            }
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::IndexBinary {
            result, lhs, rhs, ..
        } => {
            require_index(*lhs, argument_count, locals)?;
            require_index(*rhs, argument_count, locals)?;
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::DeterministicJoin {
            result,
            dependencies,
        } => {
            if !(1..=MAX_DETERMINISTIC_JOIN_INPUTS_V1).contains(&dependencies.len()) {
                return Err(ProductionRankedKernelErrorV1::ResourceLimit {
                    resource: "deterministic dependency",
                    limit: MAX_DETERMINISTIC_JOIN_INPUTS_V1,
                    actual: dependencies.len(),
                });
            }
            for dependency in dependencies {
                require_index(*dependency, argument_count, locals)?;
            }
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::CheckedTiledIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        } => {
            for value in [invocation, component, rows, columns, row_stride] {
                require_index(*value, argument_count, locals)?;
            }
            if *lanes_per_tile == 0
                || *tile_rows == 0
                || *tile_columns == 0
                || *elements_per_lane == 0
                || !lanes_per_tile.is_multiple_of(*tile_columns)
                || lanes_per_tile.checked_mul(*elements_per_lane)
                    != tile_rows.checked_mul(*tile_columns)
                || (lanes_per_tile / tile_columns).checked_mul(*elements_per_lane)
                    != Some(*tile_rows)
            {
                return Err(ProductionRankedKernelErrorV1::InvalidShape);
            }
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => {
            let (rank, _) = require_view(*view, argument_count, locals)?;
            if usize::try_from(*dimension)
                .ok()
                .is_none_or(|dimension| dimension >= rank)
            {
                return Err(ProductionRankedKernelErrorV1::DimensionOutOfBounds {
                    dimension: *dimension,
                    rank,
                });
            }
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => {
            if kind.is_atomic() {
                return Err(ProductionRankedKernelErrorV1::AtomicContractRequired);
            }
            validate_access(*kind, *view, indices, argument_count, locals)?;
            Ok(None)
        }
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            view,
            indices,
            ..
        } => {
            if !kind.is_atomic() {
                return Err(ProductionRankedKernelErrorV1::NonAtomicKindForAtomicAccess);
            }
            validate_access(*kind, *view, indices, argument_count, locals)?;
            Ok(None)
        }
        ProductionRankedOperationV1::Barrier { .. }
        | ProductionRankedOperationV1::Fence { .. }
        | ProductionRankedOperationV1::TensorLayout { .. } => Ok(None),
        ProductionRankedOperationV1::SemanticSymbol { result, .. }
        | ProductionRankedOperationV1::SemanticConstant { result, .. } => {
            Ok(Some((*result, RecipeValueKindV1::Semantic)))
        }
        ProductionRankedOperationV1::SemanticBinary {
            result, lhs, rhs, ..
        } => {
            require_semantic(*lhs, argument_count, locals)?;
            require_semantic(*rhs, argument_count, locals)?;
            Ok(Some((*result, RecipeValueKindV1::Semantic)))
        }
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => {
            require_semantic(*actual, argument_count, locals)?;
            require_semantic(*expected, argument_count, locals)?;
            Ok(None)
        }
    }
}

fn validate_access(
    kind: AccessKindAttr,
    view: ProductionRankedValueV1,
    indices: &[ProductionRankedValueV1],
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    let (rank, writable) = require_view(view, argument_count, locals)?;
    if indices.len() != rank {
        return Err(ProductionRankedKernelErrorV1::AccessRankMismatch {
            expected: rank,
            actual: indices.len(),
        });
    }
    if kind.writes_memory() && !writable {
        return Err(ProductionRankedKernelErrorV1::WriteThroughReadOnlyView);
    }
    for index in indices {
        require_index(*index, argument_count, locals)?;
    }
    Ok(())
}

fn validate_block_argument_value_v1(
    value: ProductionRankedValueV1,
    current_block: usize,
    blocks: &[ProductionRankedBlockV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    let ProductionRankedValueV1::BlockArgument { block, argument } = value else {
        return Ok(());
    };
    if block as usize != current_block
        || blocks
            .get(block as usize)
            .is_none_or(|recipe| argument >= recipe.index_argument_count)
    {
        return Err(ProductionRankedKernelErrorV1::UndefinedValue(value));
    }
    Ok(())
}

fn validate_block_argument_values_v1(
    operation: &ProductionRankedOperationV1,
    current_block: usize,
    blocks: &[ProductionRankedBlockV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    let validate = |value| validate_block_argument_value_v1(value, current_block, blocks);
    match operation {
        ProductionRankedOperationV1::View {
            dynamic_extents, ..
        }
        | ProductionRankedOperationV1::ViewInSpace {
            dynamic_extents, ..
        } => {
            for value in dynamic_extents {
                validate(*value)?;
            }
        }
        ProductionRankedOperationV1::IndexBinary { lhs, rhs, .. }
        | ProductionRankedOperationV1::SemanticBinary { lhs, rhs, .. } => {
            validate(*lhs)?;
            validate(*rhs)?;
        }
        ProductionRankedOperationV1::DeterministicJoin { dependencies, .. } => {
            for dependency in dependencies {
                validate(*dependency)?;
            }
        }
        ProductionRankedOperationV1::CheckedTiledIndex2D {
            invocation,
            component,
            rows,
            columns,
            row_stride,
            ..
        } => {
            for value in [invocation, component, rows, columns, row_stride] {
                validate(*value)?;
            }
        }
        ProductionRankedOperationV1::Dimension { view, .. } => validate(*view)?,
        ProductionRankedOperationV1::Access { view, indices, .. }
        | ProductionRankedOperationV1::AtomicAccess { view, indices, .. } => {
            validate(*view)?;
            for value in indices {
                validate(*value)?;
            }
        }
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => {
            validate(*actual)?;
            validate(*expected)?;
        }
        ProductionRankedOperationV1::ExecutionLayout { .. }
        | ProductionRankedOperationV1::IndexConstant { .. }
        | ProductionRankedOperationV1::InvocationIndex { .. }
        | ProductionRankedOperationV1::Barrier { .. }
        | ProductionRankedOperationV1::Fence { .. }
        | ProductionRankedOperationV1::TensorLayout { .. }
        | ProductionRankedOperationV1::SemanticSymbol { .. }
        | ProductionRankedOperationV1::SemanticConstant { .. } => {}
    }
    Ok(())
}

fn validate_terminator_block_argument_values_v1(
    terminator: &ProductionRankedTerminatorV1,
    current_block: usize,
    blocks: &[ProductionRankedBlockV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    let validate = |value| validate_block_argument_value_v1(value, current_block, blocks);
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. }
        | ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. }
        | ProductionRankedTerminatorV1::BranchArgsAdd {
            value: lhs,
            step: rhs,
            ..
        } => {
            validate(*lhs)?;
            validate(*rhs)
        }
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            ..
        } => {
            validate(*lhs)?;
            validate(*rhs)?;
            for value in true_arguments.iter().chain(false_arguments) {
                validate(*value)?;
            }
            Ok(())
        }
        ProductionRankedTerminatorV1::BranchArgs { arguments, .. } => {
            for value in arguments {
                validate(*value)?;
            }
            Ok(())
        }
        ProductionRankedTerminatorV1::AnalysisSplit { .. }
        | ProductionRankedTerminatorV1::Branch { .. }
        | ProductionRankedTerminatorV1::Return => Ok(()),
    }
}

fn validate_terminator(
    terminator: &ProductionRankedTerminatorV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
    blocks: &[ProductionRankedBlockV1],
    current_block: usize,
) -> Result<(), ProductionRankedKernelErrorV1> {
    let target = |target: u32| {
        usize::try_from(target)
            .ok()
            .filter(|target| *target < blocks.len())
            .map(|_| ())
            .ok_or(ProductionRankedKernelErrorV1::InvalidBlockTarget(target))
    };
    validate_terminator_block_argument_values_v1(terminator, current_block, blocks)?;
    let target_without_arguments = |destination: u32| {
        target(destination)?;
        if blocks[destination as usize].index_argument_count != 0 {
            return Err(ProductionRankedKernelErrorV1::Materialization(
                "ranked branch omits required successor arguments",
            ));
        }
        Ok(())
    };
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        }
        | ProductionRankedTerminatorV1::IndexEqual {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            require_index(*lhs, argument_count, locals)?;
            require_index(*rhs, argument_count, locals)?;
            target_without_arguments(*true_block)?;
            target_without_arguments(*false_block)
        }
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => {
            require_index(*lhs, argument_count, locals)?;
            require_index(*rhs, argument_count, locals)?;
            target(*true_block)?;
            target(*false_block)?;
            if true_arguments.len() != blocks[*true_block as usize].index_argument_count as usize
                || false_arguments.len()
                    != blocks[*false_block as usize].index_argument_count as usize
            {
                return Err(ProductionRankedKernelErrorV1::Materialization(
                    "ranked conditional branch arguments do not match successors",
                ));
            }
            for value in true_arguments.iter().chain(false_arguments) {
                require_index(*value, argument_count, locals)?;
            }
            Ok(())
        }
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => {
            target_without_arguments(*first_block)?;
            target_without_arguments(*second_block)
        }
        ProductionRankedTerminatorV1::Branch {
            target: destination,
        } => target_without_arguments(*destination),
        ProductionRankedTerminatorV1::BranchArgs {
            arguments,
            target: destination,
        } => {
            target(*destination)?;
            let expected = blocks[*destination as usize].index_argument_count as usize;
            if arguments.len() != expected {
                return Err(ProductionRankedKernelErrorV1::Materialization(
                    "ranked branch argument count does not match its successor",
                ));
            }
            for argument in arguments {
                require_index(*argument, argument_count, locals)?;
            }
            Ok(())
        }
        ProductionRankedTerminatorV1::BranchArgsAdd {
            value,
            step,
            target: destination,
        } => {
            target(*destination)?;
            if blocks[*destination as usize].index_argument_count != 1 {
                return Err(ProductionRankedKernelErrorV1::Materialization(
                    "ranked induction backedge requires one successor index argument",
                ));
            }
            require_index(*value, argument_count, locals)?;
            require_index(*step, argument_count, locals)
        }
        ProductionRankedTerminatorV1::Return => Ok(()),
    }
}

pub(super) struct ConstructedRootV1 {
    pub(super) identity: RootIdentityV1,
    pub(super) ranked_function: Option<Ptr<Operation>>,
    pub(super) ranked_kernel: Option<ProductionRankedKernelV1>,
    pub(super) general_check_report: Option<ProductionPlironPreloweringReportV1>,
}

pub(super) struct MaterializedConstructionV1 {
    pub(super) operation: OperationHandle,
    pub(super) ranked_function: Option<Ptr<Operation>>,
    pub(super) ranked_kernel: Option<ProductionRankedKernelV1>,
}

impl ProductionConstructionV1 {
    pub fn ranked_kernel(
        root_name: &str,
        kernel: ProductionRankedKernelV1,
    ) -> Result<Self, NameError> {
        validate_name(root_name, NameKind::Dialect)?;
        Ok(Self {
            kind: ProductionConstructionKindV1::RankedKernel {
                root_name: root_name.to_owned(),
                kernel,
            },
        })
    }
}

impl ProductionPlironSessionV1 {
    fn run_general_kernel_checks_guarded(
        &mut self,
        function: Ptr<Operation>,
    ) -> Result<ProductionPlironPreloweringReportV1, ProductionSessionErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let function = FuncOp::from_operation(function);
            require_production_pliron_checks_before_lowering_v1(&self.inner.context, &function)
        }));
        match result {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(error)) => Err(production_general_check_error(error)),
            Err(_) => {
                self.poisoned = true;
                Err(ProductionSessionErrorV1::Operation(
                    OperationHandleError::UpstreamPanicked,
                ))
            }
        }
    }

    pub(super) fn preflight_construction(
        &self,
        construction: &ProductionConstructionV1,
    ) -> Result<(), ProductionSessionErrorV1> {
        let tree_work = match &construction.kind {
            ProductionConstructionKindV1::BuiltinModule { .. } => 3,
            ProductionConstructionKindV1::RankedKernel { kernel, .. } => kernel.tree_work,
        };
        self.inner
            .require_internal_tree_capacity(tree_work)
            .map_err(ProductionSessionErrorV1::Operation)
    }

    pub(super) fn materialize_construction(
        &mut self,
        construction: ProductionConstructionV1,
        root_name: &str,
    ) -> Result<MaterializedConstructionV1, ProductionSessionErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| match construction.kind {
            ProductionConstructionKindV1::BuiltinModule { .. } => self
                .inner
                .create_module(root_name)
                .map(|operation| MaterializedConstructionV1 {
                    operation,
                    ranked_function: None,
                    ranked_kernel: None,
                })
                .map_err(ProductionSessionErrorV1::Operation),
            ProductionConstructionKindV1::RankedKernel { kernel, .. } => {
                self.materialize_ranked_kernel(root_name, kernel)
            }
        }));
        match result {
            Ok(result) => result,
            Err(_) => Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::UpstreamPanicked,
            )),
        }
    }

    fn materialize_ranked_kernel(
        &mut self,
        root_name: &str,
        kernel: ProductionRankedKernelV1,
    ) -> Result<MaterializedConstructionV1, ProductionSessionErrorV1> {
        if !self
            .inner
            .manifest()
            .registration_order()
            .iter()
            .any(|name| name == dialect_kernel::DIALECT_NAME)
        {
            return Err(ProductionSessionErrorV1::RankedRecipe(
                ProductionRankedKernelErrorV1::MissingKernelDialect,
            ));
        }
        let has_barrier = kernel.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation,
                    ProductionRankedOperationV1::Barrier { .. }
                        | ProductionRankedOperationV1::Fence { .. }
                        | ProductionRankedOperationV1::ExecutionLayout { .. }
                )
            })
        });
        if has_barrier
            && !self
                .inner
                .manifest()
                .registration_order()
                .iter()
                .any(|name| name == dialect_gpu::DIALECT_NAME)
        {
            return Err(ProductionSessionErrorV1::RankedRecipe(
                ProductionRankedKernelErrorV1::MissingGpuDialect,
            ));
        }
        let operation = self
            .inner
            .create_module(root_name)
            .map_err(ProductionSessionErrorV1::Operation)?;
        let root_pointer = self
            .inner
            .operations
            .get(&operation.identity)
            .copied()
            .ok_or(ProductionSessionErrorV1::Operation(
                OperationHandleError::StaleHandle,
            ))?;
        let module = ModuleOp::from_operation(root_pointer);
        let index: TypeHandle = IndexType::get(&self.inner.context).into();
        let function_type = FunctionType::get(
            &self.inner.context,
            vec![index; kernel.argument_count],
            vec![],
        );
        let function_name: Identifier = kernel.function_name.as_str().try_into().map_err(|_| {
            ProductionSessionErrorV1::RankedRecipe(ProductionRankedKernelErrorV1::Materialization(
                "validated function name could not be interned",
            ))
        })?;
        let function = FuncOp::new(&mut self.inner.context, function_name, function_type);
        module.append_operation(&mut self.inner.context, function.get_operation(), 0);

        let mut blocks = vec![function.get_entry_block(&self.inner.context)];
        for block_index in 1..kernel.blocks.len() {
            let label: Identifier =
                format!("bb{block_index}")
                    .as_str()
                    .try_into()
                    .map_err(|_| {
                        ProductionSessionErrorV1::RankedRecipe(
                            ProductionRankedKernelErrorV1::Materialization(
                                "generated block label could not be interned",
                            ),
                        )
                    })?;
            let block = BasicBlock::new(
                &mut self.inner.context,
                Some(label),
                vec![index; kernel.blocks[block_index].index_argument_count as usize],
            );
            block.insert_at_back(
                function.get_region(&self.inner.context),
                &self.inner.context,
            );
            blocks.push(block);
        }

        let arguments = blocks[0]
            .deref(&self.inner.context)
            .arguments()
            .collect::<Vec<_>>();
        let mut block_arguments = HashMap::new();
        for (block_index, block) in blocks.iter().copied().enumerate().skip(1) {
            for (argument_index, argument) in
                block.deref(&self.inner.context).arguments().enumerate()
            {
                block_arguments.insert((block_index as u32, argument_index as u32), argument);
            }
        }
        let mut locals = Vec::new();
        for (block_index, recipe_block) in kernel.blocks.iter().enumerate() {
            let block = blocks[block_index];
            for recipe in &recipe_block.operations {
                materialize_operation(
                    &mut self.inner.context,
                    block,
                    recipe,
                    &arguments,
                    &mut locals,
                    &block_arguments,
                )
                .map_err(ProductionSessionErrorV1::RankedRecipe)?;
            }
            materialize_terminator(
                &mut self.inner.context,
                block,
                &recipe_block.terminator,
                &blocks,
                &arguments,
                &locals,
                &block_arguments,
            )
            .map_err(ProductionSessionErrorV1::RankedRecipe)?;
        }
        self.inner
            .finish_internal_root_construction(&operation)
            .map_err(ProductionSessionErrorV1::Operation)?;
        Ok(MaterializedConstructionV1 {
            operation,
            ranked_function: Some(function.get_operation()),
            ranked_kernel: Some(kernel),
        })
    }

    /// Runs the fixed generic verifier pipeline in one prerequisite-aware
    /// sweep and returns only the final safety typestate.
    pub fn verify_general_ranked_kernel_checks(
        &mut self,
        stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: ProductionRootHandleV1<ConstructedGraphStageV1>,
    ) -> Result<
        (
            ProductionStageHandleV1<KernelChecksVerifiedGraphStageV1>,
            ProductionRootHandleV1<KernelChecksVerifiedGraphStageV1>,
        ),
        ProductionSessionErrorV1,
    > {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        self.authenticate_owner(root.owner)?;
        let record = self
            .constructed_roots
            .get(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if root.stage != stage.identity || root.identity != record.identity {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        if record.general_check_report.is_some() {
            return Err(ProductionSessionErrorV1::StaleStage);
        }
        let function = record
            .ranked_function
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let report = self.run_general_kernel_checks_guarded(function)?;
        let record = self
            .constructed_roots
            .get_mut(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        record.general_check_report = Some(report);
        Ok((
            ProductionStageHandleV1 {
                owner: stage.owner,
                identity: stage.identity,
                _stage: std::marker::PhantomData,
            },
            ProductionRootHandleV1 {
                owner: root.owner,
                stage: root.stage,
                identity: root.identity,
                operation: root.operation,
                _stage: std::marker::PhantomData,
            },
        ))
    }

    pub fn prepare_ranked_lowering(
        mut self,
        stage: ProductionStageHandleV1<KernelChecksVerifiedGraphStageV1>,
        root: ProductionRootHandleV1<KernelChecksVerifiedGraphStageV1>,
    ) -> Result<ProductionRankedKernelLoweringInputV1, ProductionSessionErrorV1> {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        self.authenticate_owner(root.owner)?;
        if let Err(error) = self.inner.operation_shape(&root.operation) {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::Operation(error));
        }
        let (expected_root, function, expected_report) = {
            let record = self
                .constructed_roots
                .get(&stage.identity)
                .ok_or(ProductionSessionErrorV1::StaleStage)?;
            (
                record.identity,
                record.ranked_function,
                record.general_check_report.clone(),
            )
        };
        if root.stage != stage.identity
            || root.identity != expected_root
            || expected_report.is_none()
        {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        let function = function.ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let revalidated = match self.run_general_kernel_checks_guarded(function) {
            Ok(report) => report,
            Err(_) => {
                self.poisoned = true;
                return Err(ProductionSessionErrorV1::RankedGraphChanged);
            }
        };
        if expected_report.as_ref() != Some(&revalidated) {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        let record = self
            .constructed_roots
            .remove(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if root.stage != stage.identity || root.identity != record.identity {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        let kernel = record
            .ranked_kernel
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let report = record
            .general_check_report
            .ok_or(ProductionSessionErrorV1::StageRootMismatch)?;
        if !report.is_clean() {
            return Err(ProductionSessionErrorV1::RankedRecipe(
                ProductionRankedKernelErrorV1::Materialization(
                    "safety-verified stage carried a rejected report",
                ),
            ));
        }
        let (tensor_layout_report, general_report) = report.into_parts();
        let (
            bounds_report,
            atomic_report,
            race_report,
            barrier_report,
            workgroup_report,
            semantic_report,
        ) = general_report.into_parts();
        Ok(ProductionRankedKernelLoweringInputV1 {
            kernel,
            tensor_layout_report,
            bounds_report,
            atomic_report,
            race_report,
            barrier_report,
            workgroup_report,
            semantic_report,
            _session: self,
            _stage: stage,
            _root: root,
        })
    }
}

fn production_general_check_error(
    error: ProductionPlironPreloweringErrorV1,
) -> ProductionSessionErrorV1 {
    let error = match error {
        ProductionPlironPreloweringErrorV1::TensorLayout(error) => {
            return ProductionSessionErrorV1::RankedTensorLayout(error);
        }
        ProductionPlironPreloweringErrorV1::General(error) => error,
    };
    match error {
        GeneralPlironKernelCheckErrorV1::Bounds(error) => {
            ProductionSessionErrorV1::RankedBounds(error)
        }
        GeneralPlironKernelCheckErrorV1::Atomic(error) => {
            ProductionSessionErrorV1::RankedAtomic(error)
        }
        GeneralPlironKernelCheckErrorV1::Race(error) => ProductionSessionErrorV1::RankedRace(error),
        GeneralPlironKernelCheckErrorV1::Barrier(error) => {
            ProductionSessionErrorV1::RankedBarrier(error)
        }
        GeneralPlironKernelCheckErrorV1::Workgroup(error) => {
            ProductionSessionErrorV1::RankedWorkgroup(error)
        }
        GeneralPlironKernelCheckErrorV1::Semantic(error) => {
            ProductionSessionErrorV1::RankedSemantic(error)
        }
    }
}

fn resolve_value(
    value: ProductionRankedValueV1,
    arguments: &[Value],
    locals: &[Value],
    block_arguments: &HashMap<(u32, u32), Value>,
) -> Result<Value, ProductionRankedKernelErrorV1> {
    match value {
        ProductionRankedValueV1::Argument(argument) => arguments
            .get(argument as usize)
            .copied()
            .ok_or(ProductionRankedKernelErrorV1::UndefinedValue(value)),
        ProductionRankedValueV1::Local(identity) => locals
            .get(identity.get() as usize)
            .copied()
            .ok_or(ProductionRankedKernelErrorV1::UndefinedValue(value)),
        ProductionRankedValueV1::BlockArgument { block, argument } => block_arguments
            .get(&(block, argument))
            .copied()
            .ok_or(ProductionRankedKernelErrorV1::UndefinedValue(value)),
    }
}

fn materialize_operation(
    context: &mut pliron::context::Context,
    block: Ptr<BasicBlock>,
    recipe: &ProductionRankedOperationV1,
    arguments: &[Value],
    locals: &mut Vec<Value>,
    block_arguments: &HashMap<(u32, u32), Value>,
) -> Result<(), ProductionRankedKernelErrorV1> {
    let (operation, result) = match recipe {
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
        } => {
            let op = ExecutionLayoutOp::new(
                context,
                *grid_identity,
                *global_extents,
                *workgroup_extents,
                *subgroup_size,
            );
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            allocation_origin,
            noalias_class,
        } => {
            let view_type = RankedViewType::new(context, *element_width, *writable, shape.clone())
                .map_err(|_| {
                    ProductionRankedKernelErrorV1::Materialization(
                        "validated ranked view failed materialization",
                    )
                })?;
            let dynamic_extents = dynamic_extents
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedViewOp::new_in_space_with_allocation_contract(
                context,
                view_type,
                dynamic_extents,
                MemorySpaceAttr::Global,
                *allocation_origin,
                *noalias_class,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated ranked view operation failed materialization",
                )
            })?;
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::CheckedTiledIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        } => {
            let op = CheckedTiledIndex2DOp::new(
                context,
                resolve_value(*invocation, arguments, locals, block_arguments)?,
                resolve_value(*component, arguments, locals, block_arguments)?,
                resolve_value(*rows, arguments, locals, block_arguments)?,
                resolve_value(*columns, arguments, locals, block_arguments)?,
                resolve_value(*row_stride, arguments, locals, block_arguments)?,
                [
                    *lanes_per_tile,
                    *tile_rows,
                    *tile_columns,
                    *elements_per_lane,
                ],
            );
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            allocation_origin,
            noalias_class,
        } => {
            let view_type = RankedViewType::new(context, *element_width, *writable, shape.clone())
                .map_err(|_| {
                    ProductionRankedKernelErrorV1::Materialization(
                        "validated ranked view failed materialization",
                    )
                })?;
            let dynamic_extents = dynamic_extents
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedViewOp::new_in_space_with_allocation_contract(
                context,
                view_type,
                dynamic_extents,
                *memory_space,
                *allocation_origin,
                *noalias_class,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated ranked view operation failed materialization",
                )
            })?;
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            let op = IndexConstantOp::new(context, *value);
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => {
            let op = InvocationIndexOp::new(context, *dimension, *launch_extent);
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            let op = IndexBinaryOp::new(
                context,
                *kind,
                resolve_value(*lhs, arguments, locals, block_arguments)?,
                resolve_value(*rhs, arguments, locals, block_arguments)?,
            );
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::DeterministicJoin {
            result,
            dependencies,
        } => {
            let dependencies = dependencies
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?;
            let op = DeterministicJoinOp::new(context, dependencies);
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => {
            let op = DimensionOp::new(
                context,
                resolve_value(*view, arguments, locals, block_arguments)?,
                *dimension,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated dimension failed materialization",
                )
            })?;
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => {
            let indices = indices
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedAccessOp::new(
                context,
                *kind,
                resolve_value(*view, arguments, locals, block_arguments)?,
                indices,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated ranked access failed materialization",
                )
            })?;
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
        } => {
            let indices = indices
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedAccessOp::new_atomic(
                context,
                *kind,
                *ordering,
                *scope,
                resolve_value(*view, arguments, locals, block_arguments)?,
                indices,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated ranked atomic access failed materialization",
                )
            })?;
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => {
            let op = BarrierOp::new(
                context,
                *execution_scope,
                *memory_scope,
                *address_space,
                *order,
            );
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::Fence {
            memory_scope,
            address_space,
            order,
        } => {
            let op = FenceOp::new(context, *memory_scope, *address_space, *order);
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
        } => {
            let op = TensorLayoutOp::new(context, contract, *convergence, *active_lanes);
            (op.get_operation(), None)
        }
        ProductionRankedOperationV1::SemanticSymbol { result, symbol } => {
            let op = SemanticSymbolOp::new(context, *symbol);
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::SemanticConstant { result, value } => {
            let op = SemanticConstantOp::new(context, *value);
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::SemanticBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            let op = SemanticBinaryOp::new(
                context,
                *kind,
                resolve_value(*lhs, arguments, locals, block_arguments)?,
                resolve_value(*rhs, arguments, locals, block_arguments)?,
            );
            (op.get_operation(), Some((*result, op.result(context))))
        }
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => {
            let op = RequireEquivalentOp::new(
                context,
                resolve_value(*actual, arguments, locals, block_arguments)?,
                resolve_value(*expected, arguments, locals, block_arguments)?,
            );
            (op.get_operation(), None)
        }
    };
    operation.insert_at_back(block, context);
    if let Some((identity, value)) = result {
        if identity.get() as usize != locals.len() {
            return Err(ProductionRankedKernelErrorV1::Materialization(
                "validated local value order changed before materialization",
            ));
        }
        locals.push(value);
    }
    Ok(())
}

fn materialize_terminator(
    context: &mut pliron::context::Context,
    block: Ptr<BasicBlock>,
    terminator: &ProductionRankedTerminatorV1,
    blocks: &[Ptr<BasicBlock>],
    arguments: &[Value],
    locals: &[Value],
    block_arguments: &HashMap<(u32, u32), Value>,
) -> Result<(), ProductionRankedKernelErrorV1> {
    let operation = match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } => IndexLessThanBranchOp::new(
            context,
            resolve_value(*lhs, arguments, locals, block_arguments)?,
            resolve_value(*rhs, arguments, locals, block_arguments)?,
            blocks[*true_block as usize],
            blocks[*false_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => IndexLessThanBranchArgsOp::new(
            context,
            resolve_value(*lhs, arguments, locals, block_arguments)?,
            resolve_value(*rhs, arguments, locals, block_arguments)?,
            true_arguments
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?,
            false_arguments
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?,
            blocks[*true_block as usize],
            blocks[*false_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::IndexEqual {
            lhs,
            rhs,
            true_block,
            false_block,
        } => IndexEqualBranchOp::new(
            context,
            resolve_value(*lhs, arguments, locals, block_arguments)?,
            resolve_value(*rhs, arguments, locals, block_arguments)?,
            blocks[*true_block as usize],
            blocks[*false_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => IndexEqualBranchArgsOp::new(
            context,
            resolve_value(*lhs, arguments, locals, block_arguments)?,
            resolve_value(*rhs, arguments, locals, block_arguments)?,
            true_arguments
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?,
            false_arguments
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?,
            blocks[*true_block as usize],
            blocks[*false_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => AnalysisSplitOp::new(
            context,
            blocks[*first_block as usize],
            blocks[*second_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::Branch { target } => {
            BranchOp::new(context, blocks[*target as usize]).get_operation()
        }
        ProductionRankedTerminatorV1::BranchArgs {
            arguments: edge_arguments,
            target,
        } => BranchArgsOp::new(
            context,
            edge_arguments
                .iter()
                .map(|value| resolve_value(*value, arguments, locals, block_arguments))
                .collect::<Result<Vec<_>, _>>()?,
            blocks[*target as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::BranchArgsAdd {
            value,
            step,
            target,
        } => {
            let next = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Add,
                resolve_value(*value, arguments, locals, block_arguments)?,
                resolve_value(*step, arguments, locals, block_arguments)?,
            );
            next.get_operation().insert_at_back(block, context);
            BranchArgsOp::new(
                context,
                vec![next.result(context)],
                blocks[*target as usize],
            )
            .get_operation()
        }
        ProductionRankedTerminatorV1::Return => ReturnOp::new(context).get_operation(),
    };
    operation.insert_at_back(block, context);
    Ok(())
}

/// Move-only output of the closed construction, bounds, and race stages.
///
/// The value owns the complete production session and verified stage/root, so
/// the exact checked graph remains alive while no raw Pliron pointer is exposed.
/// It does not authenticate a source allocation or grant compiler/artifact
/// authority; later production stages must bind the graph to retained frontend
/// memory facts and consume this value without reconstructing it.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionRankedKernelLoweringInputV1;
///
/// fn duplicate(input: ProductionRankedKernelLoweringInputV1) {
///     let _second = input.clone();
/// }
/// ```
#[must_use = "safety-verified ranked input must be consumed by a checked lowering stage"]
pub struct ProductionRankedKernelLoweringInputV1 {
    kernel: ProductionRankedKernelV1,
    tensor_layout_report: PlironTensorLayoutReportV1,
    bounds_report: RankedBoundsReportV1,
    atomic_report: PlironAtomicLegalityReportV1,
    race_report: RankedRaceReportV1,
    barrier_report: PlironBarrierReportV1,
    workgroup_report: PlironWorkgroupMemoryReportV1,
    semantic_report: PlironSemanticRefinementReportV1,
    _session: ProductionPlironSessionV1,
    _stage: ProductionStageHandleV1<KernelChecksVerifiedGraphStageV1>,
    _root: ProductionRootHandleV1<KernelChecksVerifiedGraphStageV1>,
}

impl fmt::Debug for ProductionRankedKernelLoweringInputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionRankedKernelLoweringInputV1")
            .field("function_name", &self.kernel.function_name())
            .field("argument_count", &self.kernel.argument_count())
            .field("block_count", &self.kernel.blocks().len())
            .finish_non_exhaustive()
    }
}

impl ProductionRankedKernelLoweringInputV1 {
    pub(super) fn revalidate_structure(&self) -> Result<(), ProductionRankedKernelErrorV1> {
        let tree_work = self.kernel.validate()?;
        if tree_work != self.kernel.tree_work {
            return Err(ProductionRankedKernelErrorV1::Materialization(
                "validated ranked-kernel tree work changed before evidence construction",
            ));
        }
        Ok(())
    }

    pub const fn kernel(&self) -> &ProductionRankedKernelV1 {
        &self.kernel
    }

    pub const fn bounds_report(&self) -> &RankedBoundsReportV1 {
        &self.bounds_report
    }

    pub const fn tensor_layout_report(&self) -> &PlironTensorLayoutReportV1 {
        &self.tensor_layout_report
    }

    pub const fn atomic_report(&self) -> &PlironAtomicLegalityReportV1 {
        &self.atomic_report
    }

    pub const fn race_report(&self) -> &RankedRaceReportV1 {
        &self.race_report
    }

    pub const fn barrier_report(&self) -> &PlironBarrierReportV1 {
        &self.barrier_report
    }

    pub const fn workgroup_report(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.workgroup_report
    }

    pub const fn semantic_report(&self) -> &PlironSemanticRefinementReportV1 {
        &self.semantic_report
    }

    pub fn all_mandatory_reports_are_clean(&self) -> bool {
        self.tensor_layout_report.is_clean()
            && self.bounds_report.is_clean()
            && self.atomic_report.is_clean()
            && self.race_report.is_clean()
            && self.barrier_report.is_clean()
            && self.workgroup_report.is_clean()
            && self.semantic_report.is_clean()
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionRankedCompileErrorV1 {
    Registration(NameError),
    Context(ContextBuildError),
    Session(ProductionSessionErrorV1),
}

impl fmt::Display for ProductionRankedCompileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registration(_) => {
                formatter.write_str("kernel dialect registration construction failed")
            }
            Self::Context(error) => write!(
                formatter,
                "production Pliron context construction failed: {error:?}"
            ),
            Self::Session(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionRankedCompileErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registration(_) | Self::Context(_) => None,
            Self::Session(error) => Some(error),
        }
    }
}

/// Executes the sole closed ranked-kernel production path through construction,
/// recursive structural verification, the fixed generic verifier pipeline, and
/// one checked lowering transition.
pub fn compile_ranked_kernel_for_lowering_v1(
    construction: ProductionConstructionV1,
    limits: ProductionSessionLimitsV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    let kernel_registration = dialect_kernel::dialect_registration()
        .map_err(ProductionRankedCompileErrorV1::Registration)?;
    let gpu_registration = dialect_gpu::dialect_registration()
        .map_err(ProductionRankedCompileErrorV1::Registration)?;
    let mut session =
        ProductionPlironSessionV1::new(limits, [kernel_registration, gpu_registration])
            .map_err(ProductionRankedCompileErrorV1::Context)?;
    let registered = session
        .register_construction(construction)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    let (constructed, root) = session
        .construct_registered(registered)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    let (verified, root) = session
        .verify_general_ranked_kernel_checks(constructed, root)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    session
        .prepare_ranked_lowering(verified, root)
        .map_err(ProductionRankedCompileErrorV1::Session)
}
