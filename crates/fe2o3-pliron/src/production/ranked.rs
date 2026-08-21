use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{
    AccessKindAttr, BranchOp, DYNAMIC_EXTENT, DimensionOp, IndexConstantOp, IndexLessThanBranchOp,
    IndexType, MAX_RANKED_MEMORY_RANK, RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp,
    SUPPORTED_ELEMENT_WIDTHS,
};
use fe2o3_kernel_analysis::{
    MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_OPERATIONS, RankedBoundsReportV1,
    require_pliron_ranked_bounds_before_lowering_v1,
};
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
    BoundsVerifiedGraphStageV1, ConstructedGraphStageV1, ProductionConstructionKindV1,
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
    Local(ProductionRankedValueIdV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedOperationV1 {
    View {
        result: ProductionRankedValueIdV1,
        element_width: u32,
        writable: bool,
        shape: Vec<u64>,
        dynamic_extents: Vec<ProductionRankedValueV1>,
    },
    IndexConstant {
        result: ProductionRankedValueIdV1,
        value: u64,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedTerminatorV1 {
    IndexLessThan {
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        true_block: u32,
        false_block: u32,
    },
    Branch {
        target: u32,
    },
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRankedBlockV1 {
    operations: Vec<ProductionRankedOperationV1>,
    terminator: ProductionRankedTerminatorV1,
}

impl ProductionRankedBlockV1 {
    pub fn new(
        operations: Vec<ProductionRankedOperationV1>,
        terminator: ProductionRankedTerminatorV1,
    ) -> Self {
        Self {
            operations,
            terminator,
        }
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
            total.checked_add(block.operations.len() + 1)
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
        for (block_index, block) in self.blocks.iter().enumerate() {
            for operation in &block.operations {
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
                self.blocks.len(),
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
    InvalidBlockTarget(u32),
    NonEntryDefinition {
        block: usize,
    },
    MissingKernelDialect,
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
        RecipeValueKindV1::Index => Err(ProductionRankedKernelErrorV1::ExpectedView(value)),
    }
}

fn validate_operation(
    operation: &ProductionRankedOperationV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<Option<(ProductionRankedValueIdV1, RecipeValueKindV1)>, ProductionRankedKernelErrorV1> {
    match operation {
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
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
            let (rank, writable) = require_view(*view, argument_count, locals)?;
            if indices.len() != rank {
                return Err(ProductionRankedKernelErrorV1::AccessRankMismatch {
                    expected: rank,
                    actual: indices.len(),
                });
            }
            if *kind == AccessKindAttr::Write && !writable {
                return Err(ProductionRankedKernelErrorV1::WriteThroughReadOnlyView);
            }
            for index in indices {
                require_index(*index, argument_count, locals)?;
            }
            Ok(None)
        }
    }
}

fn validate_terminator(
    terminator: &ProductionRankedTerminatorV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
    block_count: usize,
) -> Result<(), ProductionRankedKernelErrorV1> {
    let target = |target: u32| {
        usize::try_from(target)
            .ok()
            .filter(|target| *target < block_count)
            .map(|_| ())
            .ok_or(ProductionRankedKernelErrorV1::InvalidBlockTarget(target))
    };
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            require_index(*lhs, argument_count, locals)?;
            require_index(*rhs, argument_count, locals)?;
            target(*true_block)?;
            target(*false_block)
        }
        ProductionRankedTerminatorV1::Branch {
            target: destination,
        } => target(*destination),
        ProductionRankedTerminatorV1::Return => Ok(()),
    }
}

pub(super) struct ConstructedRootV1 {
    pub(super) identity: RootIdentityV1,
    pub(super) ranked_function: Option<Ptr<Operation>>,
    pub(super) ranked_kernel: Option<ProductionRankedKernelV1>,
    pub(super) bounds_verified: bool,
    pub(super) bounds_report: Option<RankedBoundsReportV1>,
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

    fn run_ranked_bounds_guarded(
        &mut self,
        function: Ptr<Operation>,
    ) -> Result<RankedBoundsReportV1, ProductionSessionErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let function = FuncOp::from_operation(function);
            require_pliron_ranked_bounds_before_lowering_v1(&self.inner.context, &function)
        }));
        match result {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(error)) => Err(ProductionSessionErrorV1::RankedBounds(error)),
            Err(_) => {
                self.poisoned = true;
                Err(ProductionSessionErrorV1::Operation(
                    OperationHandleError::UpstreamPanicked,
                ))
            }
        }
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
            let block = BasicBlock::new(&mut self.inner.context, Some(label), vec![]);
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

    pub fn verify_ranked_bounds(
        &mut self,
        stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: ProductionRootHandleV1<ConstructedGraphStageV1>,
    ) -> Result<
        (
            ProductionStageHandleV1<BoundsVerifiedGraphStageV1>,
            ProductionRootHandleV1<BoundsVerifiedGraphStageV1>,
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
        if record.bounds_verified {
            return Err(ProductionSessionErrorV1::StaleStage);
        }
        let function = record
            .ranked_function
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let report = self.run_ranked_bounds_guarded(function)?;
        let record = self
            .constructed_roots
            .get_mut(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        record.bounds_verified = true;
        record.bounds_report = Some(report);
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
        stage: ProductionStageHandleV1<BoundsVerifiedGraphStageV1>,
        root: ProductionRootHandleV1<BoundsVerifiedGraphStageV1>,
    ) -> Result<ProductionRankedKernelLoweringInputV1, ProductionSessionErrorV1> {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        self.authenticate_owner(root.owner)?;
        if let Err(error) = self.inner.operation_shape(&root.operation) {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::Operation(error));
        }
        let (expected_root, function, bounds_verified) = {
            let record = self
                .constructed_roots
                .get(&stage.identity)
                .ok_or(ProductionSessionErrorV1::StaleStage)?;
            (
                record.identity,
                record.ranked_function,
                record.bounds_verified,
            )
        };
        if root.stage != stage.identity || root.identity != expected_root || !bounds_verified {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        let function = function.ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let revalidated = match self.run_ranked_bounds_guarded(function) {
            Ok(report) => report,
            Err(_) => {
                self.poisoned = true;
                return Err(ProductionSessionErrorV1::RankedGraphChanged);
            }
        };
        if self
            .constructed_roots
            .get(&stage.identity)
            .and_then(|record| record.bounds_report.as_ref())
            != Some(&revalidated)
        {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        let record = self
            .constructed_roots
            .remove(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if root.stage != stage.identity
            || root.identity != record.identity
            || !record.bounds_verified
        {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        let kernel = record
            .ranked_kernel
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let report = record
            .bounds_report
            .ok_or(ProductionSessionErrorV1::StageRootMismatch)?;
        if !report.is_clean() {
            return Err(ProductionSessionErrorV1::RankedRecipe(
                ProductionRankedKernelErrorV1::Materialization(
                    "bounds-verified stage carried a rejected report",
                ),
            ));
        }
        Ok(ProductionRankedKernelLoweringInputV1 {
            kernel,
            report,
            _session: self,
            _stage: stage,
            _root: root,
        })
    }
}

fn resolve_value(
    value: ProductionRankedValueV1,
    arguments: &[Value],
    locals: &[Value],
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
    }
}

fn materialize_operation(
    context: &mut pliron::context::Context,
    block: Ptr<BasicBlock>,
    recipe: &ProductionRankedOperationV1,
    arguments: &[Value],
    locals: &mut Vec<Value>,
) -> Result<(), ProductionRankedKernelErrorV1> {
    let (operation, result) = match recipe {
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
        } => {
            let view_type = RankedViewType::new(context, *element_width, *writable, shape.clone())
                .map_err(|_| {
                    ProductionRankedKernelErrorV1::Materialization(
                        "validated ranked view failed materialization",
                    )
                })?;
            let dynamic_extents = dynamic_extents
                .iter()
                .map(|value| resolve_value(*value, arguments, locals))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedViewOp::new(context, view_type, dynamic_extents).map_err(|_| {
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
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => {
            let op = DimensionOp::new(
                context,
                resolve_value(*view, arguments, locals)?,
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
                .map(|value| resolve_value(*value, arguments, locals))
                .collect::<Result<Vec<_>, _>>()?;
            let op = RankedAccessOp::new(
                context,
                *kind,
                resolve_value(*view, arguments, locals)?,
                indices,
            )
            .map_err(|_| {
                ProductionRankedKernelErrorV1::Materialization(
                    "validated ranked access failed materialization",
                )
            })?;
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
) -> Result<(), ProductionRankedKernelErrorV1> {
    let operation = match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } => IndexLessThanBranchOp::new(
            context,
            resolve_value(*lhs, arguments, locals)?,
            resolve_value(*rhs, arguments, locals)?,
            blocks[*true_block as usize],
            blocks[*false_block as usize],
        )
        .get_operation(),
        ProductionRankedTerminatorV1::Branch { target } => {
            BranchOp::new(context, blocks[*target as usize]).get_operation()
        }
        ProductionRankedTerminatorV1::Return => ReturnOp::new(context).get_operation(),
    };
    operation.insert_at_back(block, context);
    Ok(())
}

/// Move-only output of the closed construction and bounds stages.
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
#[must_use = "bounds-verified ranked input must be consumed by a checked lowering stage"]
pub struct ProductionRankedKernelLoweringInputV1 {
    kernel: ProductionRankedKernelV1,
    report: RankedBoundsReportV1,
    _session: ProductionPlironSessionV1,
    _stage: ProductionStageHandleV1<BoundsVerifiedGraphStageV1>,
    _root: ProductionRootHandleV1<BoundsVerifiedGraphStageV1>,
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
    pub const fn kernel(&self) -> &ProductionRankedKernelV1 {
        &self.kernel
    }

    pub const fn bounds_report(&self) -> &RankedBoundsReportV1 {
        &self.report
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

/// Executes the sole closed ranked-memory production path through construction,
/// recursive structural verification, whole-function bounds analysis, and the
/// bounds-verified typestate transition.
pub fn compile_ranked_kernel_for_lowering_v1(
    construction: ProductionConstructionV1,
    limits: ProductionSessionLimitsV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    let registration = dialect_kernel::dialect_registration()
        .map_err(ProductionRankedCompileErrorV1::Registration)?;
    let mut session = ProductionPlironSessionV1::new(limits, [registration])
        .map_err(ProductionRankedCompileErrorV1::Context)?;
    let registered = session
        .register_construction(construction)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    let (constructed, root) = session
        .construct_registered(registered)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    let (verified, root) = session
        .verify_ranked_bounds(constructed, root)
        .map_err(ProductionRankedCompileErrorV1::Session)?;
    session
        .prepare_ranked_lowering(verified, root)
        .map_err(ProductionRankedCompileErrorV1::Session)
}
