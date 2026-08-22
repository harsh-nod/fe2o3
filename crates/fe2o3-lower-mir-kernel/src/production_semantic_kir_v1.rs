//! Owner-consuming production lowering from exact semantic MIR to Kernel IR.
//!
//! This boundary is intentionally fail-closed. It admits the exact scalar
//! `DisjointSlice::get_mut` fill profile, including trusted invocation indices,
//! structured control flow, and typed global stores. Every additional semantic
//! operation must arrive with an exact correspondence rule and Kernel IR
//! verification. The older detached `AlgorithmOp` marker pass is not used by
//! this API.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate,
    Constant, Function, FunctionId, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, SwitchCase, Terminator, Type, UnaryOp, ValueDef, ValueId, VerificationErrors,
    WorkgroupSize, verify_module,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAxisV1, SemanticBinaryOpV1, SemanticBlockIdV1, SemanticCallableDeclV1,
    SemanticCastKindV1, SemanticCompilerIntrinsicOperationV1, SemanticConstantValueV1,
    SemanticDirectCallV1, SemanticDisjointIndexSpaceV1, SemanticFunctionDeclV1,
    SemanticFunctionIdV1, SemanticFunctionRoleV1, SemanticLocalRoleV1, SemanticMutabilityV1,
    SemanticOperandV1, SemanticPlaceV1, SemanticPointerMetadataV1, SemanticProjectionKindV1,
    SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticScalarValueV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
    SemanticUnaryOpV1, SemanticUnwindActionV1, SemanticVolatilityV1,
};
use fe2o3_pliron::{ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1};

const DEFAULT_MAX_FUNCTIONS_V1: usize = 1_024;
const DEFAULT_MAX_BLOCKS_V1: usize = 16_384;
const DEFAULT_MAX_STATEMENTS_V1: usize = 1_048_576;

/// Independent work limits for semantic-MIR-to-Kernel-IR lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticKirLimitsV1 {
    max_functions: usize,
    max_blocks: usize,
    max_statements: usize,
}

impl ProductionSemanticKirLimitsV1 {
    /// Constructs explicit lowering limits.
    pub const fn new(max_functions: usize, max_blocks: usize, max_statements: usize) -> Self {
        Self {
            max_functions,
            max_blocks,
            max_statements,
        }
    }
}

impl Default for ProductionSemanticKirLimitsV1 {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_FUNCTIONS_V1,
            DEFAULT_MAX_BLOCKS_V1,
            DEFAULT_MAX_STATEMENTS_V1,
        )
    }
}

/// A bounded resource charged by production target-neutral lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticKirResourceV1 {
    /// Semantic functions inspected.
    Functions,
    /// Semantic blocks inspected and materialized.
    Blocks,
    /// Semantic statements inspected.
    Statements,
}

/// Pointer-independent evidence relating one source block to one Kernel IR block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirBlockCorrespondenceV1 {
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    kernel_ir_block: BlockId,
    source_statement_count: u32,
}

impl SemanticKirBlockCorrespondenceV1 {
    /// Returns the exact semantic function locator.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the exact semantic block locator.
    pub const fn semantic_block(self) -> SemanticBlockIdV1 {
        self.semantic_block
    }

    /// Returns the corresponding Kernel IR block identity.
    pub const fn kernel_ir_block(self) -> BlockId {
        self.kernel_ir_block
    }

    /// Returns the number of source statements covered by this block rule.
    pub const fn source_statement_count(self) -> u32 {
        self.source_statement_count
    }
}

/// Stable evidence binding one Kernel IR module to exact admitted semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticKirCorrespondenceV1 {
    semantic_sha256: [u8; 32],
    function_count: usize,
    blocks: Box<[SemanticKirBlockCorrespondenceV1]>,
}

impl SemanticKirCorrespondenceV1 {
    /// Returns the exact admitted semantic identity.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }

    /// Returns the number of semantic functions covered.
    pub const fn function_count(&self) -> usize {
        self.function_count
    }

    /// Returns source-to-Kernel-IR block evidence in lowering order.
    pub fn blocks(&self) -> &[SemanticKirBlockCorrespondenceV1] {
        &self.blocks
    }
}

/// Fail-closed diagnostics from production target-neutral lowering.
#[derive(Debug)]
pub enum ProductionSemanticKirErrorV1 {
    /// The exact semantic owner failed recursive verification.
    SemanticOwner(ProductionSemanticMirErrorV1),
    /// A bounded lowering resource exceeded its limit.
    ResourceLimit {
        /// Resource that exceeded its limit.
        resource: ProductionSemanticKirResourceV1,
        /// Observed work.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A semantic construct has no exact lowering rule.
    Unsupported {
        /// Source semantic function index.
        function: u32,
        /// Source semantic block index when available.
        block: Option<u32>,
        /// Source semantic statement ordinal when available.
        statement: Option<u32>,
        /// Stable rejection reason.
        detail: &'static str,
    },
    /// A semantic local is used before an SSA value is available on this path.
    MissingLocalDefinition {
        /// Source semantic function index.
        function: u32,
        /// Source semantic block index.
        block: u32,
        /// Source semantic statement ordinal when available.
        statement: Option<u32>,
        /// Missing semantic local index.
        local: u32,
    },
    /// The constructed Kernel IR failed structural or semantic verification.
    InvalidKernelIr(VerificationErrors),
    /// Retained correspondence no longer matches the exact source owner.
    CorrespondenceMismatch,
}

impl fmt::Display for ProductionSemanticKirErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticOwner(error) => write!(formatter, "exact semantic owner failed: {error}"),
            Self::ResourceLimit {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "semantic-to-Kernel-IR {resource:?} work {actual} exceeds limit {limit}",
            ),
            Self::Unsupported {
                function,
                block,
                statement,
                detail,
            } => write!(
                formatter,
                "semantic-to-Kernel-IR lowering rejected function {function}, block {block:?}, statement {statement:?}: {detail}",
            ),
            Self::MissingLocalDefinition {
                function,
                block,
                statement,
                local,
            } => write!(
                formatter,
                "semantic-to-Kernel-IR lowering rejected function {function}, block {block}, statement {statement:?}: local {local} has no path-available SSA definition",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::CorrespondenceMismatch => formatter.write_str(
                "semantic-to-Kernel-IR correspondence no longer matches its exact owner",
            ),
        }
    }
}

impl Error for ProductionSemanticKirErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::InvalidKernelIr(error) => Some(error),
            Self::ResourceLimit { .. }
            | Self::Unsupported { .. }
            | Self::MissingLocalDefinition { .. }
            | Self::CorrespondenceMismatch => None,
        }
    }
}

/// Move-only owner of one exact semantic source and its verified Kernel IR.
#[must_use = "dropping the owner abandons the verified target-neutral lowering"]
pub struct ProductionSemanticKirOwnerV1 {
    semantic: ProductionSemanticMirOwnerV1,
    module: Module,
    correspondence: SemanticKirCorrespondenceV1,
}

impl fmt::Debug for ProductionSemanticKirOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSemanticKirOwnerV1")
            .field("module", &self.module.id)
            .field("correspondence", &self.correspondence)
            .finish_non_exhaustive()
    }
}

