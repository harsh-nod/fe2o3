//! Owner-consuming production lowering from exact semantic MIR to Kernel IR.
//!
//! This boundary is intentionally fail-closed. It currently admits an
//! effect-free control-flow subset; each additional semantic operation must
//! arrive with an exact correspondence rule and Kernel IR verification. The
//! older detached `AlgorithmOp` marker pass is not used by this API.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, FunctionId, Kernel, LaunchDomain,
    LaunchExtent, Module, ScalarType, Signature, Terminator, Type, ValueId, VerificationErrors,
    WorkgroupSize, verify_module,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticFunctionIdV1, SemanticFunctionRoleV1, SemanticLocalRoleV1, SemanticMutabilityV1,
    SemanticPointerMetadataV1, SemanticScalarTypeV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
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
    erased_effect_free_statements: u32,
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

    /// Returns the number of explicitly classified effect-free statements erased.
    pub const fn erased_effect_free_statements(self) -> u32 {
        self.erased_effect_free_statements
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

    let mut order = Vec::with_capacity(function.blocks().len());
    order.push(function.entry());
    order.extend((0..function.blocks().len()).filter_map(|index| {
        let block = SemanticBlockIdV1::from_index(index as u32);
        (block != function.entry()).then_some(block)
    }));
    let mut blocks = Vec::with_capacity(order.len());
    let mut correspondence = Vec::with_capacity(order.len());
    for semantic_block in order {
        let index = usize::try_from(semantic_block.index())
            .map_err(|_| unsupported(0, None, None, "block identity does not fit this host"))?;
        let source = function.blocks().get(index).ok_or_else(|| {
            unsupported(0, Some(semantic_block.index()), None, "block is missing")
        })?;
        for (statement, operation) in source.statements().iter().enumerate() {
            if !matches!(
                operation.kind(),
                SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop
            ) {
                return Err(unsupported(
                    0,
                    Some(semantic_block.index()),
                    u32::try_from(statement).ok(),
                    "semantic statement has no exact Kernel IR lowering rule",
                ));
            }
        }
        let mut target = BasicBlock::new(BlockId(semantic_block.index()));
        target.terminator = Some(match source.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => Terminator::Branch {
                target: BlockId(edge.target().index()),
                arguments: vec![],
            },
            SemanticTerminatorKindV1::Return => Terminator::Return { values: vec![] },
            SemanticTerminatorKindV1::Unreachable => Terminator::Unreachable,
            _ => {
                return Err(unsupported(
                    0,
                    Some(semantic_block.index()),
                    None,
                    "semantic terminator has no exact Kernel IR lowering rule",
                ));
            }
        });
        blocks.push(target);
        correspondence.push(SemanticKirBlockCorrespondenceV1 {
            semantic_function: SemanticFunctionIdV1::from_index(0),
            semantic_block,
            kernel_ir_block: BlockId(semantic_block.index()),
            erased_effect_free_statements: u32::try_from(source.statements().len()).map_err(
                |_| {
                    unsupported(
                        0,
                        Some(semantic_block.index()),
                        None,
                        "statement count is too large",
                    )
                },
            )?,
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