impl ProductionSemanticKirOwnerV1 {
    /// Consumes exact semantic ownership and constructs verified Kernel IR.
    pub fn try_lower(
        semantic: ProductionSemanticMirOwnerV1,
        limits: ProductionSemanticKirLimitsV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        semantic
            .verify_equivalence()
            .map_err(ProductionSemanticKirErrorV1::SemanticOwner)?;
        let (module, correspondence) = lower_module(&semantic, limits)?;
        verify_module(&module).map_err(ProductionSemanticKirErrorV1::InvalidKernelIr)?;
        let owner = Self {
            semantic,
            module,
            correspondence,
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Re-verifies semantic ownership, Kernel IR, and retained correspondence.
    pub fn verify_equivalence(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        self.semantic
            .verify_equivalence()
            .map_err(ProductionSemanticKirErrorV1::SemanticOwner)?;
        verify_module(&self.module).map_err(ProductionSemanticKirErrorV1::InvalidKernelIr)?;
        if self.correspondence.semantic_sha256
            != *self.semantic.semantic().semantic_sha256().as_bytes()
            || self.correspondence.function_count != self.semantic.semantic().functions().len()
            || self.correspondence.blocks.len()
                != self
                    .semantic
                    .semantic()
                    .functions()
                    .iter()
                    .map(|function| function.blocks().len())
                    .sum::<usize>()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    /// Borrows the retained exact semantic owner.
    pub const fn semantic(&self) -> &ProductionSemanticMirOwnerV1 {
        &self.semantic
    }

    /// Borrows the structurally verified Kernel IR module.
    pub const fn module(&self) -> &Module {
        &self.module
    }

    /// Borrows pointer-independent source correspondence evidence.
    pub const fn correspondence(&self) -> &SemanticKirCorrespondenceV1 {
        &self.correspondence
    }

    /// Exact target-neutral lowering evidence is not artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn lower_module(
    owner: &ProductionSemanticMirOwnerV1,
    limits: ProductionSemanticKirLimitsV1,
) -> Result<(Module, SemanticKirCorrespondenceV1), ProductionSemanticKirErrorV1> {
    let semantic = owner.semantic();
    enforce_limit(
        ProductionSemanticKirResourceV1::Functions,
        semantic.functions().len(),
        limits.max_functions,
    )?;
    if semantic.functions().len() != 1 || semantic.roots().len() != 1 {
        return Err(unsupported(
            0,
            None,
            None,
            "only one closed kernel root is admitted",
        ));
    }
    if !semantic.allocations().is_empty()
        || !semantic.statics().is_empty()
        || !semantic.vtables().is_empty()
    {
        return Err(unsupported(
            0,
            None,
            None,
            "allocations, statics, and vtables are not lowered yet",
        ));
    }
    let function = &semantic.functions()[0];
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || semantic.roots()[0] != SemanticFunctionIdV1::from_index(0)
    {
        return Err(unsupported(
            0,
            None,
            None,
            "the sole function is not the sole kernel root",
        ));
    }
    let entry = function
        .kernel_entry()
        .ok_or_else(|| unsupported(0, None, None, "kernel export metadata is missing"))?;
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| unsupported(0, None, None, "kernel export symbol is not UTF-8"))?;

    enforce_limit(
        ProductionSemanticKirResourceV1::Blocks,
        function.blocks().len(),
        limits.max_blocks,
    )?;
    let statement_count = function
        .blocks()
        .iter()
        .try_fold(0_usize, |count, block| {
            count.checked_add(block.statements().len())
        })
        .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Statements,
            actual: usize::MAX,
            limit: limits.max_statements,
        })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Statements,
        statement_count,
        limits.max_statements,
    )?;

    let mut parameters = function
        .locals()
        .iter()
        .enumerate()
        .filter_map(|(local, declaration)| match declaration.role() {
            SemanticLocalRoleV1::Argument(argument) => Some((argument, local, declaration.ty())),
            SemanticLocalRoleV1::Return | SemanticLocalRoleV1::Temporary => None,
        })
        .collect::<Vec<_>>();
    parameters.sort_by_key(|(argument, _, _)| *argument);
    if parameters
        .iter()
        .enumerate()
        .any(|(expected, (actual, _, _))| usize::try_from(*actual) != Ok(expected))
    {
        return Err(unsupported(
            0,
            None,
            None,
            "kernel argument locals are not contiguous",
        ));
    }
    let parameter_types = parameters
        .iter()
        .map(|(_, _, ty)| lower_parameter_type(semantic.types(), semantic.callables(), *ty))
        .collect::<Result<Vec<_>, _>>()?;
    let parameter_values = parameters
        .iter()
        .map(|(_, local, _)| {
            u32::try_from(*local)
                .map(ValueId)
                .map_err(|_| unsupported(0, None, None, "local identity does not fit Kernel IR"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut lowering = SemanticFunctionLoweringV1::new(
        semantic.types(),
        semantic.callables(),
        function,
        &parameters,
        &parameter_values,
        &parameter_types,
    )?;

    let order = semantic_cfg_preorder(function)?;
    let mut blocks = Vec::with_capacity(order.len());
    let mut correspondence = Vec::with_capacity(order.len());
    for semantic_block in order {
        let index = usize::try_from(semantic_block.index())
            .map_err(|_| unsupported(0, None, None, "block identity does not fit this host"))?;
        let source = function.blocks().get(index).ok_or_else(|| {
            unsupported(0, Some(semantic_block.index()), None, "block is missing")
        })?;
        let mut target = BasicBlock::new(BlockId(semantic_block.index()));
        for (statement, operation) in source.statements().iter().enumerate() {
            lowering.lower_statement(
                semantic_block,
                u32::try_from(statement).ok(),
                operation.kind(),
                &mut target.operations,
            )?;
        }
        target.terminator = Some(lowering.lower_terminator(
            semantic_block,
            source.terminator().kind(),
            &mut target.operations,
        )?);
        blocks.push(target);
        correspondence.push(SemanticKirBlockCorrespondenceV1 {
            semantic_function: SemanticFunctionIdV1::from_index(0),
            semantic_block,
            kernel_ir_block: BlockId(semantic_block.index()),
            source_statement_count: u32::try_from(source.statements().len()).map_err(|_| {
                unsupported(
                    0,
                    Some(semantic_block.index()),
                    None,
                    "statement count is too large",
                )
            })?,
        });
    }

    let function_id = FunctionId::new(symbol);
    let mut module = Module::new(format!(
        "fe2o3::semantic::{}",
        hex_identity(semantic.semantic_sha256().as_bytes())
    ));
    module.functions.push(Function::kernel_entry(
        function_id.clone(),
        Signature::new(parameter_types, vec![]),
        parameter_values,
        blocks,
    ));
    let mut kernel = Kernel::new(
        symbol,
        function_id,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    if let Some(required) = entry
        .source_contract()
        .launch()
        .and_then(|launch| launch.required())
    {
        let [x, y, z] = required.as_array();
        kernel.workgroup_size = Some(WorkgroupSize::new(x, y, z));
    }
    module.kernels.push(kernel);

    Ok((
        module,
        SemanticKirCorrespondenceV1 {
            semantic_sha256: *semantic.semantic_sha256().as_bytes(),
            function_count: semantic.functions().len(),
            blocks: correspondence.into_boxed_slice(),
        },
    ))
}

fn semantic_cfg_preorder(
    function: &SemanticFunctionDeclV1,
) -> Result<Vec<SemanticBlockIdV1>, ProductionSemanticKirErrorV1> {
    let mut order = Vec::with_capacity(function.blocks().len());
    let mut visited = vec![false; function.blocks().len()];
    let mut stack = vec![function.entry()];
    while let Some(block) = stack.pop() {
        let index = usize::try_from(block.index())
            .map_err(|_| unsupported(0, None, None, "block identity does not fit this host"))?;
        let Some(source) = function.blocks().get(index) else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "CFG traversal references a missing block",
            ));
        };
        if visited[index] {
            continue;
        }
        visited[index] = true;
        order.push(block);
        let mut successors = Vec::with_capacity(source.terminator().kind().edge_count());
        source
            .terminator()
            .kind()
            .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                let target = edge.target();
                let target_index = usize::try_from(target.index()).map_err(|_| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "CFG successor identity does not fit this host",
                    )
                })?;
                if target_index >= function.blocks().len() {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "CFG traversal references a missing successor",
                    ));
                }
                successors.push(target);
                Ok(())
            })?;
        stack.extend(successors.into_iter().rev());
    }
    order.extend(
        visited
            .iter()
            .enumerate()
            .filter(|(_, seen)| !**seen)
            .map(|(index, _)| SemanticBlockIdV1::from_index(index as u32)),
    );
    Ok(order)
}

#[derive(Clone, Debug)]
enum SemanticValueBindingV1 {
    Value {
        id: ValueId,
        ty: Type,
    },
    OptionPointer {
        present: ValueId,
        pointer: ValueId,
        pointer_ty: Type,
    },
    IndexWitness {
        id: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        disjoint: bool,
    },
    OptionIndexWitness {
        present: ValueId,
        id: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
    },
    BlockWitness {
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
    },
    OptionBlockWitness {
        present: ValueId,
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
    },
    GridLeader,
    OptionGridLeader {
        present: ValueId,
    },
}

impl SemanticValueBindingV1 {
    fn value(&self) -> Result<(ValueId, Type), &'static str> {
        match self {
            Self::Value { id, ty } => Ok((*id, ty.clone())),
            Self::IndexWitness { id, .. } => Ok((*id, Type::INDEX)),
            Self::OptionPointer { .. }
            | Self::OptionIndexWitness { .. }
            | Self::GridLeader
            | Self::BlockWitness { .. }
            | Self::OptionBlockWitness { .. }
            | Self::OptionGridLeader { .. } => {
                Err("aggregate or capability value requires a semantic projection")
            }
        }
    }
}

struct SemanticFunctionLoweringV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    locals: Vec<Option<SemanticValueBindingV1>>,
    next_value: u32,
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        parameters: &[(u32, usize, SemanticTypeIdV1)],
        parameter_values: &[ValueId],
        parameter_types: &[Type],
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut locals = vec![None; function.locals().len()];
        for ((_, local, _), (value, ty)) in parameters
            .iter()
            .zip(parameter_values.iter().zip(parameter_types))
        {
            locals[*local] = Some(SemanticValueBindingV1::Value {
                id: *value,
                ty: ty.clone(),
            });
        }
        let next_value = u32::try_from(function.locals().len())
            .map_err(|_| unsupported(0, None, None, "local count does not fit Kernel IR"))?;
        Ok(Self {
            types,
            callables,
            function,
            locals,
            next_value,
        })
    }

    fn lower_statement(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        kind: &SemanticStatementKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                let value = self.lower_rvalue(
                    block,
                    statement,
                    assignment.value().result_type(),
                    assignment.value().kind(),
                    operations,
                )?;
                self.assign_place(
                    block,
                    statement,
                    assignment.destination(),
                    value,
                    SemanticVolatilityV1::NonVolatile,
                    operations,
                )
            }
            SemanticStatementKindV1::Store(store) if store.atomic().is_none() => {
                let value = self.lower_operand(block, statement, store.value(), operations)?;
                self.assign_place(
                    block,
                    statement,
                    store.destination(),
                    value,
                    store.volatility(),
                    operations,
                )
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                self.require_local(block, statement, local.index())?;
                Ok(())
            }
            SemanticStatementKindV1::Nop => Ok(()),
            _ => Err(unsupported(
                0,
                Some(block.index()),
                statement,
                unsupported_statement_detail(kind)
                    .unwrap_or("semantic statement has no exact Kernel IR lowering rule"),
            )),
        }
    }

    fn lower_rvalue(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        match value {
            SemanticRvalueKindV1::Use(operand) => {
                self.lower_operand(block, statement, operand, operations)
            }
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                self.resolve_place(block, statement, place)
            }
            SemanticRvalueKindV1::Discriminant(place) => {
                match self.resolve_place(block, statement, place)? {
                    SemanticValueBindingV1::OptionPointer { present, .. }
                    | SemanticValueBindingV1::OptionIndexWitness { present, .. }
                    | SemanticValueBindingV1::OptionBlockWitness { present, .. }
                    | SemanticValueBindingV1::OptionGridLeader { present } => {
                        let target = lower_scalar_type(self.types, result_type)?;
                        if target == Type::BOOL {
                            Ok(SemanticValueBindingV1::Value {
                                id: present,
                                ty: Type::BOOL,
                            })
                        } else if target.as_scalar().is_some_and(ScalarType::is_integer) {
                            self.emit(
                                operations,
                                target.clone(),
                                OperationKind::Cast {
                                    kind: CastKind::ZeroExtend,
                                    value: present,
                                    to: target,
                                },
                            )
                        } else {
                            Err(unsupported(
                                0,
                                Some(block.index()),
                                statement,
                                "semantic option discriminant is not integer-valued",
                            ))
                        }
                    }
                    SemanticValueBindingV1::Value { .. }
                    | SemanticValueBindingV1::IndexWitness { .. }
                    | SemanticValueBindingV1::BlockWitness { .. }
                    | SemanticValueBindingV1::GridLeader => Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic discriminant source is not a lowered option",
                    )),
                }
            }
            SemanticRvalueKindV1::Length(place) => {
                let (slice, ty) = self
                    .resolve_place(block, statement, place)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                if !matches!(ty, Type::Slice(_)) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic length source is not a lowered slice",
                    ));
                }
                self.emit(
                    operations,
                    Type::INDEX,
                    OperationKind::SliceLength { slice },
                )
            }
            SemanticRvalueKindV1::Unary { operation, operand } => {
                let input = self.lower_operand(block, statement, operand, operations)?;
                let (input, input_ty) = input
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                match operation {
                    SemanticUnaryOpV1::Not => self.emit(
                        operations,
                        input_ty,
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: input,
                        },
                    ),
                    SemanticUnaryOpV1::Negate => self.emit(
                        operations,
                        input_ty,
                        OperationKind::Unary {
                            op: UnaryOp::Negate,
                            operand: input,
                        },
                    ),
                    SemanticUnaryOpV1::PointerMetadata if matches!(input_ty, Type::Slice(_)) => {
                        self.emit(
                            operations,
                            Type::INDEX,
                            OperationKind::SliceLength { slice: input },
                        )
                    }
                    SemanticUnaryOpV1::PointerMetadata => Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "pointer metadata is available only for lowered slices",
                    )),
                }
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => {
                let (left, left_ty) = self
                    .lower_operand(block, statement, left, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let (right, right_ty) = self
                    .lower_operand(block, statement, right, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                if left_ty != right_ty {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic binary operand types differ",
                    ));
                }
                if let Some(predicate) = lower_compare(*operation) {
                    self.emit(
                        operations,
                        Type::BOOL,
                        OperationKind::Compare {
                            predicate,
                            lhs: left,
                            rhs: right,
                        },
                    )
                } else if let Some(operation) = lower_binary(*operation) {
                    self.emit(
                        operations,
                        left_ty,
                        OperationKind::Binary {
                            op: operation,
                            lhs: left,
                            rhs: right,
                        },
                    )
                } else {
                    Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic pointer offset requires an explicit GEP rule",
                    ))
                }
            }
            SemanticRvalueKindV1::Cast { kind, operand } => {
                let (input, input_ty) = self
                    .lower_operand(block, statement, operand, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let target = lower_scalar_type(self.types, result_type)?;
                if input_ty == target {
                    return Ok(SemanticValueBindingV1::Value {
                        id: input,
                        ty: input_ty,
                    });
                }
                let Some(kind) = lower_cast(*kind, &input_ty, &target) else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic cast has no exact Kernel IR cast rule",
                    ));
                };
                self.emit(
                    operations,
                    target.clone(),
                    OperationKind::Cast {
                        kind,
                        value: input,
                        to: target,
                    },
                )
            }
            SemanticRvalueKindV1::Load(load) if load.atomic().is_none() => {
                let (pointer, pointer_ty) = self
                    .resolve_place(block, statement, load.source())?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let Type::Pointer(pointer_type) = pointer_ty else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic load source is not a lowered pointer",
                    ));
                };
                let mut access = memory_access_for_type(
                    self.types,
                    load.source().ty(),
                    pointer_type.address_space,
                )?;
                access.volatile = load.volatility() == SemanticVolatilityV1::Volatile;
                self.emit(
                    operations,
                    (*pointer_type.pointee).clone(),
                    OperationKind::Load { pointer, access },
                )
            }
            _ => Err(unsupported(
                0,
                Some(block.index()),
                statement,
                unsupported_rvalue_detail(value),
            )),
        }
    }

    fn lower_operand(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.resolve_place(block, statement, place)
            }
            SemanticOperandV1::Constant(constant) => {
                let ty = lower_scalar_type(self.types, constant.ty())?;
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic constant is not a scalar",
                    ));
                };
                self.emit(
                    operations,
                    ty.clone(),
                    OperationKind::Constant(lower_constant(ty, *value)?),
                )
            }
        }
    }

    fn lower_terminator(
        &mut self,
        block: SemanticBlockIdV1,
        terminator: &SemanticTerminatorKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        match terminator {
            SemanticTerminatorKindV1::Goto(edge) => Ok(Terminator::Branch {
                target: BlockId(edge.target().index()),
                arguments: vec![],
            }),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                let (selector, _) = self
                    .lower_operand(block, None, discriminant, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                let cases = targets
                    .values()
                    .iter()
                    .map(|target| {
                        Ok(SwitchCase {
                            value: u64::try_from(target.value()).map_err(|_| {
                                unsupported(
                                    0,
                                    Some(block.index()),
                                    None,
                                    "switch value exceeds Kernel IR V1",
                                )
                            })?,
                            target: BlockId(target.edge().target().index()),
                            arguments: vec![],
                        })
                    })
                    .collect::<Result<Vec<_>, ProductionSemanticKirErrorV1>>()?;
                Ok(Terminator::Switch {
                    selector,
                    cases,
                    default_target: BlockId(targets.otherwise().target().index()),
                    default_arguments: vec![],
                })
            }
            SemanticTerminatorKindV1::Call(call) => self.lower_call(block, call, operations),
            SemanticTerminatorKindV1::Return => Ok(Terminator::Return { values: vec![] }),
            SemanticTerminatorKindV1::Unreachable => Ok(Terminator::Unreachable),
            _ => Err(unsupported(
                0,
                Some(block.index()),
                None,
                unsupported_terminator_detail(terminator),
            )),
        }
    }

    fn lower_call(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "trusted compiler intrinsic has a cleanup unwind edge",
            ));
        }
        let callable = self
            .callables
            .get(call.callee().index() as usize)
            .ok_or_else(|| unsupported(0, Some(block.index()), None, "callable is missing"))?;
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "defined and device-FFI calls require interprocedural lowering",
            ));
        };
        let destination = call.destination().ok_or_else(|| {
            unsupported(
                0,
                Some(block.index()),
                None,
                "compiler intrinsic call has no destination",
            )
        })?;
        let binding = match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => {
                if !call.arguments().is_empty() {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "thread index intrinsic has arguments",
                    ));
                }
                let (id, _) = self
                    .emit(
                        operations,
                        Type::INDEX,
                        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                    )?
                    .value()
                    .expect("emitted index value");
                SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                    disjoint: false,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. } => {
                self.require_call_argument_count(block, call, 1)?;
                self.lower_operand(block, None, &call.arguments()[0], operations)?
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                index_space, ..
            } => {
                self.require_call_argument_count(block, call, 1)?;
                let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space: actual,
                    disjoint: false,
                } = binding
                else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "into_disjoint receiver is not a thread-index witness",
                    ));
                };
                if actual != *index_space {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "into_disjoint mapping identity changed",
                    ));
                }
                SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space: actual,
                    disjoint: true,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                input_space,
                output_space,
                offset,
                ..
            } => self.lower_checked_shift(
                block,
                call,
                operations,
                *input_space,
                *output_space,
                *offset,
                false,
            )?,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                input_space,
                output_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => self.lower_checked_block(
                block,
                call,
                operations,
                *input_space,
                *output_space,
                *lanes_per_block,
                *elements_per_lane,
            )?,
            SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { index_space, .. } => {
                self.require_call_argument_count(block, call, 1)?;
                let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space: actual,
                    disjoint: true,
                } = binding
                else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "DisjointIndex::get receiver is not disjoint authority",
                    ));
                };
                if actual != *index_space {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "DisjointIndex::get mapping identity changed",
                    ));
                }
                SemanticValueBindingV1::Value {
                    id,
                    ty: Type::INDEX,
                }
            }
            SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                input_space,
                output_space,
                offset,
                ..
            } => self.lower_checked_shift(
                block,
                call,
                operations,
                *input_space,
                *output_space,
                *offset,
                true,
            )?,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex(axis) => {
                self.emit_index_intrinsic(operations, IndexKind::Local, lower_axis(*axis))?
            }
            SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(axis) => {
                self.emit_index_intrinsic(operations, IndexKind::Workgroup, lower_axis(*axis))?
            }
            SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(axis) => {
                self.emit_index_intrinsic(operations, IndexKind::WorkgroupSize, lower_axis(*axis))?
            }
            SemanticCompilerIntrinsicOperationV1::GridDimension(axis) => {
                self.emit_index_intrinsic(operations, IndexKind::WorkgroupCount, lower_axis(*axis))?
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. } => {
                self.require_call_argument_count(block, call, 2)?;
                let index_binding =
                    self.lower_operand(block, None, &call.arguments()[1], operations)?;
                if !matches!(
                    index_binding,
                    SemanticValueBindingV1::IndexWitness {
                        index_space: SemanticDisjointIndexSpaceV1::Index1d,
                        disjoint: false,
                        ..
                    }
                ) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "DisjointSlice::get_mut requires the identity thread-index witness",
                    ));
                }
                self.lower_checked_slice_access(block, call, operations, 0, index_binding, None)?
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                index_space,
                ..
            } => {
                self.require_call_argument_count(block, call, 2)?;
                let index_binding =
                    self.lower_operand(block, None, &call.arguments()[1], operations)?;
                if !matches!(index_binding, SemanticValueBindingV1::IndexWitness {
                    index_space: actual,
                    disjoint: true,
                    ..
                } if actual == *index_space)
                {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "get_disjoint_mut mapping authority does not match the slice",
                    ));
                }
                self.lower_checked_slice_access(block, call, operations, 0, index_binding, None)?
            }
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. } => {
                self.require_call_argument_count(block, call, 0)?;
                let (index, _) = self
                    .emit(
                        operations,
                        Type::INDEX,
                        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                    )?
                    .value()
                    .expect("emitted index value");
                let (one, _) = self
                    .emit(
                        operations,
                        Type::INDEX,
                        OperationKind::Constant(Constant::Index(1)),
                    )?
                    .value()
                    .expect("emitted index constant");
                let (present, _) = self
                    .emit(
                        operations,
                        Type::BOOL,
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            lhs: index,
                            rhs: one,
                        },
                    )?
                    .value()
                    .expect("emitted leader predicate");
                SemanticValueBindingV1::OptionGridLeader { present }
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. } => {
                self.require_call_argument_count(block, call, 3)?;
                let leader = self.lower_operand(block, None, &call.arguments()[1], operations)?;
                if !matches!(leader, SemanticValueBindingV1::GridLeader) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "exclusive access lacks grid-leader authority",
                    ));
                }
                let index = self.lower_operand(block, None, &call.arguments()[2], operations)?;
                let index = self.coerce_index(block, operations, index)?;
                self.lower_checked_slice_access(block, call, operations, 0, index, None)?
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                index_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                self.require_call_argument_count(block, call, 3)?;
                let witness = self.lower_operand(block, None, &call.arguments()[1], operations)?;
                let SemanticValueBindingV1::BlockWitness {
                    raw,
                    index_space: actual,
                } = witness
                else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "get_block_mut lacks blocked ownership authority",
                    ));
                };
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if actual != expected || *index_space != expected {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "get_block_mut mapping identity changed",
                    ));
                }
                let component =
                    self.lower_operand(block, None, &call.arguments()[2], operations)?;
                let component = self.coerce_index(block, operations, component)?;
                let (component, _) = component
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                let (index, present) = self.lower_block_component_index(
                    block,
                    operations,
                    raw,
                    component,
                    *lanes_per_block,
                    *elements_per_lane,
                )?;
                self.lower_checked_slice_access(
                    block,
                    call,
                    operations,
                    0,
                    SemanticValueBindingV1::Value {
                        id: index,
                        ty: Type::INDEX,
                    },
                    Some(present),
                )?
            }
            SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier
            | SemanticCompilerIntrinsicOperationV1::WaveBarrier
            | SemanticCompilerIntrinsicOperationV1::FabsF32 => {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "compiler intrinsic has no fill-profile lowering rule",
                ));
            }
        };
        self.bind_destination(block, None, destination.place(), binding)?;
        Ok(Terminator::Branch {
            target: BlockId(destination.edge().target().index()),
            arguments: vec![],
        })
    }

    fn require_call_argument_count(
        &self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        expected: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if call.arguments().len() == expected {
            Ok(())
        } else {
            Err(unsupported(
                0,
                Some(block.index()),
                None,
                "compiler intrinsic argument count changed",
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_checked_shift(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        input_space: SemanticDisjointIndexSpaceV1,
        output_space: SemanticDisjointIndexSpaceV1,
        offset: u64,
        input_is_disjoint: bool,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 1)?;
        let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
        let SemanticValueBindingV1::IndexWitness {
            id,
            index_space: actual,
            disjoint,
        } = binding
        else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_shift receiver is not index authority",
            ));
        };
        if actual != input_space || disjoint != input_is_disjoint {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_shift input mapping identity changed",
            ));
        }
        let expected_output = match input_space {
            SemanticDisjointIndexSpaceV1::Index1d => {
                SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset }
            }
            SemanticDisjointIndexSpaceV1::ShiftedIndex1d { .. }
            | SemanticDisjointIndexSpaceV1::BlockedIndex1d { .. }
            | SemanticDisjointIndexSpaceV1::GridExclusive => {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "checked_shift input mapping is unsupported",
                ));
            }
        };
        if output_space != expected_output {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_shift output mapping identity changed",
            ));
        }
        let (maximum, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(u64::MAX - offset)),
            )?
            .value()
            .expect("emitted index constant");
        let (present, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: id,
                    rhs: maximum,
                },
            )?
            .value()
            .expect("emitted checked-shift predicate");
        let (offset_value, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(offset)),
            )?
            .value()
            .expect("emitted index constant");
        let (shifted, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: id,
                    rhs: offset_value,
                },
            )?
            .value()
            .expect("emitted shifted index");
        Ok(SemanticValueBindingV1::OptionIndexWitness {
            present,
            id: shifted,
            index_space: output_space,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_checked_block(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        input_space: SemanticDisjointIndexSpaceV1,
        output_space: SemanticDisjointIndexSpaceV1,
        lanes_per_block: u64,
        elements_per_lane: u64,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 1)?;
        let input = self.lower_operand(block, None, &call.arguments()[0], operations)?;
        let SemanticValueBindingV1::IndexWitness {
            id: raw,
            index_space: actual,
            disjoint: false,
        } = input
        else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_block receiver is not thread-index authority",
            ));
        };
        let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
            lanes_per_block,
            elements_per_lane,
        };
        let Some(block_elements) = lanes_per_block.checked_mul(elements_per_lane) else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_block dimensions overflow",
            ));
        };
        if actual != input_space
            || input_space != SemanticDisjointIndexSpaceV1::Index1d
            || output_space != expected
            || lanes_per_block == 0
            || elements_per_lane == 0
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_block mapping identity is malformed",
            ));
        }
        let (lanes, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(lanes_per_block)),
            )?
            .value()
            .expect("emitted lanes constant");
        let (block_elements_value, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(block_elements)),
            )?
            .value()
            .expect("emitted block-elements constant");
        let (block_index, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: raw,
                    rhs: lanes,
                },
            )?
            .value()
            .expect("emitted block quotient");
        let (lane, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Remainder,
                    lhs: raw,
                    rhs: lanes,
                },
            )?
            .value()
            .expect("emitted lane remainder");
        let (maximum_block, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(u64::MAX / block_elements)),
            )?
            .value()
            .expect("emitted maximum block");
        let (block_safe, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: block_index,
                    rhs: maximum_block,
                },
            )?
            .value()
            .expect("emitted block overflow predicate");
        let (block_base, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs: block_index,
                    rhs: block_elements_value,
                },
            )?
            .value()
            .expect("emitted block base");
        let final_component_base = (elements_per_lane - 1) * lanes_per_block;
        let (final_component, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(final_component_base)),
            )?
            .value()
            .expect("emitted final-component base");
        let (final_offset, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: final_component,
                    rhs: lane,
                },
            )?
            .value()
            .expect("emitted final-component offset");
        let (final_index, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: block_base,
                    rhs: final_offset,
                },
            )?
            .value()
            .expect("emitted final blocked index");
        let (sum_safe, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: block_base,
                    rhs: final_index,
                },
            )?
            .value()
            .expect("emitted blocked sum predicate");
        let (present, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: block_safe,
                    rhs: sum_safe,
                },
            )?
            .value()
            .expect("emitted checked-block predicate");
        Ok(SemanticValueBindingV1::OptionBlockWitness {
            present,
            raw,
            index_space: expected,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_block_component_index(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        raw: ValueId,
        component: ValueId,
        lanes_per_block: u64,
        elements_per_lane: u64,
    ) -> Result<(ValueId, ValueId), ProductionSemanticKirErrorV1> {
        let Some(block_elements) = lanes_per_block.checked_mul(elements_per_lane) else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "blocked dimensions overflow during component projection",
            ));
        };
        if lanes_per_block == 0 || elements_per_lane == 0 {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "blocked dimensions are zero during component projection",
            ));
        }
        let mut constant = |value| {
            self.emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(value)),
            )
        };
        let (lanes, _) = constant(lanes_per_block)?
            .value()
            .expect("emitted lanes constant");
        let (elements, _) = constant(elements_per_lane)?
            .value()
            .expect("emitted elements constant");
        let (block_elements_value, _) = constant(block_elements)?
            .value()
            .expect("emitted block-elements constant");
        let (component_present, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: component,
                    rhs: elements,
                },
            )?
            .value()
            .expect("emitted component predicate");
        let (block_index, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: raw,
                    rhs: lanes,
                },
            )?
            .value()
            .expect("emitted block quotient");
        let (lane, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Remainder,
                    lhs: raw,
                    rhs: lanes,
                },
            )?
            .value()
            .expect("emitted lane remainder");
        let (block_base, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs: block_index,
                    rhs: block_elements_value,
                },
            )?
            .value()
            .expect("emitted block base");
        let (component_offset, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs: component,
                    rhs: lanes,
                },
            )?
            .value()
            .expect("emitted component offset");
        let (offset, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: component_offset,
                    rhs: lane,
                },
            )?
            .value()
            .expect("emitted blocked lane offset");
        let (index, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: block_base,
                    rhs: offset,
                },
            )?
            .value()
            .expect("emitted blocked component index");
        Ok((index, component_present))
    }

    fn coerce_index(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        binding: SemanticValueBindingV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let (id, ty) = binding
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
        if ty == Type::INDEX {
            return Ok(SemanticValueBindingV1::Value { id, ty });
        }
        if ty != Type::Scalar(ScalarType::U64) {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "exclusive access index is not usize",
            ));
        }
        self.emit(
            operations,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: id,
                to: Type::INDEX,
            },
        )
    }

    fn lower_checked_slice_access(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        receiver: usize,
        index: SemanticValueBindingV1,
        precondition: Option<ValueId>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let (slice, slice_ty) = self
            .lower_operand(block, None, &call.arguments()[receiver], operations)?
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
        let (index, index_ty) = index
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
        let Type::Slice(slice_type) = slice_ty else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked DisjointSlice receiver is not a lowered slice",
            ));
        };
        if index_ty != Type::INDEX {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked DisjointSlice index is not a trusted index",
            ));
        }
        let (length, _) = self
            .emit(
                operations,
                Type::INDEX,
                OperationKind::SliceLength { slice },
            )?
            .value()
            .expect("emitted scalar value");
        let (extent_present, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: index,
                    rhs: length,
                },
            )?
            .value()
            .expect("emitted scalar value");
        let present = if let Some(precondition) = precondition {
            self.emit(
                operations,
                Type::BOOL,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: precondition,
                    rhs: extent_present,
                },
            )?
            .value()
            .expect("emitted combined checked-access predicate")
            .0
        } else {
            extent_present
        };
        let pointer_ty = Type::pointer(
            (*slice_type.element).clone(),
            slice_type.address_space,
            slice_type.access,
        );
        let (base, _) = self
            .emit(
                operations,
                pointer_ty.clone(),
                OperationKind::SliceData { slice },
            )?
            .value()
            .expect("emitted scalar value");
        let (pointer, _) = self
            .emit(
                operations,
                pointer_ty.clone(),
                OperationKind::GetElementPointer {
                    base,
                    offset: index,
                },
            )?
            .value()
            .expect("emitted scalar value");
        Ok(SemanticValueBindingV1::OptionPointer {
            present,
            pointer,
            pointer_ty,
        })
    }

    fn emit_index_intrinsic(
        &mut self,
        operations: &mut Vec<Operation>,
        kind: IndexKind,
        axis: Axis,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.emit(
            operations,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex { kind, axis },
                Type::INDEX,
            )),
        )
    }

    fn assign_place(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        destination: &SemanticPlaceV1,
        value: SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if destination.projections().is_empty() {
            return self.bind_destination(block, statement, destination, value);
        }
        if !destination
            .projections()
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "projected local assignment is not a dereferenced store",
            ));
        }
        let (pointer, pointer_ty) = self
            .resolve_place(block, statement, destination)?
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
        let (value, value_ty) = value
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
        let Type::Pointer(pointer_type) = pointer_ty else {
            return Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "dereferenced store destination is not a lowered pointer",
            ));
        };
        if *pointer_type.pointee != value_ty {
            return Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "dereferenced store value type differs from its pointee",
            ));
        }
        let mut access =
            memory_access_for_type(self.types, destination.ty(), pointer_type.address_space)?;
        access.volatile = volatility == SemanticVolatilityV1::Volatile;
        operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer,
                value,
                access,
            },
        ));
        Ok(())
    }

    fn bind_destination(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        destination: &SemanticPlaceV1,
        value: SemanticValueBindingV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !destination.projections().is_empty() {
            return Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "semantic result destination is projected",
            ));
        }
        let index = self.require_local(block, statement, destination.local().index())?;
        self.locals[index] = Some(value);
        Ok(())
    }

    fn resolve_place(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let index = self.require_local(block, statement, place.local().index())?;
        let mut binding = self.locals[index].clone().ok_or(
            ProductionSemanticKirErrorV1::MissingLocalDefinition {
                function: 0,
                block: block.index(),
                statement,
                local: place.local().index(),
            },
        )?;
        for projection in place.projections() {
            binding = match (binding, projection.kind()) {
                (
                    SemanticValueBindingV1::OptionPointer {
                        pointer,
                        pointer_ty,
                        ..
                    },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::Value {
                    id: pointer,
                    ty: pointer_ty,
                },
                (
                    SemanticValueBindingV1::OptionIndexWitness {
                        id, index_space, ..
                    },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space,
                    disjoint: true,
                },
                (
                    SemanticValueBindingV1::OptionBlockWitness {
                        raw, index_space, ..
                    },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::BlockWitness { raw, index_space },
                (
                    SemanticValueBindingV1::OptionGridLeader { .. },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::GridLeader,
                (
                    binding @ SemanticValueBindingV1::OptionPointer { .. },
                    SemanticProjectionKindV1::Downcast(_),
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::OptionIndexWitness { .. },
                    SemanticProjectionKindV1::Downcast(_),
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::OptionGridLeader { .. },
                    SemanticProjectionKindV1::Downcast(_),
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::OptionBlockWitness { .. },
                    SemanticProjectionKindV1::Downcast(_),
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::Value { .. },
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::Field(0)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::IndexWitness { .. },
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::Field(0)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::GridLeader,
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::Field(0)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::BlockWitness { .. },
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::Field(0)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
                _ => {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic place projection has no exact fill-profile representation",
                    ));
                }
            };
        }
        Ok(binding)
    }

    fn emit(
        &mut self,
        operations: &mut Vec<Operation>,
        ty: Type,
        kind: OperationKind,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let id = ValueId(self.next_value);
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| unsupported(0, None, None, "Kernel IR SSA identity overflow"))?;
        operations.push(Operation::effect_free(ValueDef::new(id, ty.clone()), kind));
        Ok(SemanticValueBindingV1::Value { id, ty })
    }

    fn require_local(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: u32,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let index = usize::try_from(local).map_err(|_| {
            unsupported(
                0,
                Some(block.index()),
                statement,
                "local does not fit this host",
            )
        })?;
        if index >= self.function.locals().len() {
            Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "semantic local is out of range",
            ))
        } else {
            Ok(index)
        }
    }
}

fn unsupported_terminator_detail(terminator: &SemanticTerminatorKindV1) -> &'static str {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { .. } => {
            "semantic switch-int terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::Call(_) => {
            "semantic call terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::TailCall(_) => {
            "semantic tail-call terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::Drop { .. } => {
            "semantic drop terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::Assert { .. } => {
            "semantic assert terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            "semantic false-edge terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::UnwindResume => {
            "semantic unwind-resume terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::UnwindTerminate => {
            "semantic unwind-terminate terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::Abort => {
            "semantic abort terminator has no exact Kernel IR lowering rule"
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::Unreachable => {
            "internally supported semantic terminator reached unsupported diagnostics"
        }
    }
}

fn unsupported_statement_detail(statement: &SemanticStatementKindV1) -> Option<&'static str> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            Some(unsupported_rvalue_detail(assignment.value().kind()))
        }
        SemanticStatementKindV1::Store(_) => {
            Some("semantic store has no exact Kernel IR lowering rule")
        }
        SemanticStatementKindV1::AtomicRmw(_) => {
            Some("semantic atomic-rmw has no exact Kernel IR lowering rule")
        }
        SemanticStatementKindV1::AtomicCompareExchange(_) => {
            Some("semantic compare-exchange has no exact Kernel IR lowering rule")
        }
        SemanticStatementKindV1::SetDiscriminant { .. } => {
            Some("semantic set-discriminant has no exact Kernel IR lowering rule")
        }
        SemanticStatementKindV1::Deinitialize(_) => {
            Some("semantic deinitialize has no exact Kernel IR lowering rule")
        }
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => None,
    }
}

fn unsupported_rvalue_detail(value: &SemanticRvalueKindV1) -> &'static str {
    match value {
        SemanticRvalueKindV1::Use(_) => {
            "semantic assignment/use has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Unary { .. } => {
            "semantic assignment/unary has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Binary { .. } => {
            "semantic assignment/binary has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Cast { .. } => {
            "semantic assignment/cast has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Borrow { .. } => {
            "semantic assignment/borrow has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::AddressOf { .. } => {
            "semantic assignment/address-of has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Length(_) => {
            "semantic assignment/length has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Discriminant(_) => {
            "semantic assignment/discriminant has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Aggregate(_) => {
            "semantic assignment/aggregate has no exact Kernel IR lowering rule"
        }
        SemanticRvalueKindV1::Load(_) => {
            "semantic assignment/load has no exact Kernel IR lowering rule"
        }
    }
}

fn lower_parameter_type(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let shape = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "kernel argument type is missing"))?
        .shape();
    if let Some(element) = disjoint_slice_element(callables, ty) {
        return Ok(Type::slice(
            lower_scalar_type(types, element)?,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ));
    }
    match shape {
        SemanticTypeShapeV1::Pointer(pointer) => {
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            let address_space = lower_address_space(pointer.address_space())?;
            match pointer.metadata() {
                SemanticPointerMetadataV1::None => Ok(Type::pointer(
                    lower_scalar_type(types, pointer.pointee())?,
                    address_space,
                    access,
                )),
                SemanticPointerMetadataV1::SliceLength => {
                    let pointee =
                        types
                            .get(pointer.pointee().index() as usize)
                            .ok_or_else(|| {
                                unsupported(0, None, None, "slice pointee type is missing")
                            })?;
                    let SemanticTypeShapeV1::Slice { element } = pointee.shape() else {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "slice-length pointer metadata has a non-slice pointee",
                        ));
                    };
                    Ok(Type::slice(
                        lower_scalar_type(types, *element)?,
                        address_space,
                        access,
                    ))
                }
                SemanticPointerMetadataV1::VTable => Err(unsupported(
                    0,
                    None,
                    None,
                    "vtable-bearing kernel arguments are unsupported",
                )),
            }
        }
        SemanticTypeShapeV1::Scalar(_) => Ok(lower_scalar_type(types, ty)?),
        _ => Err(unsupported(
            0,
            None,
            None,
            "kernel argument type has no authenticated Kernel IR representation",
        )),
    }
}

fn lower_scalar_type(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let shape = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "scalar type is missing"))?
        .shape();
    let scalar = match shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => ScalarType::Bool,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => {
            match (*signed, *bits) {
                (true, 8) => ScalarType::I8,
                (true, 16) => ScalarType::I16,
                (true, 32) => ScalarType::I32,
                (true, 64) => ScalarType::I64,
                (true, 128) => ScalarType::I128,
                (false, 8) => ScalarType::U8,
                (false, 16) => ScalarType::U16,
                (false, 32) => ScalarType::U32,
                (false, 64) => ScalarType::U64,
                (false, 128) => ScalarType::U128,
                _ => {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "integer argument width is unsupported",
                    ));
                }
            }
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }) => match bits {
            16 => ScalarType::F16,
            32 => ScalarType::F32,
            64 => ScalarType::F64,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "floating argument width is unsupported",
                ));
            }
        },
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char) => ScalarType::U32,
        SemanticTypeShapeV1::ValidityScalar(validity) => {
            return lower_scalar_kind(validity.scalar());
        }
        _ => {
            return Err(unsupported(
                0,
                None,
                None,
                "referenced element type is not a supported scalar",
            ));
        }
    };
    Ok(Type::Scalar(scalar))
}

fn lower_scalar_kind(scalar: SemanticScalarTypeV1) -> Result<Type, ProductionSemanticKirErrorV1> {
    let scalar = match scalar {
        SemanticScalarTypeV1::Bool => ScalarType::Bool,
        SemanticScalarTypeV1::Integer { signed, bits } => match (signed, bits) {
            (true, 8) => ScalarType::I8,
            (true, 16) => ScalarType::I16,
            (true, 32) => ScalarType::I32,
            (true, 64) => ScalarType::I64,
            (true, 128) => ScalarType::I128,
            (false, 8) => ScalarType::U8,
            (false, 16) => ScalarType::U16,
            (false, 32) => ScalarType::U32,
            (false, 64) => ScalarType::U64,
            (false, 128) => ScalarType::U128,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "integer argument width is unsupported",
                ));
            }
        },
        SemanticScalarTypeV1::Float { bits } => match bits {
            16 => ScalarType::F16,
            32 => ScalarType::F32,
            64 => ScalarType::F64,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "floating argument width is unsupported",
                ));
            }
        },
        SemanticScalarTypeV1::Char => ScalarType::U32,
    };
    Ok(Type::Scalar(scalar))
}

fn disjoint_slice_element(
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    callables.iter().find_map(|callable| match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice,
                    element,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                    disjoint_slice,
                    element,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                    disjoint_slice,
                    element,
                    ..
                },
            ..
        } if *disjoint_slice == ty => Some(*element),
        SemanticCallableDeclV1::Defined { .. }
        | SemanticCallableDeclV1::DeviceFfiImport { .. }
        | SemanticCallableDeclV1::CompilerIntrinsic { .. } => None,
    })
}

fn lower_address_space(address_space: u32) -> Result<AddressSpace, ProductionSemanticKirErrorV1> {
    match address_space {
        0 | 1 => Ok(AddressSpace::Global),
        3 => Ok(AddressSpace::Workgroup),
        4 => Ok(AddressSpace::Constant),
        5 => Ok(AddressSpace::Private),
        _ => Err(unsupported(
            0,
            None,
            None,
            "semantic pointer address space is unsupported",
        )),
    }
}

const fn lower_axis(axis: SemanticAxisV1) -> Axis {
    match axis {
        SemanticAxisV1::X => Axis::X,
        SemanticAxisV1::Y => Axis::Y,
        SemanticAxisV1::Z => Axis::Z,
    }
}

const fn lower_compare(operation: SemanticBinaryOpV1) -> Option<ComparePredicate> {
    match operation {
        SemanticBinaryOpV1::Equal => Some(ComparePredicate::Equal),
        SemanticBinaryOpV1::NotEqual => Some(ComparePredicate::NotEqual),
        SemanticBinaryOpV1::LessThan => Some(ComparePredicate::LessThan),
        SemanticBinaryOpV1::LessOrEqual => Some(ComparePredicate::LessThanOrEqual),
        SemanticBinaryOpV1::GreaterThan => Some(ComparePredicate::GreaterThan),
        SemanticBinaryOpV1::GreaterOrEqual => Some(ComparePredicate::GreaterThanOrEqual),
        SemanticBinaryOpV1::Add
        | SemanticBinaryOpV1::Subtract
        | SemanticBinaryOpV1::Multiply
        | SemanticBinaryOpV1::Divide
        | SemanticBinaryOpV1::Remainder
        | SemanticBinaryOpV1::BitXor
        | SemanticBinaryOpV1::BitAnd
        | SemanticBinaryOpV1::BitOr
        | SemanticBinaryOpV1::ShiftLeft
        | SemanticBinaryOpV1::ShiftRight
        | SemanticBinaryOpV1::Offset => None,
    }
}

const fn lower_binary(operation: SemanticBinaryOpV1) -> Option<BinaryOp> {
    match operation {
        SemanticBinaryOpV1::Add => Some(BinaryOp::Add),
        SemanticBinaryOpV1::Subtract => Some(BinaryOp::Subtract),
        SemanticBinaryOpV1::Multiply => Some(BinaryOp::Multiply),
        SemanticBinaryOpV1::Divide => Some(BinaryOp::Divide),
        SemanticBinaryOpV1::Remainder => Some(BinaryOp::Remainder),
        SemanticBinaryOpV1::BitXor => Some(BinaryOp::BitXor),
        SemanticBinaryOpV1::BitAnd => Some(BinaryOp::BitAnd),
        SemanticBinaryOpV1::BitOr => Some(BinaryOp::BitOr),
        SemanticBinaryOpV1::ShiftLeft => Some(BinaryOp::ShiftLeft),
        SemanticBinaryOpV1::ShiftRight => Some(BinaryOp::ShiftRight),
        SemanticBinaryOpV1::Equal
        | SemanticBinaryOpV1::LessThan
        | SemanticBinaryOpV1::LessOrEqual
        | SemanticBinaryOpV1::NotEqual
        | SemanticBinaryOpV1::GreaterOrEqual
        | SemanticBinaryOpV1::GreaterThan
        | SemanticBinaryOpV1::Offset => None,
    }
}

fn lower_cast(kind: SemanticCastKindV1, from: &Type, to: &Type) -> Option<CastKind> {
    let (Some(from), Some(to)) = (from.as_scalar(), to.as_scalar()) else {
        return None;
    };
    let (from_width, to_width) = (from.bit_width()?, to.bit_width()?);
    match kind {
        SemanticCastKindV1::Integer if to.is_integer() => {
            if from.is_float() {
                Some(CastKind::FloatToInteger)
            } else if (from.is_integer() || from == ScalarType::Bool) && from_width > to_width {
                Some(CastKind::Truncate)
            } else if (from.is_integer() || from == ScalarType::Bool) && from_width < to_width {
                Some(if from.is_signed_integer() {
                    CastKind::SignExtend
                } else {
                    CastKind::ZeroExtend
                })
            } else {
                Some(CastKind::Bitcast)
            }
        }
        SemanticCastKindV1::Float if to.is_float() => {
            if from.is_integer() {
                Some(CastKind::IntegerToFloat)
            } else if from.is_float() && from_width < to_width {
                Some(CastKind::FloatExtend)
            } else if from.is_float() && from_width > to_width {
                Some(CastKind::FloatTruncate)
            } else {
                Some(CastKind::Bitcast)
            }
        }
        SemanticCastKindV1::Transmute if from_width == to_width => Some(CastKind::Bitcast),
        SemanticCastKindV1::Integer
        | SemanticCastKindV1::Float
        | SemanticCastKindV1::Pointer
        | SemanticCastKindV1::PointerExposeProvenance
        | SemanticCastKindV1::PointerWithExposedProvenance
        | SemanticCastKindV1::Transmute => None,
    }
}

fn lower_constant(
    ty: Type,
    value: SemanticScalarValueV1,
) -> Result<Constant, ProductionSemanticKirErrorV1> {
    let bits = value.bits();
    let constant = match ty.as_scalar() {
        Some(ScalarType::Bool) if value.size_bytes() == 1 && bits <= 1 => Constant::Bool(bits != 0),
        Some(ScalarType::I8) if value.size_bytes() == 1 => Constant::I8(bits as u8 as i8),
        Some(ScalarType::I16) if value.size_bytes() == 2 => Constant::I16(bits as u16 as i16),
        Some(ScalarType::I32) if value.size_bytes() == 4 => Constant::I32(bits as u32 as i32),
        Some(ScalarType::I64) if value.size_bytes() == 8 => Constant::I64(bits as u64 as i64),
        Some(ScalarType::U8) if value.size_bytes() == 1 => Constant::U8(bits as u8),
        Some(ScalarType::U16) if value.size_bytes() == 2 => Constant::U16(bits as u16),
        Some(ScalarType::U32) if value.size_bytes() == 4 => Constant::U32(bits as u32),
        Some(ScalarType::U64) if value.size_bytes() == 8 => Constant::U64(bits as u64),
        Some(ScalarType::Index) if value.size_bytes() == 8 => Constant::Index(bits as u64),
        Some(ScalarType::F16) if value.size_bytes() == 2 => Constant::F16Bits(bits as u16),
        Some(ScalarType::F32) if value.size_bytes() == 4 => Constant::F32Bits(bits as u32),
        Some(ScalarType::F64) if value.size_bytes() == 8 => Constant::F64Bits(bits as u64),
        Some(ScalarType::Bf16) if value.size_bytes() == 2 => Constant::Bf16Bits(bits as u16),
        Some(ScalarType::I128 | ScalarType::U128) => {
            return Err(unsupported(
                0,
                None,
                None,
                "128-bit constants have no Kernel IR V1 representation",
            ));
        }
        Some(_) | None => {
            return Err(unsupported(
                0,
                None,
                None,
                "semantic scalar constant size does not match its lowered type",
            ));
        }
    };
    Ok(constant)
}

fn memory_access_for_type(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    address_space: AddressSpace,
) -> Result<MemoryAccess, ProductionSemanticKirErrorV1> {
    let alignment = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "memory access type is missing"))?
        .layout()
        .alignment_bytes();
    let alignment = u32::try_from(alignment)
        .ok()
        .filter(|alignment| *alignment != 0)
        .ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "memory access alignment has no Kernel IR V1 representation",
            )
        })?;
    Ok(MemoryAccess::new(address_space, alignment))
}

fn enforce_limit(
    resource: ProductionSemanticKirResourceV1,
    actual: usize,
    limit: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if actual > limit {
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

const fn unsupported(
    function: u32,
    block: Option<u32>,
    statement: Option<u32>,
    detail: &'static str,
) -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::Unsupported {
        function,
        block,
        statement,
        detail,
    }
}

fn hex_identity(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}
