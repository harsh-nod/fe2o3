//! Owner-consuming production lowering from exact semantic MIR to Kernel IR.
//!
//! This boundary is intentionally fail-closed. It admits only operations with
//! explicit semantic correspondence rules, including trusted invocation
//! capabilities, structured control flow, typed memory access, synchronization,
//! and cooperative matrix operations. The resulting Kernel IR is verified
//! before release. Detached workload markers are not used by this API.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, Axis, BarrierSemantics, BasicBlock,
    BinaryOp, BlockId, CastKind, CheckedBinaryOperator, ComparePredicate, Constant, Convergence,
    F32MathFunction, FloatOperation, Function, FunctionId, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MAX_OPERATIONS_V1 as MAX_BLOCK_OPERATIONS_V1, MatrixOperation, MemoryAccess, MemoryOrdering,
    Module, Operation, OperationKind, ScalarType, Signature, SwitchCase, SynchronizationScope,
    Terminator, Type, UnaryOp, ValueDef, ValueId, VerificationErrors,
    VerifiedCanonicalKernelIrErrorV6, VerifiedCanonicalKernelIrIdentityV6,
    VerifiedCanonicalKernelIrV6, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
    WorkgroupSize, verify_module,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1, SemanticAxisV1, SemanticBinaryOpV1, SemanticBlockIdV1,
    SemanticCallableDeclV1, SemanticCastKindV1, SemanticCheckedBinaryOpV1,
    SemanticCompilerIntrinsicOperationV1, SemanticConstantValueV1, SemanticDirectCallV1,
    SemanticDisjointIndexSpaceV1, SemanticF32MathFunctionV1, SemanticFunctionDeclV1,
    SemanticFunctionIdV1, SemanticFunctionRoleV1, SemanticLocalRoleV1, SemanticMutabilityV1,
    SemanticOperandV1, SemanticPlaceV1, SemanticPointerMetadataV1, SemanticProjectionKindV1,
    SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticScalarValueV1, SemanticStatementKindV1,
    SemanticSubgroupReductionKindV1, SemanticTerminatorKindV1, SemanticTypeDeclV1,
    SemanticTypeIdV1, SemanticTypeShapeV1, SemanticUnaryOpV1, SemanticUnwindActionV1,
    SemanticVolatilityV1,
};
use fe2o3_mir_model::{
    SemanticOptionAvailabilityV1, SemanticOptionDominanceV1, semantic_option_producers_v1,
};
use fe2o3_pliron::{
    ProductionRankedKernelLoweringInputV1, ProductionSemanticMirErrorV1,
    ProductionSemanticMirOwnerV1,
};

const DEFAULT_MAX_FUNCTIONS_V1: usize = 1_024;
const DEFAULT_MAX_BLOCKS_V1: usize = 16_384;
const DEFAULT_MAX_STATEMENTS_V1: usize = 1_048_576;
const DEFAULT_MAX_OPERATIONS_V1: usize = 1_048_576;

/// Independent work limits for semantic-MIR-to-Kernel-IR lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticKirLimitsV1 {
    max_functions: usize,
    max_blocks: usize,
    max_statements: usize,
    max_operations: usize,
}

impl ProductionSemanticKirLimitsV1 {
    /// Constructs explicit lowering limits.
    pub const fn new(max_functions: usize, max_blocks: usize, max_statements: usize) -> Self {
        Self::new_with_max_operations(
            max_functions,
            max_blocks,
            max_statements,
            DEFAULT_MAX_OPERATIONS_V1,
        )
    }

    /// Constructs explicit lowering limits, including the module-wide emitted-operation budget.
    pub const fn new_with_max_operations(
        max_functions: usize,
        max_blocks: usize,
        max_statements: usize,
        max_operations: usize,
    ) -> Self {
        Self {
            max_functions,
            max_blocks,
            max_statements,
            max_operations,
        }
    }
}

impl Default for ProductionSemanticKirLimitsV1 {
    fn default() -> Self {
        Self::new_with_max_operations(
            DEFAULT_MAX_FUNCTIONS_V1,
            DEFAULT_MAX_BLOCKS_V1,
            DEFAULT_MAX_STATEMENTS_V1,
            DEFAULT_MAX_OPERATIONS_V1,
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
    /// Kernel IR operations emitted across all blocks.
    Operations,
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

/// Exact Kernel IR operation span emitted by one semantic MIR statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirStatementOperationSpanV1 {
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    statement_ordinal: u32,
    kernel_ir_block: BlockId,
    first_operation_ordinal: u32,
    operation_count: u32,
}

impl SemanticKirStatementOperationSpanV1 {
    /// Returns the exact semantic function locator.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the exact semantic block locator.
    pub const fn semantic_block(self) -> SemanticBlockIdV1 {
        self.semantic_block
    }

    /// Returns the zero-based statement ordinal within the semantic block.
    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }

    /// Returns the Kernel IR block that contains the emitted operations.
    pub const fn kernel_ir_block(self) -> BlockId {
        self.kernel_ir_block
    }

    /// Returns the zero-based ordinal of the first emitted operation.
    pub const fn first_operation_ordinal(self) -> u32 {
        self.first_operation_ordinal
    }

    /// Returns the exact number of emitted operations, including zero.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

/// Exact Kernel IR operation span emitted while lowering one semantic MIR terminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirTerminatorOperationSpanV1 {
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    kernel_ir_block: BlockId,
    first_operation_ordinal: u32,
    operation_count: u32,
}

impl SemanticKirTerminatorOperationSpanV1 {
    /// Returns the exact semantic function locator.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the exact semantic block locator.
    pub const fn semantic_block(self) -> SemanticBlockIdV1 {
        self.semantic_block
    }

    /// Returns the Kernel IR block that contains the emitted operations.
    pub const fn kernel_ir_block(self) -> BlockId {
        self.kernel_ir_block
    }

    /// Returns the zero-based ordinal of the first emitted operation.
    pub const fn first_operation_ordinal(self) -> u32 {
        self.first_operation_ordinal
    }

    /// Returns the exact number of operations emitted by the terminator.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

/// Closed lowering rule responsible for operations without a semantic MIR source construct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirSyntheticOperationRuleV1 {
    /// The canonical trap operation in the shared runtime-assert failure block.
    RuntimeAssertFailureTrap,
}

/// Exact Kernel IR operation span emitted by one typed synthetic lowering rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirSyntheticOperationSpanV1 {
    rule: SemanticKirSyntheticOperationRuleV1,
    kernel_ir_block: BlockId,
    first_operation_ordinal: u32,
    operation_count: u32,
}

impl SemanticKirSyntheticOperationSpanV1 {
    /// Returns the closed synthetic lowering rule.
    pub const fn rule(self) -> SemanticKirSyntheticOperationRuleV1 {
        self.rule
    }

    /// Returns the Kernel IR block that contains the synthetic operations.
    pub const fn kernel_ir_block(self) -> BlockId {
        self.kernel_ir_block
    }

    /// Returns the zero-based ordinal of the first synthetic operation.
    pub const fn first_operation_ordinal(self) -> u32 {
        self.first_operation_ordinal
    }

    /// Returns the exact number of operations emitted by the synthetic rule.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

/// Stable operation-attribution trace retained by one live lowering owner.
///
/// Span records identify which lowering invocation emitted each operation. They
/// do not independently prove that an operation implements its source
/// construct. Semantic authority remains with [`ProductionSemanticKirOwnerV1`],
/// whose equivalence check replays lowering and compares the complete module
/// and trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticKirCorrespondenceV1 {
    semantic_sha256: [u8; 32],
    function_count: usize,
    blocks: Box<[SemanticKirBlockCorrespondenceV1]>,
    statement_operation_spans: Box<[SemanticKirStatementOperationSpanV1]>,
    terminator_operation_spans: Box<[SemanticKirTerminatorOperationSpanV1]>,
    synthetic_operation_spans: Box<[SemanticKirSyntheticOperationSpanV1]>,
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

    /// Returns exact source-statement operation spans in lowering order.
    ///
    /// Zero-operation statements are represented by an explicit zero-length
    /// span at the current operation ordinal.
    pub fn statement_operation_spans(&self) -> &[SemanticKirStatementOperationSpanV1] {
        &self.statement_operation_spans
    }

    /// Returns exact source-terminator operation spans in lowering order.
    pub fn terminator_operation_spans(&self) -> &[SemanticKirTerminatorOperationSpanV1] {
        &self.terminator_operation_spans
    }

    /// Returns operation spans introduced by closed synthetic lowering rules.
    pub fn synthetic_operation_spans(&self) -> &[SemanticKirSyntheticOperationSpanV1] {
        &self.synthetic_operation_spans
    }

    fn validate_layout_against(
        &self,
        semantic_owner: &ProductionSemanticMirOwnerV1,
        module: &Module,
        discharge_ranked_bounds: bool,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        validate_semantic_kir_correspondence(semantic_owner, module, self, discharge_ranked_bounds)
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
    /// Storage for a bounded lowering resource could not be reserved.
    AllocationFailure {
        /// Resource whose bounded storage reservation failed.
        resource: ProductionSemanticKirResourceV1,
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
    /// The lowered module could not become exact verified canonical Kernel IR V6.
    CanonicalKernelIrV6(VerifiedCanonicalKernelIrErrorV6),
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
            Self::AllocationFailure { resource } => write!(
                formatter,
                "semantic-to-Kernel-IR could not reserve bounded {resource:?} storage",
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
            Self::CanonicalKernelIrV6(error) => {
                write!(
                    formatter,
                    "canonical Kernel IR V6 admission failed: {error}"
                )
            }
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
            Self::CanonicalKernelIrV6(error) => Some(error),
            Self::ResourceLimit { .. }
            | Self::AllocationFailure { .. }
            | Self::Unsupported { .. }
            | Self::MissingLocalDefinition { .. }
            | Self::CorrespondenceMismatch => None,
        }
    }
}

/// Move-only custody for one compiler-asserted semantic-to-ranked projection.
///
/// The public minting API is intentionally named as an internal compiler
/// assertion: this receipt prevents safe callers from later mixing independent
/// semantic, ranked-graph, and diagnostic-IR values, but it does not authenticate
/// a hostile caller that invokes the assertion with unrelated inputs.
#[must_use = "dropping the receipt abandons the checked semantic-to-ranked projection"]
pub struct ProductionRankedSemanticProjectionReceiptV1 {
    semantic: ProductionSemanticMirOwnerV1,
    lowering: ProductionRankedKernelLoweringInputV1,
    ranked_ir: String,
    semantic_sha256: [u8; 32],
    function_name: String,
}

impl fmt::Debug for ProductionRankedSemanticProjectionReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionRankedSemanticProjectionReceiptV1")
            .field("function_name", &self.function_name)
            .field("ranked_ir_bytes", &self.ranked_ir.len())
            .finish_non_exhaustive()
    }
}

impl ProductionRankedSemanticProjectionReceiptV1 {
    /// Asserts the result of the compiler's deterministic semantic projector.
    ///
    /// This is a compiler-internal trust assertion rather than public proof
    /// authentication. It verifies all cheaply reconstructible bindings and
    /// packages the three move-only inputs immediately at the projection
    /// boundary so downstream safe code cannot substitute one independently.
    #[doc(hidden)]
    pub fn assert_compiler_internal_projection(
        semantic: ProductionSemanticMirOwnerV1,
        lowering: ProductionRankedKernelLoweringInputV1,
        ranked_ir: String,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        semantic
            .verify_equivalence()
            .map_err(ProductionSemanticKirErrorV1::SemanticOwner)?;
        if !mandatory_generic_checks_are_clean(&lowering) {
            return Err(unsupported(
                0,
                None,
                None,
                "ranked projection receipt contains a rejected mandatory kernel check",
            ));
        }
        if ranked_ir.is_empty() {
            return Err(unsupported(
                0,
                None,
                None,
                "ranked projection receipt has empty diagnostic IR",
            ));
        }
        let document = semantic.semantic();
        let root = document
            .roots()
            .first()
            .and_then(|root| document.functions().get(root.index() as usize))
            .and_then(SemanticFunctionDeclV1::kernel_entry)
            .ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "ranked projection receipt has no exact kernel root",
                )
            })?;
        let function_name = std::str::from_utf8(root.export_symbol().as_bytes()).map_err(|_| {
            unsupported(
                0,
                None,
                None,
                "ranked projection receipt has a non-UTF-8 kernel symbol",
            )
        })?;
        if function_name != lowering.kernel().function_name() {
            return Err(unsupported(
                0,
                None,
                None,
                "ranked projection receipt function identity changed",
            ));
        }
        Ok(Self {
            semantic_sha256: *document.semantic_sha256().as_bytes(),
            function_name: function_name.to_owned(),
            semantic,
            lowering,
            ranked_ir,
        })
    }

    /// Borrows the exact semantic owner retained by this receipt.
    pub const fn semantic(&self) -> &ProductionSemanticMirOwnerV1 {
        &self.semantic
    }

    /// Borrows the owner-held ranked graph and mandatory check reports.
    pub const fn lowering(&self) -> &ProductionRankedKernelLoweringInputV1 {
        &self.lowering
    }

    /// Borrows the bounded diagnostic ranked IR emitted by the projector.
    pub fn ranked_ir(&self) -> &str {
        &self.ranked_ir
    }

    /// A projection receipt is custody only, never artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
/// Move-only owner of one exact semantic source and its verified Kernel IR.
#[must_use = "dropping the owner abandons the verified target-neutral lowering"]
pub struct ProductionSemanticKirOwnerV1 {
    semantic: ProductionSemanticMirOwnerV1,
    module: Module,
    canonical_kernel_ir_v6: VerifiedCanonicalKernelIrV6,
    correspondence: SemanticKirCorrespondenceV1,
    limits: ProductionSemanticKirLimitsV1,
    discharge_ranked_bounds: bool,
    generic_checks: Option<RetainedGenericKernelChecksV1>,
}

struct RetainedGenericKernelChecksV1 {
    semantic_sha256: [u8; 32],
    function_name: String,
    ranked_ir: Box<str>,
    lowering: ProductionRankedKernelLoweringInputV1,
}

impl fmt::Debug for ProductionSemanticKirOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSemanticKirOwnerV1")
            .field("module", &self.module.id)
            .field(
                "canonical_kernel_ir_v6_identity",
                self.canonical_kernel_ir_v6.identity(),
            )
            .field("correspondence", &self.correspondence)
            .field("limits", &self.limits)
            .field("discharge_ranked_bounds", &self.discharge_ranked_bounds)
            .field("retains_generic_checks", &self.generic_checks.is_some())
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
        let (module, correspondence) = lower_module(&semantic, limits, false)?;
        let canonical_kernel_ir_v6 = VerifiedCanonicalKernelIrV6::from_module(module.clone())
            .map_err(ProductionSemanticKirErrorV1::CanonicalKernelIrV6)?;
        let owner = Self {
            semantic,
            module,
            canonical_kernel_ir_v6,
            correspondence,
            limits,
            discharge_ranked_bounds: false,
            generic_checks: None,
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Constructs Kernel IR while retaining the exact ranked graph and every
    /// mandatory generic-check report that admitted the same semantic owner.
    pub fn try_lower_after_ranked_checks(
        receipt: ProductionRankedSemanticProjectionReceiptV1,
        limits: ProductionSemanticKirLimitsV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let ProductionRankedSemanticProjectionReceiptV1 {
            semantic,
            lowering,
            ranked_ir,
            semantic_sha256,
            function_name,
        } = receipt;
        semantic
            .verify_equivalence()
            .map_err(ProductionSemanticKirErrorV1::SemanticOwner)?;
        if !mandatory_generic_checks_are_clean(&lowering) {
            return Err(unsupported(
                0,
                None,
                None,
                "ranked proof custody contains a rejected mandatory kernel check",
            ));
        }
        let (module, correspondence) = lower_module(&semantic, limits, true)?;
        let canonical_kernel_ir_v6 = VerifiedCanonicalKernelIrV6::from_module(module.clone())
            .map_err(ProductionSemanticKirErrorV1::CanonicalKernelIrV6)?;
        let owner = Self {
            semantic,
            module,
            canonical_kernel_ir_v6,
            correspondence,
            limits,
            discharge_ranked_bounds: true,
            generic_checks: Some(RetainedGenericKernelChecksV1 {
                semantic_sha256,
                function_name,
                ranked_ir: ranked_ir.into_boxed_str(),
                lowering,
            }),
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Re-verifies semantic ownership, Kernel IR, and retained correspondence.
    pub fn verify_equivalence(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        self.semantic
            .verify_equivalence()
            .map_err(ProductionSemanticKirErrorV1::SemanticOwner)?;
        self.canonical_kernel_ir_v6
            .revalidate()
            .map_err(ProductionSemanticKirErrorV1::CanonicalKernelIrV6)?;
        verify_module(&self.module).map_err(ProductionSemanticKirErrorV1::InvalidKernelIr)?;
        let (rederived_module, rederived_correspondence) =
            lower_module(&self.semantic, self.limits, self.discharge_ranked_bounds)?;
        let rederived_canonical_kernel_ir_v6 =
            VerifiedCanonicalKernelIrV6::from_module(rederived_module.clone())
                .map_err(ProductionSemanticKirErrorV1::CanonicalKernelIrV6)?;
        if self.module != rederived_module
            || self.correspondence != rederived_correspondence
            || self.canonical_kernel_ir_v6.identity() != rederived_canonical_kernel_ir_v6.identity()
            || self.canonical_kernel_ir_v6.canonical_bytes()
                != rederived_canonical_kernel_ir_v6.canonical_bytes()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if let Some(generic_checks) = &self.generic_checks {
            let Some(function) = self.module.functions.first() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if generic_checks.semantic_sha256 != self.correspondence.semantic_sha256
                || generic_checks.function_name != function.id.as_str()
                || generic_checks.ranked_ir.is_empty()
                || !mandatory_generic_checks_are_clean(&generic_checks.lowering)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
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

    /// Borrows the authoritative exact, semantically verified Kernel IR V6 bytes.
    pub const fn canonical_kernel_ir_v6(&self) -> &VerifiedCanonicalKernelIrV6 {
        &self.canonical_kernel_ir_v6
    }

    /// Borrows the typed identity of the authoritative canonical Kernel IR V6 bytes.
    pub const fn canonical_kernel_ir_v6_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV6 {
        self.canonical_kernel_ir_v6.identity()
    }

    /// Borrows pointer-independent source correspondence evidence.
    pub const fn correspondence(&self) -> &SemanticKirCorrespondenceV1 {
        &self.correspondence
    }

    /// Reports whether mandatory ranked checks remain owned by this lowering.
    pub const fn retains_mandatory_generic_checks(&self) -> bool {
        self.generic_checks.is_some()
    }

    pub(crate) fn retained_generic_checks_discharge_dynamic_indices(&self) -> bool {
        self.generic_checks
            .as_ref()
            .is_some_and(|checks| mandatory_generic_checks_are_clean(&checks.lowering))
    }
    /// Exact target-neutral lowering evidence is not artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn mandatory_generic_checks_are_clean(lowering: &ProductionRankedKernelLoweringInputV1) -> bool {
    lowering.bounds_report().is_clean()
        && lowering.atomic_report().is_clean()
        && lowering.race_report().is_clean()
        && lowering.barrier_report().is_clean()
        && lowering.workgroup_report().is_clean()
        && lowering.semantic_report().is_clean()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedSemanticKirBlockCoverageV1 {
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    kernel_ir_block: BlockId,
    source_statement_count: u32,
}

fn validate_semantic_kir_correspondence(
    owner: &ProductionSemanticMirOwnerV1,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    discharge_ranked_bounds: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let semantic = owner.semantic();
    if correspondence.semantic_sha256 != *semantic.semantic_sha256().as_bytes()
        || correspondence.function_count != semantic.functions().len()
        || semantic.functions().len() != 1
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let function = &semantic.functions()[0];
    let synthetic_rule =
        semantic_requires_runtime_assert_failure(function, discharge_ranked_bounds)
            .then_some(SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap);
    let order = semantic_cfg_preorder(function)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let expected = order
        .into_iter()
        .map(|semantic_block| {
            let block_index = usize::try_from(semantic_block.index())
                .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let source = function
                .blocks()
                .get(block_index)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            Ok(ExpectedSemanticKirBlockCoverageV1 {
                semantic_function: SemanticFunctionIdV1::from_index(0),
                semantic_block,
                kernel_ir_block: BlockId(semantic_block.index()),
                source_statement_count: u32::try_from(source.statements().len())
                    .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            })
        })
        .collect::<Result<Vec<_>, ProductionSemanticKirErrorV1>>()?;
    let mut function_bodies = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref());
    let target_blocks = function_bodies
        .next()
        .map(|body| body.blocks.as_slice())
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if function_bodies.next().is_some() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    if validate_operation_correspondence_layout(
        &expected,
        target_blocks,
        &correspondence.blocks,
        &correspondence.statement_operation_spans,
        &correspondence.terminator_operation_spans,
        &correspondence.synthetic_operation_spans,
        synthetic_rule,
    ) {
        Ok(())
    } else {
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }
}

fn validate_operation_correspondence_layout(
    expected: &[ExpectedSemanticKirBlockCoverageV1],
    target_blocks: &[BasicBlock],
    blocks: &[SemanticKirBlockCorrespondenceV1],
    statements: &[SemanticKirStatementOperationSpanV1],
    terminators: &[SemanticKirTerminatorOperationSpanV1],
    synthetic: &[SemanticKirSyntheticOperationSpanV1],
    synthetic_rule: Option<SemanticKirSyntheticOperationRuleV1>,
) -> bool {
    let expected_synthetic_count = usize::from(synthetic_rule.is_some());
    let Some(expected_target_blocks) = expected.len().checked_add(expected_synthetic_count) else {
        return false;
    };
    if blocks.len() != expected.len()
        || terminators.len() != expected.len()
        || synthetic.len() != expected_synthetic_count
        || target_blocks.len() != expected_target_blocks
    {
        return false;
    }

    let mut statement_index = 0_usize;
    for (block_index, expected_block) in expected.iter().enumerate() {
        let Some(target) = target_blocks.get(block_index) else {
            return false;
        };
        let expected_block_record = SemanticKirBlockCorrespondenceV1 {
            semantic_function: expected_block.semantic_function,
            semantic_block: expected_block.semantic_block,
            kernel_ir_block: expected_block.kernel_ir_block,
            source_statement_count: expected_block.source_statement_count,
        };
        if blocks.get(block_index) != Some(&expected_block_record)
            || target.id != expected_block.kernel_ir_block
            || target.terminator.is_none()
        {
            return false;
        }

        let mut next_operation = 0_usize;
        for statement_ordinal in 0..expected_block.source_statement_count {
            let Some(span) = statements.get(statement_index) else {
                return false;
            };
            if span.semantic_function != expected_block.semantic_function
                || span.semantic_block != expected_block.semantic_block
                || span.statement_ordinal != statement_ordinal
                || span.kernel_ir_block != expected_block.kernel_ir_block
                || usize::try_from(span.first_operation_ordinal) != Ok(next_operation)
            {
                return false;
            }
            let Some(end) = checked_operation_span_end(
                span.first_operation_ordinal,
                span.operation_count,
                target.operations.len(),
            ) else {
                return false;
            };
            next_operation = end;
            statement_index += 1;
        }

        let Some(terminator) = terminators.get(block_index) else {
            return false;
        };
        if terminator.semantic_function != expected_block.semantic_function
            || terminator.semantic_block != expected_block.semantic_block
            || terminator.kernel_ir_block != expected_block.kernel_ir_block
            || usize::try_from(terminator.first_operation_ordinal) != Ok(next_operation)
        {
            return false;
        }
        let Some(end) = checked_operation_span_end(
            terminator.first_operation_ordinal,
            terminator.operation_count,
            target.operations.len(),
        ) else {
            return false;
        };
        if end != target.operations.len() {
            return false;
        }
    }
    if statement_index != statements.len() {
        return false;
    }

    let canonical_trap = AmdGpuDiagnosticOperation::Trap.operation(None);
    for (synthetic_index, span) in synthetic.iter().enumerate() {
        let Some(target) = target_blocks.get(expected.len() + synthetic_index) else {
            return false;
        };
        if Some(span.rule) != synthetic_rule
            || span.kernel_ir_block != target.id
            || span.first_operation_ordinal != 0
            || span.operation_count != 1
            || target.operations.as_slice() != [canonical_trap.clone()]
            || !matches!(target.terminator.as_ref(), Some(Terminator::Unreachable))
        {
            return false;
        }
    }
    true
}

fn checked_operation_span_end(first: u32, count: u32, operation_len: usize) -> Option<usize> {
    let first = usize::try_from(first).ok()?;
    let count = usize::try_from(count).ok()?;
    first.checked_add(count).filter(|end| *end <= operation_len)
}

fn measured_operation_span(
    first: usize,
    after: usize,
    block: BlockId,
    statement: Option<u32>,
) -> Result<(u32, u32), ProductionSemanticKirErrorV1> {
    let count = after.checked_sub(first).ok_or_else(|| {
        unsupported(
            0,
            Some(block.0),
            statement,
            "Kernel IR operation count moved backwards during lowering",
        )
    })?;
    Ok((
        u32::try_from(first).map_err(|_| {
            unsupported(
                0,
                Some(block.0),
                statement,
                "Kernel IR operation ordinal is too large",
            )
        })?,
        u32::try_from(count).map_err(|_| {
            unsupported(
                0,
                Some(block.0),
                statement,
                "Kernel IR operation span is too large",
            )
        })?,
    ))
}

fn semantic_requires_runtime_assert_failure(
    function: &SemanticFunctionDeclV1,
    discharge_ranked_bounds: bool,
) -> bool {
    function
        .blocks()
        .iter()
        .any(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Assert {
                message: SemanticAssertMessageV1::BoundsCheck { .. },
                ..
            } if discharge_ranked_bounds => false,
            SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::UnwindTerminate => true,
            _ => false,
        })
}

fn lower_module(
    owner: &ProductionSemanticMirOwnerV1,
    limits: ProductionSemanticKirLimitsV1,
    discharge_ranked_bounds: bool,
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

    let has_runtime_assert =
        semantic_requires_runtime_assert_failure(function, discharge_ranked_bounds);
    let lowered_block_count = function
        .blocks()
        .len()
        .checked_add(usize::from(has_runtime_assert))
        .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: usize::MAX,
            limit: limits.max_blocks,
        })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Blocks,
        lowered_block_count,
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
        SemanticParameterBindingsV1 {
            declarations: &parameters,
            values: &parameter_values,
            types: &parameter_types,
        },
        has_runtime_assert.then(|| BlockId(function.blocks().len() as u32)),
        discharge_ranked_bounds,
        limits.max_operations,
    )?;

    let order = semantic_cfg_preorder(function)?;
    let mut blocks = Vec::with_capacity(order.len());
    let mut correspondence = Vec::with_capacity(order.len());
    let mut statement_operation_spans = Vec::with_capacity(statement_count);
    let mut terminator_operation_spans = Vec::with_capacity(order.len());
    let mut synthetic_operation_spans = Vec::with_capacity(usize::from(has_runtime_assert));
    for semantic_block in order {
        let index = usize::try_from(semantic_block.index())
            .map_err(|_| unsupported(0, None, None, "block identity does not fit this host"))?;
        let source = function.blocks().get(index).ok_or_else(|| {
            unsupported(0, Some(semantic_block.index()), None, "block is missing")
        })?;
        let mut target = BasicBlock::new(BlockId(semantic_block.index()));
        lowering.begin_block(semantic_block, &mut target)?;
        for (statement, operation) in source.statements().iter().enumerate() {
            let statement = u32::try_from(statement).map_err(|_| {
                unsupported(
                    0,
                    Some(semantic_block.index()),
                    None,
                    "statement ordinal is too large",
                )
            })?;
            let first = target.operations.len();
            lowering.lower_statement(
                semantic_block,
                Some(statement),
                operation.kind(),
                &mut target.operations,
            )?;
            let (first_operation_ordinal, operation_count) = measured_operation_span(
                first,
                target.operations.len(),
                target.id,
                Some(statement),
            )?;
            statement_operation_spans.push(SemanticKirStatementOperationSpanV1 {
                semantic_function: SemanticFunctionIdV1::from_index(0),
                semantic_block,
                statement_ordinal: statement,
                kernel_ir_block: target.id,
                first_operation_ordinal,
                operation_count,
            });
        }
        let terminator_first = target.operations.len();
        target.terminator = Some(lowering.lower_terminator(
            semantic_block,
            source.terminator().kind(),
            &mut target.operations,
        )?);
        let (first_operation_ordinal, operation_count) =
            measured_operation_span(terminator_first, target.operations.len(), target.id, None)?;
        terminator_operation_spans.push(SemanticKirTerminatorOperationSpanV1 {
            semantic_function: SemanticFunctionIdV1::from_index(0),
            semantic_block,
            kernel_ir_block: target.id,
            first_operation_ordinal,
            operation_count,
        });
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
    if let Some(failure_block) = lowering.assert_failure_block {
        let mut block = BasicBlock::new(failure_block);
        let first = block.operations.len();
        lowering.push_operation(&mut block.operations, || {
            AmdGpuDiagnosticOperation::Trap.operation(None)
        })?;
        let (first_operation_ordinal, operation_count) =
            measured_operation_span(first, block.operations.len(), failure_block, None)?;
        synthetic_operation_spans.push(SemanticKirSyntheticOperationSpanV1 {
            rule: SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
            kernel_ir_block: failure_block,
            first_operation_ordinal,
            operation_count,
        });
        block.terminator = Some(Terminator::Unreachable);
        blocks.push(block);
    }
    let operation_capabilities = blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .flat_map(Operation::required_capabilities)
        .collect::<BTreeSet<_>>();
    let float_declarations = blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, arguments } => {
                FloatOperation::from_intrinsic_call(callee, arguments)
            }
            _ => None,
        })
        .map(|operation| {
            let declaration = operation.declaration();
            (declaration.id.clone(), declaration)
        })
        .collect::<BTreeMap<_, _>>();

    let function_id = FunctionId::new(symbol);
    let mut module = Module::new(format!(
        "fe2o3::semantic::{}",
        hex_identity(semantic.semantic_sha256().as_bytes())
    ));
    let trap = AmdGpuDiagnosticOperation::Trap;
    if has_runtime_assert {
        module
            .required_capabilities
            .extend(trap.required_capabilities());
    }
    module
        .required_capabilities
        .extend(operation_capabilities.iter().cloned());
    let mut entry_function = Function::kernel_entry(
        function_id.clone(),
        Signature::new(parameter_types, vec![]),
        parameter_values,
        blocks,
    );
    if has_runtime_assert {
        entry_function
            .required_capabilities
            .extend(trap.required_capabilities());
    }
    entry_function
        .required_capabilities
        .extend(operation_capabilities.iter().cloned());
    module.functions.push(entry_function);
    module.functions.extend(float_declarations.into_values());
    if has_runtime_assert {
        module.functions.push(trap.declaration());
    }
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
    if has_runtime_assert {
        kernel
            .required_capabilities
            .extend(trap.required_capabilities());
    }
    kernel.required_capabilities.extend(operation_capabilities);
    module.kernels.push(kernel);

    let correspondence = SemanticKirCorrespondenceV1 {
        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
        function_count: semantic.functions().len(),
        blocks: correspondence.into_boxed_slice(),
        statement_operation_spans: statement_operation_spans.into_boxed_slice(),
        terminator_operation_spans: terminator_operation_spans.into_boxed_slice(),
        synthetic_operation_spans: synthetic_operation_spans.into_boxed_slice(),
    };
    correspondence.validate_layout_against(owner, &module, discharge_ranked_bounds)?;
    Ok((module, correspondence))
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

const MAX_PROMOTED_LOCALS_V1: usize = 128;
const MAX_PROMOTED_BLOCK_PARAMETERS_V1: usize = 16_384;

#[derive(Clone, Debug)]
struct SemanticPromotedLocalV1 {
    semantic_type: SemanticTypeIdV1,
    kernel_types: Box<[Type]>,
}

#[derive(Clone, Debug, Default)]
struct SemanticControlFlowSsaPlanV1 {
    promoted: BTreeMap<u32, SemanticPromotedLocalV1>,
    live_in: BTreeMap<u32, Vec<u32>>,
}

impl SemanticControlFlowSsaPlanV1 {
    fn analyze(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut definition_counts = vec![0_u32; function.locals().len()];
        let mut projected = BTreeSet::new();
        for block in function.blocks() {
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                    let destination = assignment.destination();
                    if destination.projections().is_empty() {
                        let count = definition_counts
                            .get_mut(destination.local().index() as usize)
                            .ok_or_else(|| {
                                unsupported(0, None, None, "assignment local is missing")
                            })?;
                        *count = count.saturating_add(1);
                    } else {
                        projected.insert(destination.local().index());
                    }
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
            {
                if destination.place().projections().is_empty() {
                    let count = definition_counts
                        .get_mut(destination.place().local().index() as usize)
                        .ok_or_else(|| {
                            unsupported(0, None, None, "call destination local is missing")
                        })?;
                    *count = count.saturating_add(1);
                } else {
                    projected.insert(destination.place().local().index());
                }
            }
        }

        let mut promoted = BTreeMap::new();
        for (local, declaration) in function.locals().iter().enumerate() {
            if definition_counts[local] < 2 || projected.contains(&(local as u32)) {
                continue;
            }
            if let Ok(kernel_types) = lower_ssa_value_types(types, declaration.ty())
                && !kernel_types.is_empty()
            {
                promoted.insert(
                    local as u32,
                    SemanticPromotedLocalV1 {
                        semantic_type: declaration.ty(),
                        kernel_types: kernel_types.into_boxed_slice(),
                    },
                );
            }
        }
        if promoted.is_empty() {
            return Ok(Self::default());
        }
        if promoted.len() > MAX_PROMOTED_LOCALS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "mutable control flow exceeds the promoted-local limit",
            ));
        }

        let block_ids = (0..function.blocks().len())
            .map(|block| block as u32)
            .collect::<BTreeSet<_>>();
        let mut uses = BTreeMap::<u32, BTreeSet<u32>>::new();
        let mut defs = BTreeMap::<u32, BTreeSet<u32>>::new();
        let mut successors = BTreeMap::<u32, Vec<u32>>::new();
        for (block_index, block) in function.blocks().iter().enumerate() {
            let block_id = block_index as u32;
            let mut block_uses = BTreeSet::new();
            let mut block_defs = BTreeSet::new();
            for statement in block.statements() {
                collect_statement_uses_v1(
                    statement.kind(),
                    &promoted,
                    &block_defs,
                    &mut block_uses,
                );
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().projections().is_empty()
                    && promoted.contains_key(&assignment.destination().local().index())
                {
                    block_defs.insert(assignment.destination().local().index());
                }
            }
            collect_terminator_uses_v1(
                block.terminator().kind(),
                &promoted,
                &block_defs,
                &mut block_uses,
            );
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.place().projections().is_empty()
                && promoted.contains_key(&destination.place().local().index())
            {
                block_defs.insert(destination.place().local().index());
            }
            let mut block_successors = Vec::new();
            block
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    if !block_ids.contains(&edge.target().index()) {
                        return Err(unsupported(
                            0,
                            Some(block_id),
                            None,
                            "CFG successor is missing",
                        ));
                    }
                    block_successors.push(edge.target().index());
                    Ok(())
                })?;
            uses.insert(block_id, block_uses);
            defs.insert(block_id, block_defs);
            successors.insert(block_id, block_successors);
        }

        let mut live_in = function
            .blocks()
            .iter()
            .enumerate()
            .map(|(block, _)| (block as u32, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut predecessors = function
            .blocks()
            .iter()
            .enumerate()
            .map(|(block, _)| (block as u32, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for (source, targets) in &successors {
            for target in targets {
                predecessors
                    .get_mut(target)
                    .expect("validated successor")
                    .insert(*source);
            }
        }
        let mut worklist = (0..function.blocks().len() as u32).collect::<VecDeque<_>>();
        let mut queued = (0..function.blocks().len() as u32).collect::<BTreeSet<_>>();
        while let Some(block_id) = worklist.pop_front() {
            queued.remove(&block_id);
            let live_out = successors[&block_id]
                .iter()
                .flat_map(|target| live_in[target].iter().copied())
                .collect::<BTreeSet<_>>();
            let mut next = uses[&block_id].clone();
            next.extend(live_out.difference(&defs[&block_id]).copied());
            if next != live_in[&block_id] {
                live_in.insert(block_id, next);
                for predecessor in &predecessors[&block_id] {
                    if queued.insert(*predecessor) {
                        worklist.push_back(*predecessor);
                    }
                }
            }
        }

        let entry = function.entry().index();
        if live_in[&entry].iter().any(|local| {
            !matches!(
                function.locals()[*local as usize].role(),
                SemanticLocalRoleV1::Argument(_)
            )
        }) {
            return Err(unsupported(
                0,
                Some(entry),
                None,
                "mutable scalar control flow reads a local before its entry definition",
            ));
        }
        let parameter_count = live_in
            .iter()
            .filter(|(block, _)| **block != entry)
            .flat_map(|(_, locals)| locals)
            .map(|local| promoted[local].kernel_types.len())
            .sum::<usize>();
        if parameter_count > MAX_PROMOTED_BLOCK_PARAMETERS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "mutable scalar control flow exceeds the block-parameter limit",
            ));
        }
        for (block, targets) in &successors {
            let mut seen = BTreeSet::new();
            for target in targets {
                if !seen.insert(*target) && !live_in[target].is_empty() {
                    return Err(unsupported(
                        0,
                        Some(*block),
                        None,
                        "multiple live-value edges from one block to one successor are unsupported",
                    ));
                }
            }
        }

        Ok(Self {
            promoted,
            live_in: live_in
                .into_iter()
                .map(|(block, locals)| (block, locals.into_iter().collect()))
                .collect(),
        })
    }

    fn live_in(&self, block: u32) -> &[u32] {
        self.live_in.get(&block).map_or(&[], Vec::as_slice)
    }
}

fn collect_statement_uses_v1(
    statement: &SemanticStatementKindV1,
    promoted: &BTreeMap<u32, SemanticPromotedLocalV1>,
    defs: &BTreeSet<u32>,
    uses: &mut BTreeSet<u32>,
) {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            collect_rvalue_uses_v1(assignment.value().kind(), promoted, defs, uses);
            if !assignment.destination().projections().is_empty() {
                collect_place_use_v1(assignment.destination(), promoted, defs, uses);
            }
        }
        SemanticStatementKindV1::Store(store) => {
            collect_place_use_v1(store.destination(), promoted, defs, uses);
            collect_operand_use_v1(store.value(), promoted, defs, uses);
        }
        _ => {}
    }
}

fn collect_rvalue_uses_v1(
    value: &SemanticRvalueKindV1,
    promoted: &BTreeMap<u32, SemanticPromotedLocalV1>,
    defs: &BTreeSet<u32>,
    uses: &mut BTreeSet<u32>,
) {
    match value {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => {
            collect_operand_use_v1(operand, promoted, defs, uses);
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            collect_operand_use_v1(left, promoted, defs, uses);
            collect_operand_use_v1(right, promoted, defs, uses);
        }
        SemanticRvalueKindV1::CheckedBinary(checked) => {
            collect_operand_use_v1(checked.left(), promoted, defs, uses);
            collect_operand_use_v1(checked.right(), promoted, defs, uses);
        }
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => {
            collect_place_use_v1(place, promoted, defs, uses);
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                collect_operand_use_v1(operand, promoted, defs, uses);
            }
        }
        SemanticRvalueKindV1::Load(load) => {
            collect_place_use_v1(load.source(), promoted, defs, uses);
        }
    }
}

fn collect_terminator_uses_v1(
    terminator: &SemanticTerminatorKindV1,
    promoted: &BTreeMap<u32, SemanticPromotedLocalV1>,
    defs: &BTreeSet<u32>,
    uses: &mut BTreeSet<u32>,
) {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            collect_operand_use_v1(discriminant, promoted, defs, uses);
        }
        SemanticTerminatorKindV1::Call(call) => {
            for argument in call.arguments() {
                collect_operand_use_v1(argument, promoted, defs, uses);
            }
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            collect_operand_use_v1(condition, promoted, defs, uses);
            match message {
                SemanticAssertMessageV1::BoundsCheck { length, index }
                | SemanticAssertMessageV1::Overflow {
                    left: length,
                    right: index,
                    ..
                } => {
                    collect_operand_use_v1(length, promoted, defs, uses);
                    collect_operand_use_v1(index, promoted, defs, uses);
                }
                SemanticAssertMessageV1::DivisionByZero(operand)
                | SemanticAssertMessageV1::RemainderByZero(operand) => {
                    collect_operand_use_v1(operand, promoted, defs, uses);
                }
                SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment,
                    found_alignment,
                } => {
                    collect_operand_use_v1(required_alignment, promoted, defs, uses);
                    collect_operand_use_v1(found_alignment, promoted, defs, uses);
                }
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {}
            }
        }
        _ => {}
    }
}

fn collect_operand_use_v1(
    operand: &SemanticOperandV1,
    promoted: &BTreeMap<u32, SemanticPromotedLocalV1>,
    defs: &BTreeSet<u32>,
    uses: &mut BTreeSet<u32>,
) {
    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
        collect_place_use_v1(place, promoted, defs, uses);
    }
}

fn collect_place_use_v1(
    place: &SemanticPlaceV1,
    promoted: &BTreeMap<u32, SemanticPromotedLocalV1>,
    defs: &BTreeSet<u32>,
    uses: &mut BTreeSet<u32>,
) {
    let local = place.local().index();
    if promoted.contains_key(&local) && !defs.contains(&local) {
        uses.insert(local);
    }
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(index) = projection.kind() {
            let index = index.index();
            if promoted.contains_key(&index) && !defs.contains(&index) {
                uses.insert(index);
            }
        }
    }
}

#[derive(Clone, Debug)]
enum SemanticValueBindingV1 {
    Unit,
    Aggregate(Vec<SemanticValueBindingV1>),
    MathContext,
    CollectiveContext,
    MatrixContext,
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
        availability: Option<SemanticOptionAvailabilityV1>,
    },
    OptionIndexWitness {
        present: ValueId,
        id: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        availability: SemanticOptionAvailabilityV1,
    },
    GridLeader {
        availability: SemanticOptionAvailabilityV1,
    },
    ComponentWitness {
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        availability: SemanticOptionAvailabilityV1,
    },
    OptionComponentWitness {
        present: ValueId,
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        availability: SemanticOptionAvailabilityV1,
    },
    OptionGridLeader {
        present: ValueId,
        availability: SemanticOptionAvailabilityV1,
    },
}

impl SemanticValueBindingV1 {
    fn value(&self) -> Result<(ValueId, Type), &'static str> {
        match self {
            Self::Value { id, ty } => Ok((*id, ty.clone())),
            Self::IndexWitness { id, .. } => Ok((*id, Type::INDEX)),
            Self::Unit
            | Self::Aggregate(_)
            | Self::MathContext
            | Self::CollectiveContext
            | Self::MatrixContext
            | Self::OptionPointer { .. }
            | Self::OptionIndexWitness { .. }
            | Self::ComponentWitness { .. }
            | Self::OptionComponentWitness { .. }
            | Self::GridLeader { .. }
            | Self::OptionGridLeader { .. } => {
                Err("aggregate or capability value requires a semantic projection")
            }
        }
    }

    fn values(&self) -> Result<Vec<(ValueId, Type)>, &'static str> {
        let mut values = Vec::new();
        self.append_values(&mut values)?;
        Ok(values)
    }

    fn append_values(&self, values: &mut Vec<(ValueId, Type)>) -> Result<(), &'static str> {
        match self {
            Self::Value { id, ty } => values.push((*id, ty.clone())),
            Self::IndexWitness { id, .. } => values.push((*id, Type::INDEX)),
            Self::Aggregate(fields) => {
                for field in fields {
                    field.append_values(values)?;
                }
            }
            Self::Unit => {}
            Self::MathContext
            | Self::CollectiveContext
            | Self::MatrixContext
            | Self::OptionPointer { .. }
            | Self::OptionIndexWitness { .. }
            | Self::ComponentWitness { .. }
            | Self::OptionComponentWitness { .. }
            | Self::GridLeader { .. }
            | Self::OptionGridLeader { .. } => {
                return Err("capability value has no ordinary SSA representation");
            }
        }
        Ok(())
    }
}

fn require_binding_components(
    block: SemanticBlockIdV1,
    binding: SemanticValueBindingV1,
    expected_type: Type,
    expected_count: usize,
    description: &'static str,
) -> Result<Vec<(ValueId, Type)>, ProductionSemanticKirErrorV1> {
    let values = binding
        .values()
        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
    if values.len() != expected_count || values.iter().any(|(_, actual)| actual != &expected_type) {
        return Err(unsupported(0, Some(block.index()), None, description));
    }
    Ok(values)
}

struct SemanticFunctionLoweringV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    locals: Vec<Option<SemanticValueBindingV1>>,
    option_dominance: SemanticOptionDominanceV1,
    control_flow_ssa: SemanticControlFlowSsaPlanV1,
    block_parameters: BTreeMap<u32, BTreeMap<u32, Vec<ValueDef>>>,
    next_value: u32,
    assert_failure_block: Option<BlockId>,
    discharge_ranked_bounds: bool,
    max_operations: usize,
    emitted_operations: usize,
}

struct SemanticParameterBindingsV1<'a> {
    declarations: &'a [(u32, usize, SemanticTypeIdV1)],
    values: &'a [ValueId],
    types: &'a [Type],
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        parameters: SemanticParameterBindingsV1<'_>,
        assert_failure_block: Option<BlockId>,
        discharge_ranked_bounds: bool,
        max_operations: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut locals = vec![None; function.locals().len()];
        let option_producers = semantic_option_producers_v1(function, callables)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        let option_dominance = SemanticOptionDominanceV1::analyze(function, &option_producers)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        for ((_, local, _), (value, ty)) in parameters
            .declarations
            .iter()
            .zip(parameters.values.iter().zip(parameters.types))
        {
            locals[*local] = Some(SemanticValueBindingV1::Value {
                id: *value,
                ty: ty.clone(),
            });
        }
        let mut next_value = u32::try_from(function.locals().len())
            .map_err(|_| unsupported(0, None, None, "local count does not fit Kernel IR"))?;
        let control_flow_ssa = SemanticControlFlowSsaPlanV1::analyze(types, function)?;
        let mut block_parameters = BTreeMap::new();
        for block in 0..function.blocks().len() as u32 {
            if block == function.entry().index() {
                continue;
            }
            let mut parameters = BTreeMap::new();
            for local in control_flow_ssa.live_in(block) {
                let promoted = control_flow_ssa
                    .promoted
                    .get(local)
                    .expect("live-in local must be promoted");
                let mut components = Vec::with_capacity(promoted.kernel_types.len());
                for ty in promoted.kernel_types.iter().cloned() {
                    components.push(ValueDef::new(ValueId(next_value), ty));
                    next_value = next_value.checked_add(1).ok_or_else(|| {
                        unsupported(0, Some(block), None, "block-parameter identity overflow")
                    })?;
                }
                parameters.insert(*local, components);
            }
            block_parameters.insert(block, parameters);
        }
        Ok(Self {
            types,
            callables,
            function,
            locals,
            option_dominance,
            control_flow_ssa,
            block_parameters,
            next_value,
            assert_failure_block,
            discharge_ranked_bounds,
            max_operations,
            emitted_operations: 0,
        })
    }

    fn begin_block(
        &mut self,
        block: SemanticBlockIdV1,
        target: &mut BasicBlock,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for local in self.control_flow_ssa.promoted.keys() {
            let declaration = self.function.locals().get(*local as usize).ok_or_else(|| {
                unsupported(0, Some(block.index()), None, "promoted local is missing")
            })?;
            if block == self.function.entry()
                && matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
            {
                continue;
            }
            self.locals[*local as usize] = None;
        }
        if block == self.function.entry() {
            return Ok(());
        }
        let parameters = self
            .block_parameters
            .get(&block.index())
            .cloned()
            .ok_or_else(|| {
                unsupported(0, Some(block.index()), None, "block parameters are missing")
            })?;
        for (local, parameters) in parameters {
            let semantic_type = self.control_flow_ssa.promoted[&local].semantic_type;
            self.locals[local as usize] = Some(binding_from_value_defs(
                self.types,
                semantic_type,
                &parameters,
            )?);
            target.parameters.extend(parameters);
        }
        Ok(())
    }

    fn edge_arguments(
        &self,
        block: SemanticBlockIdV1,
        target: SemanticBlockIdV1,
    ) -> Result<Vec<ValueId>, ProductionSemanticKirErrorV1> {
        let mut arguments = Vec::new();
        for local in self.control_flow_ssa.live_in(target.index()) {
            let values = self
                .locals
                .get(*local as usize)
                .and_then(Option::as_ref)
                .ok_or(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: 0,
                    block: block.index(),
                    statement: None,
                    local: *local,
                })?
                .values()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let expected = &self.control_flow_ssa.promoted[local].kernel_types;
            if values.len() != expected.len()
                || values
                    .iter()
                    .zip(expected.iter())
                    .any(|((_, actual), expected)| actual != expected)
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "promoted aggregate changed its SSA component types",
                ));
            }
            arguments.extend(values.into_iter().map(|(value, _)| value));
        }
        Ok(arguments)
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
                let checked_checkpoint = matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::CheckedBinary(_)
                )
                .then_some((
                    operations.len(),
                    self.next_value,
                    self.emitted_operations,
                ));
                let result = self.lower_rvalue(
                    block,
                    statement,
                    assignment.value().result_type(),
                    assignment.value().kind(),
                    operations,
                );
                let result = match result {
                    Ok(value) => self.assign_place(
                        block,
                        statement,
                        assignment.destination(),
                        value,
                        SemanticVolatilityV1::NonVolatile,
                        operations,
                    ),
                    Err(error) => Err(error),
                };
                if result.is_err()
                    && let Some((operation_count, next_value, emitted_operations)) =
                        checked_checkpoint
                {
                    operations.truncate(operation_count);
                    self.next_value = next_value;
                    self.emitted_operations = emitted_operations;
                }
                result
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
                    | SemanticValueBindingV1::OptionComponentWitness { present, .. }
                    | SemanticValueBindingV1::OptionGridLeader { present, .. } => {
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
                    SemanticValueBindingV1::Unit
                    | SemanticValueBindingV1::Aggregate(_)
                    | SemanticValueBindingV1::MathContext
                    | SemanticValueBindingV1::CollectiveContext
                    | SemanticValueBindingV1::MatrixContext
                    | SemanticValueBindingV1::Value { .. }
                    | SemanticValueBindingV1::IndexWitness { .. }
                    | SemanticValueBindingV1::ComponentWitness { .. }
                    | SemanticValueBindingV1::GridLeader { .. } => Err(unsupported(
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
                let semantic_left_type = semantic_operand_type(left);
                let semantic_right_type = semantic_operand_type(right);
                let semantic_operands_match = semantic_left_type == semantic_right_type;
                let (left, left_ty) = self
                    .lower_operand(block, statement, left, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let (right, right_ty) = self
                    .lower_operand(block, statement, right, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let (left, left_ty, right, right_ty) = match (&left_ty, &right_ty) {
                    (Type::Scalar(ScalarType::Index), Type::Scalar(ScalarType::U64))
                        if semantic_operands_match =>
                    {
                        let converted = self.emit(
                            operations,
                            Type::INDEX,
                            OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value: right,
                                to: Type::INDEX,
                            },
                        )?;
                        let (right, right_ty) = converted.value().map_err(|detail| {
                            unsupported(0, Some(block.index()), statement, detail)
                        })?;
                        (left, left_ty, right, right_ty)
                    }
                    (Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::Index))
                        if semantic_operands_match =>
                    {
                        let converted = self.emit(
                            operations,
                            Type::INDEX,
                            OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value: left,
                                to: Type::INDEX,
                            },
                        )?;
                        let (left, left_ty) = converted.value().map_err(|detail| {
                            unsupported(0, Some(block.index()), statement, detail)
                        })?;
                        (left, left_ty, right, right_ty)
                    }
                    _ => (left, left_ty, right, right_ty),
                };
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
            SemanticRvalueKindV1::CheckedBinary(checked) => {
                let semantic_operand_ty = semantic_operand_type(checked.left());
                if semantic_operand_ty != semantic_operand_type(checked.right()) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "semantic checked arithmetic operand types differ",
                    ));
                }
                let operand_type =
                    checked_binary_result_type(self.types, semantic_operand_ty, result_type)
                        .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let left = self.lower_operand(block, statement, checked.left(), operations)?;
                let (left, left_type) = self.normalize_checked_operand(
                    block,
                    statement,
                    left,
                    &operand_type,
                    operations,
                )?;
                let right = self.lower_operand(block, statement, checked.right(), operations)?;
                let (right, right_type) = self.normalize_checked_operand(
                    block,
                    statement,
                    right,
                    &operand_type,
                    operations,
                )?;
                if left_type != right_type {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "lowered checked arithmetic operand types differ",
                    ));
                }
                self.emit_checked_binary(
                    operations,
                    left_type,
                    lower_checked_binary(checked.operation()),
                    left,
                    right,
                )
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
            SemanticRvalueKindV1::Aggregate(aggregate)
                if matches!(
                    self.types[result_type.index() as usize].shape(),
                    SemanticTypeShapeV1::Array { .. }
                        | SemanticTypeShapeV1::Tuple(_)
                        | SemanticTypeShapeV1::Aggregate(_)
                ) =>
            {
                let mut fields = Vec::with_capacity(aggregate.operands().len());
                for operand in aggregate.operands() {
                    fields.push(self.lower_operand(block, statement, operand, operations)?);
                }
                Ok(SemanticValueBindingV1::Aggregate(fields))
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
                let binding = if place.projections().iter().any(|projection| {
                    matches!(projection.kind(), SemanticProjectionKindV1::Index(_))
                }) {
                    self.lower_indexed_place_address(block, statement, place, operations)?
                } else {
                    self.resolve_place(block, statement, place)?
                };
                if !place
                    .projections()
                    .iter()
                    .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
                {
                    return Ok(binding);
                }
                let (pointer, pointer_ty) = binding
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                let Type::Pointer(pointer_type) = pointer_ty else {
                    return Ok(SemanticValueBindingV1::Value {
                        id: pointer,
                        ty: pointer_ty,
                    });
                };
                let mut access =
                    memory_access_for_type(self.types, place.ty(), pointer_type.address_space)?;
                access.volatile = false;
                self.emit(
                    operations,
                    (*pointer_type.pointee).clone(),
                    OperationKind::Load { pointer, access },
                )
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

    fn normalize_checked_operand(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        binding: SemanticValueBindingV1,
        expected_type: &Type,
        operations: &mut Vec<Operation>,
    ) -> Result<(ValueId, Type), ProductionSemanticKirErrorV1> {
        let (value, actual_type) = binding
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
        if &actual_type == expected_type {
            return Ok((value, actual_type));
        }
        if actual_type == Type::INDEX && *expected_type == Type::Scalar(ScalarType::U64) {
            return self
                .emit(
                    operations,
                    expected_type.clone(),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value,
                        to: expected_type.clone(),
                    },
                )?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), statement, detail));
        }
        Err(unsupported(
            0,
            Some(block.index()),
            statement,
            "checked arithmetic operand has no exact plain-integer representation",
        ))
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
                arguments: self.edge_arguments(block, edge.target())?,
            }),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                let (selector, selector_ty) = self
                    .lower_operand(block, None, discriminant, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                if selector_ty == Type::BOOL {
                    let [target] = targets.values() else {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "boolean switch must have one explicit target",
                        ));
                    };
                    if target.value() > 1 {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "boolean switch target is not zero or one",
                        ));
                    }
                    let explicit = target.edge().target();
                    let otherwise = targets.otherwise().target();
                    let (then_target, else_target) = if target.value() == 1 {
                        (explicit, otherwise)
                    } else {
                        (otherwise, explicit)
                    };
                    return Ok(Terminator::ConditionalBranch {
                        condition: selector,
                        then_target: BlockId(then_target.index()),
                        then_arguments: self.edge_arguments(block, then_target)?,
                        else_target: BlockId(else_target.index()),
                        else_arguments: self.edge_arguments(block, else_target)?,
                    });
                }
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
                            arguments: self.edge_arguments(block, target.edge().target())?,
                        })
                    })
                    .collect::<Result<Vec<_>, ProductionSemanticKirErrorV1>>()?;
                Ok(Terminator::Switch {
                    selector,
                    cases,
                    default_target: BlockId(targets.otherwise().target().index()),
                    default_arguments: self.edge_arguments(block, targets.otherwise().target())?,
                })
            }
            SemanticTerminatorKindV1::Call(call) => self.lower_call(block, call, operations),
            SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } => {
                if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "semantic assert has a cleanup unwind edge",
                    ));
                }
                if self.discharge_ranked_bounds
                    && matches!(message, SemanticAssertMessageV1::BoundsCheck { .. })
                {
                    if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "ranked bounds custody cannot discharge a noncanonical bounds assertion",
                        ));
                    }
                    return Ok(Terminator::Branch {
                        target: BlockId(target.target().index()),
                        arguments: self.edge_arguments(block, target.target())?,
                    });
                }
                let failure = self.assert_failure_block.ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "semantic assert has no retained runtime failure block",
                    )
                })?;
                let (condition, condition_ty) = self
                    .lower_operand(block, None, condition, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                if condition_ty != Type::BOOL {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "semantic assert condition is not boolean",
                    ));
                }
                let success = BlockId(target.target().index());
                let success_arguments = self.edge_arguments(block, target.target())?;
                let (then_target, then_arguments, else_target, else_arguments) = if *expected {
                    (success, success_arguments, failure, vec![])
                } else {
                    (failure, vec![], success, success_arguments)
                };
                Ok(Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                })
            }
            SemanticTerminatorKindV1::Abort | SemanticTerminatorKindV1::UnwindTerminate => {
                let failure = self.assert_failure_block.ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "semantic abort has no retained runtime failure block",
                    )
                })?;
                Ok(Terminator::Branch {
                    target: failure,
                    arguments: vec![],
                })
            }
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
            SemanticCompilerIntrinsicOperationV1::MathContextCurrent { .. } => {
                self.require_call_argument_count(block, call, 0)?;
                SemanticValueBindingV1::MathContext
            }
            SemanticCompilerIntrinsicOperationV1::MathF32 { function, .. } => {
                self.require_call_argument_count(block, call, function.arity() + 1)?;
                let context = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                if !matches!(context, SemanticValueBindingV1::MathContext) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "device math operation lacks compiler-issued math authority",
                    ));
                }
                let function = lower_f32_math_function(*function);
                let mut arguments = Vec::with_capacity(function.arity());
                for argument in &call.arguments()[1..] {
                    let (id, ty) = self
                        .lower_operand(block, None, argument, operations)?
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                    if ty != Type::Scalar(ScalarType::F32) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "device math argument is not f32",
                        ));
                    }
                    arguments.push(id);
                }
                self.emit_float_operation(
                    operations,
                    FloatOperation::F32Math {
                        function,
                        implementation: function.required_implementation(),
                        arguments,
                    },
                )?
            }
            SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { .. } => {
                self.require_call_argument_count(block, call, 0)?;
                SemanticValueBindingV1::CollectiveContext
            }
            SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 { width, kind, .. } => {
                self.require_call_argument_count(block, call, 2)?;
                let context = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                if !matches!(context, SemanticValueBindingV1::CollectiveContext) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "subgroup reduction lacks compiler-issued collective authority",
                    ));
                }
                let value = self.lower_operand(block, None, &call.arguments()[1], operations)?;
                self.lower_subgroup_reduce_f32(block, operations, value, *width, *kind)?
            }
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { .. } => {
                self.require_call_argument_count(block, call, 0)?;
                SemanticValueBindingV1::MatrixContext
            }
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixFragmentFromBits {
                fragment, ..
            } => {
                self.require_call_argument_count(block, call, 1)?;
                let bits = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let bits = require_binding_components(
                    block,
                    bits,
                    Type::Scalar(ScalarType::U16),
                    4,
                    "BF16 matrix fragment bits",
                )?;
                let mut values = Vec::with_capacity(4);
                for (value, _) in bits {
                    let (id, ty) = self
                        .emit(
                            operations,
                            Type::Scalar(ScalarType::Bf16),
                            OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value,
                                to: Type::Scalar(ScalarType::Bf16),
                            },
                        )?
                        .value()
                        .expect("emitted BF16 bitcast");
                    values.push(ValueDef::new(id, ty));
                }
                binding_from_matrix_value_defs(self.types, *fragment, &values)?
            }
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorFromValues {
                fragment,
                ..
            } => {
                self.require_call_argument_count(block, call, 1)?;
                let values = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let values = require_binding_components(
                    block,
                    values,
                    Type::Scalar(ScalarType::F32),
                    4,
                    "FP32 matrix accumulator values",
                )?
                .into_iter()
                .map(|(id, ty)| ValueDef::new(id, ty))
                .collect::<Vec<_>>();
                binding_from_value_defs(self.types, *fragment, &values)?
            }
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                values, ..
            } => {
                self.require_call_argument_count(block, call, 1)?;
                let fragment = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let fragment = require_binding_components(
                    block,
                    fragment,
                    Type::Scalar(ScalarType::F32),
                    4,
                    "FP32 matrix accumulator fragment",
                )?
                .into_iter()
                .map(|(id, ty)| ValueDef::new(id, ty))
                .collect::<Vec<_>>();
                binding_from_value_defs(self.types, *values, &fragment)?
            }
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                accumulator_fragment,
                ..
            } => {
                self.require_call_argument_count(block, call, 4)?;
                let context = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                if !matches!(context, SemanticValueBindingV1::MatrixContext) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "matrix operation lacks compiler-issued context authority",
                    ));
                }
                let lhs = require_binding_components(
                    block,
                    self.lower_operand(block, None, &call.arguments()[1], operations)?,
                    Type::Scalar(ScalarType::Bf16),
                    4,
                    "matrix lhs fragment",
                )?;
                let rhs = require_binding_components(
                    block,
                    self.lower_operand(block, None, &call.arguments()[2], operations)?,
                    Type::Scalar(ScalarType::Bf16),
                    4,
                    "matrix rhs fragment",
                )?;
                let accumulator = require_binding_components(
                    block,
                    self.lower_operand(block, None, &call.arguments()[3], operations)?,
                    Type::Scalar(ScalarType::F32),
                    4,
                    "matrix accumulator fragment",
                )?;
                let lhs = lhs
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("four checked lhs components");
                let rhs = rhs
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("four checked rhs components");
                let accumulator = accumulator
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("four checked accumulator components");
                let results = self.emit_results(
                    operations,
                    vec![Type::Scalar(ScalarType::F32); 4],
                    OperationKind::Matrix(MatrixOperation::multiply_accumulate(
                        lhs,
                        rhs,
                        accumulator,
                    )),
                )?;
                binding_from_value_defs(self.types, *accumulator_fragment, &results)?
            }
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
                    availability: None,
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
                    availability,
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
                    availability,
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
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                input_space,
                output_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                ..
            } => self.lower_checked_tiled_2d(
                block,
                call,
                operations,
                *input_space,
                *output_space,
                *lanes_per_tile,
                *tile_rows,
                *tile_columns,
                *elements_per_lane,
            )?,
            SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { index_space, .. } => {
                self.require_call_argument_count(block, call, 1)?;
                let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
                let SemanticValueBindingV1::IndexWitness {
                    id,
                    index_space: actual,
                    disjoint: true,
                    ..
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
            SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { .. } => {
                self.require_call_argument_count(block, call, 1)?;
                let (slice, slice_ty) = self
                    .lower_operand(block, None, &call.arguments()[0], operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                if !matches!(slice_ty, Type::Slice(_)) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "DisjointSlice::len receiver is not a lowered slice",
                    ));
                }
                let (id, _) = self
                    .emit(
                        operations,
                        Type::INDEX,
                        OperationKind::SliceLength { slice },
                    )?
                    .value()
                    .expect("emitted slice length");
                SemanticValueBindingV1::Value {
                    id,
                    ty: Type::INDEX,
                }
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
                let availability = self
                    .option_dominance
                    .availability(destination.place().local())
                    .ok_or_else(|| {
                        unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "grid-leader Option lacks an authenticated Some edge",
                        )
                    })?;
                SemanticValueBindingV1::OptionGridLeader {
                    present,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. } => {
                self.require_call_argument_count(block, call, 3)?;
                let leader = self.lower_operand(block, None, &call.arguments()[1], operations)?;
                if !matches!(leader, SemanticValueBindingV1::GridLeader { .. }) {
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
                let SemanticValueBindingV1::ComponentWitness {
                    raw,
                    index_space: actual,
                    ..
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
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                index_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                ..
            } => {
                self.require_call_argument_count(block, call, 6)?;
                let witness = self.lower_operand(block, None, &call.arguments()[1], operations)?;
                let SemanticValueBindingV1::ComponentWitness {
                    raw,
                    index_space: actual,
                    ..
                } = witness
                else {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "get_tiled_2d_mut lacks tiled ownership authority",
                    ));
                };
                let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: *lanes_per_tile,
                    tile_rows: *tile_rows,
                    tile_columns: *tile_columns,
                    elements_per_lane: *elements_per_lane,
                };
                if actual != expected || *index_space != expected {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "get_tiled_2d_mut mapping identity changed",
                    ));
                }
                let mut indices = Vec::with_capacity(4);
                for argument in &call.arguments()[2..6] {
                    let value = self.lower_operand(block, None, argument, operations)?;
                    let value = self.coerce_index(block, operations, value)?;
                    indices.push(
                        value
                            .value()
                            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
                            .0,
                    );
                }
                let [component, rows, columns, row_stride] = indices
                    .try_into()
                    .expect("four checked tiled-2d index operands");
                let (index, present) = self.lower_tiled_2d_component_index(
                    block,
                    operations,
                    raw,
                    component,
                    rows,
                    columns,
                    row_stride,
                    *lanes_per_tile,
                    *tile_rows,
                    *tile_columns,
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
            SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier => {
                self.require_call_argument_count(block, call, 0)?;
                self.push_operation(operations, || {
                    Operation::new(
                        Vec::new(),
                        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                            memory_scope: SynchronizationScope::Workgroup,
                            semantics: BarrierSemantics::new(
                                MemoryOrdering::AcquireRelease,
                                [AddressSpace::Workgroup],
                            ),
                            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
                        }),
                    )
                })?;
                SemanticValueBindingV1::Unit
            }
            SemanticCompilerIntrinsicOperationV1::WaveBarrier
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
            arguments: self.edge_arguments(block, destination.edge().target())?,
        })
    }

    fn lower_subgroup_reduce_f32(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        value: SemanticValueBindingV1,
        width: u32,
        kind: SemanticSubgroupReductionKindV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if width == 0 || !width.is_power_of_two() || width > 64 {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "subgroup reduction width must be a power of two in 1..=64",
            ));
        }
        let (mut reduced, ty) = value
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
        if ty != Type::Scalar(ScalarType::F32) {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "subgroup reduction input is not f32",
            ));
        }
        let (lane, _) = self
            .emit(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
            )?
            .value()
            .expect("emitted lane id");
        let width_value = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(width)),
        )?;
        let local_lane = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Remainder,
                lhs: lane,
                rhs: width_value,
            },
        )?;
        let subgroup = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs: lane,
                rhs: width_value,
            },
        )?;
        let subgroup_base = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: subgroup,
                rhs: width_value,
            },
        )?;

        let mut offset = width / 2;
        while offset != 0 {
            let offset_value = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(offset)),
            )?;
            let source_local = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Binary {
                    op: BinaryOp::BitXor,
                    lhs: local_lane,
                    rhs: offset_value,
                },
            )?;
            let source_lane = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: subgroup_base,
                    rhs: source_local,
                },
            )?;
            let bits = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: reduced,
                    to: Type::Scalar(ScalarType::U32),
                },
            )?;
            let peer_bits = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::ShuffleIndex {
                        value: bits,
                        source_lane,
                        tile_width: width,
                    },
                    WaveWidth::Wave64,
                )),
            )?;
            let peer = self.emit_id(
                operations,
                Type::Scalar(ScalarType::F32),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: peer_bits,
                    to: Type::Scalar(ScalarType::F32),
                },
            )?;
            reduced = match kind {
                SemanticSubgroupReductionKindV1::Sum => self.emit_id(
                    operations,
                    Type::Scalar(ScalarType::F32),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: reduced,
                        rhs: peer,
                    },
                )?,
                SemanticSubgroupReductionKindV1::Maximum => {
                    let take_peer = self.emit_id(
                        operations,
                        Type::BOOL,
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            lhs: reduced,
                            rhs: peer,
                        },
                    )?;
                    self.emit_id(
                        operations,
                        Type::Scalar(ScalarType::F32),
                        OperationKind::Select {
                            condition: take_peer,
                            true_value: peer,
                            false_value: reduced,
                        },
                    )?
                }
            };
            offset /= 2;
        }
        Ok(SemanticValueBindingV1::Value {
            id: reduced,
            ty: Type::Scalar(ScalarType::F32),
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
            ..
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
            | SemanticDisjointIndexSpaceV1::Tiled2dIndex1d { .. }
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
            availability: self
                .option_dominance
                .availability(
                    call.destination()
                        .expect("checked destination")
                        .place()
                        .local(),
                )
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "checked-shift Option lacks an authenticated Some edge",
                    )
                })?,
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
            availability: None,
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
        Ok(SemanticValueBindingV1::OptionComponentWitness {
            present,
            raw,
            index_space: expected,
            availability: self
                .option_dominance
                .availability(
                    call.destination()
                        .expect("checked destination")
                        .place()
                        .local(),
                )
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "checked-block Option lacks an authenticated Some edge",
                    )
                })?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_checked_tiled_2d(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        input_space: SemanticDisjointIndexSpaceV1,
        output_space: SemanticDisjointIndexSpaceV1,
        lanes_per_tile: u64,
        tile_rows: u64,
        tile_columns: u64,
        elements_per_lane: u64,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 1)?;
        let input = self.lower_operand(block, None, &call.arguments()[0], operations)?;
        let SemanticValueBindingV1::IndexWitness {
            id: raw,
            index_space: actual,
            disjoint: false,
            availability: None,
        } = input
        else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_tiled_2d receiver is not thread-index authority",
            ));
        };
        let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        };
        if actual != input_space
            || input_space != SemanticDisjointIndexSpaceV1::Index1d
            || output_space != expected
            || !tiled_2d_geometry_valid(lanes_per_tile, tile_rows, tile_columns, elements_per_lane)
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "checked_tiled_2d mapping identity is malformed",
            ));
        }
        let (present, _) = self
            .emit(
                operations,
                Type::BOOL,
                OperationKind::Constant(Constant::Bool(true)),
            )?
            .value()
            .expect("emitted tiled-2d witness predicate");
        Ok(SemanticValueBindingV1::OptionComponentWitness {
            present,
            raw,
            index_space: expected,
            availability: self
                .option_dominance
                .availability(
                    call.destination()
                        .expect("checked destination")
                        .place()
                        .local(),
                )
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "checked-tiled-2d Option lacks an authenticated Some edge",
                    )
                })?,
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

    #[allow(clippy::too_many_arguments)]
    fn lower_tiled_2d_component_index(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        raw: ValueId,
        component: ValueId,
        rows: ValueId,
        columns: ValueId,
        row_stride: ValueId,
        lanes_per_tile: u64,
        tile_rows: u64,
        tile_columns: u64,
        elements_per_lane: u64,
    ) -> Result<(ValueId, ValueId), ProductionSemanticKirErrorV1> {
        if !tiled_2d_geometry_valid(lanes_per_tile, tile_rows, tile_columns, elements_per_lane) {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "tiled-2d geometry is malformed",
            ));
        }
        let zero = self.emit_index_constant(operations, 0)?;
        let one = self.emit_index_constant(operations, 1)?;
        let maximum = self.emit_index_constant(operations, u64::MAX)?;
        let lanes = self.emit_index_constant(operations, lanes_per_tile)?;
        let tile_rows_value = self.emit_index_constant(operations, tile_rows)?;
        let tile_columns_value = self.emit_index_constant(operations, tile_columns)?;
        let elements = self.emit_index_constant(operations, elements_per_lane)?;
        let column_padding = self.emit_index_constant(operations, tile_columns - 1)?;
        let maximum_columns =
            self.emit_index_constant(operations, u64::MAX - (tile_columns - 1))?;
        let columns_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            columns,
            maximum_columns,
        )?;
        let adjusted_columns =
            self.emit_index_binary(operations, BinaryOp::Add, columns, column_padding)?;
        let tiles_per_row = self.emit_index_binary(
            operations,
            BinaryOp::Divide,
            adjusted_columns,
            tile_columns_value,
        )?;
        let tiles_nonzero =
            self.emit_compare(operations, ComparePredicate::LessThan, zero, tiles_per_row)?;
        let safe_tiles_per_row =
            self.emit_select_index(operations, tiles_nonzero, tiles_per_row, one)?;
        let tile = self.emit_index_binary(operations, BinaryOp::Divide, raw, lanes)?;
        let lane = self.emit_index_binary(operations, BinaryOp::Remainder, raw, lanes)?;
        let tile_row =
            self.emit_index_binary(operations, BinaryOp::Divide, tile, safe_tiles_per_row)?;
        let tile_column =
            self.emit_index_binary(operations, BinaryOp::Remainder, tile, safe_tiles_per_row)?;
        let lane_row =
            self.emit_index_binary(operations, BinaryOp::Divide, lane, tile_columns_value)?;
        let local_row_base =
            self.emit_index_binary(operations, BinaryOp::Multiply, lane_row, elements)?;
        let local_row =
            self.emit_index_binary(operations, BinaryOp::Add, local_row_base, component)?;
        let local_row_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            local_row_base,
            local_row,
        )?;
        let local_column =
            self.emit_index_binary(operations, BinaryOp::Remainder, lane, tile_columns_value)?;

        let maximum_tile_row =
            self.emit_index_binary(operations, BinaryOp::Divide, maximum, tile_rows_value)?;
        let tile_row_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            tile_row,
            maximum_tile_row,
        )?;
        let row_base =
            self.emit_index_binary(operations, BinaryOp::Multiply, tile_row, tile_rows_value)?;
        let row = self.emit_index_binary(operations, BinaryOp::Add, row_base, local_row)?;
        let row_add_safe =
            self.emit_compare(operations, ComparePredicate::LessThanOrEqual, row_base, row)?;

        let maximum_tile_column =
            self.emit_index_binary(operations, BinaryOp::Divide, maximum, tile_columns_value)?;
        let tile_column_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            tile_column,
            maximum_tile_column,
        )?;
        let column_base = self.emit_index_binary(
            operations,
            BinaryOp::Multiply,
            tile_column,
            tile_columns_value,
        )?;
        let column =
            self.emit_index_binary(operations, BinaryOp::Add, column_base, local_column)?;
        let column_add_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            column_base,
            column,
        )?;

        let stride_nonzero =
            self.emit_compare(operations, ComparePredicate::LessThan, zero, row_stride)?;
        let safe_stride = self.emit_select_index(operations, stride_nonzero, row_stride, one)?;
        let maximum_row =
            self.emit_index_binary(operations, BinaryOp::Divide, maximum, safe_stride)?;
        let row_multiply_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            row,
            maximum_row,
        )?;
        let row_offset = self.emit_index_binary(operations, BinaryOp::Multiply, row, row_stride)?;
        let index = self.emit_index_binary(operations, BinaryOp::Add, row_offset, column)?;
        let index_add_safe = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            row_offset,
            index,
        )?;
        let component_valid =
            self.emit_compare(operations, ComparePredicate::LessThan, component, elements)?;
        let stride_valid = self.emit_compare(
            operations,
            ComparePredicate::LessThanOrEqual,
            columns,
            row_stride,
        )?;
        let row_valid = self.emit_compare(operations, ComparePredicate::LessThan, row, rows)?;
        let column_valid =
            self.emit_compare(operations, ComparePredicate::LessThan, column, columns)?;
        let predicates = [
            columns_safe,
            tiles_nonzero,
            local_row_safe,
            tile_row_safe,
            row_add_safe,
            tile_column_safe,
            column_add_safe,
            row_multiply_safe,
            index_add_safe,
            component_valid,
            stride_valid,
            row_valid,
            column_valid,
        ];
        let mut present = predicates[0];
        for predicate in &predicates[1..] {
            present = self.emit_bool_and(operations, present, *predicate)?;
        }
        Ok((index, present))
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
        self.push_operation(operations, || {
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                },
            )
        })?;
        Ok(())
    }

    fn lower_indexed_place_address(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let local = self.require_local(block, statement, place.local().index())?;
        let mut binding = self.locals[local].clone().ok_or(
            ProductionSemanticKirErrorV1::MissingLocalDefinition {
                function: 0,
                block: block.index(),
                statement,
                local: place.local().index(),
            },
        )?;
        for projection in place.projections() {
            match projection.kind() {
                SemanticProjectionKindV1::Dereference
                | SemanticProjectionKindV1::Field(0)
                | SemanticProjectionKindV1::Downcast(_)
                | SemanticProjectionKindV1::OpaqueCast
                | SemanticProjectionKindV1::Subtype => {}
                SemanticProjectionKindV1::Index(index_local) => {
                    let (slice, slice_ty) = binding
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                    let Type::Slice(slice_type) = slice_ty else {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            statement,
                            "indexed semantic place is not a lowered slice",
                        ));
                    };
                    let index_binding = self
                        .locals
                        .get(index_local.index() as usize)
                        .and_then(Option::as_ref)
                        .ok_or(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                            function: 0,
                            block: block.index(),
                            statement,
                            local: index_local.index(),
                        })?;
                    let (index, index_ty) = index_binding
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), statement, detail))?;
                    let index = if index_ty == Type::INDEX {
                        index
                    } else if index_ty == Type::Scalar(ScalarType::U64) {
                        self.emit(
                            operations,
                            Type::INDEX,
                            OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value: index,
                                to: Type::INDEX,
                            },
                        )?
                        .value()
                        .expect("emitted index cast")
                        .0
                    } else {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            statement,
                            "slice index has no exact Kernel IR index representation",
                        ));
                    };
                    let pointer_ty = Type::pointer(
                        (*slice_type.element).clone(),
                        slice_type.address_space,
                        slice_type.access,
                    );
                    let base = self
                        .emit(
                            operations,
                            pointer_ty.clone(),
                            OperationKind::SliceData { slice },
                        )?
                        .value()
                        .expect("emitted slice data")
                        .0;
                    binding = self.emit(
                        operations,
                        pointer_ty,
                        OperationKind::GetElementPointer {
                            base,
                            offset: index,
                        },
                    )?;
                }
                SemanticProjectionKindV1::ConstantIndex { .. }
                | SemanticProjectionKindV1::Subslice { .. }
                | SemanticProjectionKindV1::Field(_) => {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "indexed semantic place has an unsupported projection",
                    ));
                }
            }
        }
        Ok(binding)
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
                (SemanticValueBindingV1::Unit, _) => {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "unit capability result cannot be projected",
                    ));
                }
                (SemanticValueBindingV1::MatrixContext, SemanticProjectionKindV1::Dereference)
                | (
                    SemanticValueBindingV1::MatrixContext,
                    SemanticProjectionKindV1::OpaqueCast | SemanticProjectionKindV1::Subtype,
                ) => SemanticValueBindingV1::MatrixContext,
                (
                    SemanticValueBindingV1::MathContext,
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => SemanticValueBindingV1::MathContext,
                (
                    SemanticValueBindingV1::CollectiveContext,
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => SemanticValueBindingV1::CollectiveContext,
                (
                    SemanticValueBindingV1::Aggregate(fields),
                    SemanticProjectionKindV1::Field(field),
                ) => fields.get(field as usize).cloned().ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        statement,
                        "aggregate field projection is out of range",
                    )
                })?,
                (
                    SemanticValueBindingV1::Aggregate(fields),
                    SemanticProjectionKindV1::ConstantIndex {
                        offset, from_end, ..
                    },
                ) => {
                    let offset = usize::try_from(offset).map_err(|_| {
                        unsupported(
                            0,
                            Some(block.index()),
                            statement,
                            "aggregate constant index does not fit this host",
                        )
                    })?;
                    let index = if from_end {
                        fields.len().checked_sub(offset)
                    } else {
                        Some(offset)
                    };
                    index
                        .and_then(|index| fields.get(index))
                        .cloned()
                        .ok_or_else(|| {
                            unsupported(
                                0,
                                Some(block.index()),
                                statement,
                                "aggregate constant index is out of range",
                            )
                        })?
                }
                (
                    binding @ SemanticValueBindingV1::Aggregate(_),
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
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
                        id,
                        index_space,
                        availability,
                        ..
                    },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::IndexWitness {
                    id,
                    availability: Some(availability),
                    index_space,
                    disjoint: true,
                },
                (
                    SemanticValueBindingV1::OptionComponentWitness {
                        raw,
                        index_space,
                        availability,
                        ..
                    },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::ComponentWitness {
                    raw,
                    index_space,
                    availability,
                },
                (
                    SemanticValueBindingV1::OptionGridLeader { availability, .. },
                    SemanticProjectionKindV1::Field(_),
                ) => SemanticValueBindingV1::GridLeader { availability },
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
                    binding @ SemanticValueBindingV1::OptionComponentWitness { .. },
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
                    binding @ SemanticValueBindingV1::GridLeader { .. },
                    SemanticProjectionKindV1::Dereference
                    | SemanticProjectionKindV1::Field(0)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                ) => binding,
                (
                    binding @ SemanticValueBindingV1::ComponentWitness { .. },
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
        let availability = match &binding {
            SemanticValueBindingV1::IndexWitness {
                availability: Some(availability),
                ..
            } => Some(*availability),
            SemanticValueBindingV1::GridLeader { availability } => Some(*availability),
            SemanticValueBindingV1::ComponentWitness { availability, .. } => Some(*availability),
            SemanticValueBindingV1::Unit
            | SemanticValueBindingV1::Aggregate(_)
            | SemanticValueBindingV1::MathContext
            | SemanticValueBindingV1::CollectiveContext
            | SemanticValueBindingV1::MatrixContext
            | SemanticValueBindingV1::Value { .. }
            | SemanticValueBindingV1::OptionPointer { .. }
            | SemanticValueBindingV1::IndexWitness {
                availability: None, ..
            }
            | SemanticValueBindingV1::OptionIndexWitness { .. }
            | SemanticValueBindingV1::OptionComponentWitness { .. }
            | SemanticValueBindingV1::OptionGridLeader { .. } => None,
        };
        if availability
            .is_some_and(|availability| !self.option_dominance.allows(availability, block))
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                statement,
                "capability payload is used outside its authenticated Some edge",
            ));
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
        self.push_operation(operations, || {
            Operation::effect_free(ValueDef::new(id, ty.clone()), kind)
        })?;
        Ok(SemanticValueBindingV1::Value { id, ty })
    }

    fn emit_checked_binary(
        &mut self,
        operations: &mut Vec<Operation>,
        ty: Type,
        operator: CheckedBinaryOperator,
        lhs: ValueId,
        rhs: ValueId,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let value = ValueId(self.next_value);
        let overflow = ValueId(
            self.next_value
                .checked_add(1)
                .ok_or_else(|| unsupported(0, None, None, "Kernel IR SSA identity overflow"))?,
        );
        let next_value = self
            .next_value
            .checked_add(2)
            .ok_or_else(|| unsupported(0, None, None, "Kernel IR SSA identity overflow"))?;
        self.push_operation(operations, || {
            Operation::checked_binary(
                ValueDef::new(value, ty.clone()),
                ValueDef::new(overflow, Type::BOOL),
                operator,
                lhs,
                rhs,
            )
        })?;
        self.next_value = next_value;
        Ok(SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::Value { id: value, ty },
            SemanticValueBindingV1::Value {
                id: overflow,
                ty: Type::BOOL,
            },
        ]))
    }

    fn emit_float_operation(
        &mut self,
        operations: &mut Vec<Operation>,
        operation: FloatOperation,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let id = ValueId(self.next_value);
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| unsupported(0, None, None, "Kernel IR SSA identity overflow"))?;
        let ty = operation.result_type();
        self.push_operation(operations, || operation.operation(id))?;
        Ok(SemanticValueBindingV1::Value { id, ty })
    }

    fn reserve_operation(
        &mut self,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let block_actual =
            operations
                .len()
                .checked_add(1)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::Operations,
                    actual: usize::MAX,
                    limit: MAX_BLOCK_OPERATIONS_V1,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            block_actual,
            MAX_BLOCK_OPERATIONS_V1,
        )?;
        let total_actual = self.emitted_operations.checked_add(1).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::Operations,
                actual: usize::MAX,
                limit: self.max_operations,
            },
        )?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            total_actual,
            self.max_operations,
        )?;
        operations
            .try_reserve(1)
            .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::Operations,
            })?;
        self.emitted_operations = total_actual;
        Ok(())
    }

    fn push_operation(
        &mut self,
        operations: &mut Vec<Operation>,
        build: impl FnOnce() -> Operation,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.reserve_operation(operations)?;
        operations.push(build());
        Ok(())
    }

    fn emit_id(
        &mut self,
        operations: &mut Vec<Operation>,
        ty: Type,
        kind: OperationKind,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit(operations, ty, kind)?
            .value()
            .map(|(id, _)| id)
            .map_err(|detail| unsupported(0, None, None, detail))
    }

    fn emit_index_constant(
        &mut self,
        operations: &mut Vec<Operation>,
        value: u64,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(value)),
        )
    }

    fn emit_index_binary(
        &mut self,
        operations: &mut Vec<Operation>,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::Binary { op, lhs, rhs },
        )
    }

    fn emit_compare(
        &mut self,
        operations: &mut Vec<Operation>,
        predicate: ComparePredicate,
        lhs: ValueId,
        rhs: ValueId,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::BOOL,
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            },
        )
    }

    fn emit_select_index(
        &mut self,
        operations: &mut Vec<Operation>,
        condition: ValueId,
        true_value: ValueId,
        false_value: ValueId,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            },
        )
    }

    fn emit_bool_and(
        &mut self,
        operations: &mut Vec<Operation>,
        lhs: ValueId,
        rhs: ValueId,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            },
        )
    }

    fn emit_results(
        &mut self,
        operations: &mut Vec<Operation>,
        types: Vec<Type>,
        kind: OperationKind,
    ) -> Result<Vec<ValueDef>, ProductionSemanticKirErrorV1> {
        self.reserve_operation(operations)?;
        let mut results = Vec::with_capacity(types.len());
        for ty in types {
            let id = ValueId(self.next_value);
            self.next_value = self
                .next_value
                .checked_add(1)
                .ok_or_else(|| unsupported(0, None, None, "Kernel IR SSA identity overflow"))?;
            results.push(ValueDef::new(id, ty));
        }
        operations.push(Operation::new(results.clone(), kind));
        Ok(results)
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
        SemanticRvalueKindV1::CheckedBinary(_) => {
            "internally supported semantic checked arithmetic reached unsupported diagnostics"
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

const MAX_SSA_VALUE_COMPONENTS_V1: usize = 256;

fn lower_ssa_value_types(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    fn append(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        output: &mut Vec<Type>,
        structural_nodes: &mut usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        *structural_nodes = structural_nodes.checked_add(1).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "aggregate SSA value exceeds the structural limit",
            )
        })?;
        if *structural_nodes > MAX_SSA_VALUE_COMPONENTS_V1
            || output.len() > MAX_SSA_VALUE_COMPONENTS_V1
        {
            return Err(unsupported(
                0,
                None,
                None,
                "aggregate SSA value exceeds the structural or component limit",
            ));
        }
        let shape = types
            .get(ty.index() as usize)
            .ok_or_else(|| unsupported(0, None, None, "aggregate SSA type is missing"))?
            .shape();
        match shape {
            SemanticTypeShapeV1::Unit => Ok(()),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                output.push(lower_scalar_type(types, ty)?);
                Ok(())
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let length = usize::try_from(*length).map_err(|_| {
                    unsupported(0, None, None, "aggregate SSA array length is too large")
                })?;
                if length > MAX_SSA_VALUE_COMPONENTS_V1 {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "aggregate SSA array length is too large",
                    ));
                }
                for _ in 0..length {
                    append(types, *element, output, structural_nodes)?;
                }
                Ok(())
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                for field in fields.fields() {
                    append(types, *field, output, structural_nodes)?;
                }
                Ok(())
            }
            _ => Err(unsupported(
                0,
                None,
                None,
                "type has no bounded aggregate SSA representation",
            )),
        }
    }

    let mut output = Vec::new();
    let mut structural_nodes = 0;
    append(types, ty, &mut output, &mut structural_nodes)?;
    if output.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate SSA value exceeds the component limit",
        ));
    }
    Ok(output)
}

fn binding_from_value_defs(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    values: &[ValueDef],
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    binding_from_value_defs_with_validation(types, ty, values, true)
}

fn binding_from_matrix_value_defs(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    values: &[ValueDef],
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    binding_from_value_defs_with_validation(types, ty, values, false)
}

fn binding_from_value_defs_with_validation(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    values: &[ValueDef],
    validate_scalar_types: bool,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    fn build(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        values: &[ValueDef],
        cursor: &mut usize,
        structural_nodes: &mut usize,
        validate_scalar_types: bool,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        *structural_nodes = structural_nodes.checked_add(1).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "aggregate SSA binding exceeds the structural limit",
            )
        })?;
        if *structural_nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "aggregate SSA binding exceeds the structural limit",
            ));
        }
        let shape = types
            .get(ty.index() as usize)
            .ok_or_else(|| unsupported(0, None, None, "aggregate SSA type is missing"))?
            .shape();
        match shape {
            SemanticTypeShapeV1::Unit => Ok(SemanticValueBindingV1::Unit),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                let value = values.get(*cursor).ok_or_else(|| {
                    unsupported(0, None, None, "aggregate SSA value is truncated")
                })?;
                let expected = lower_scalar_type(types, ty)?;
                if validate_scalar_types && value.ty != expected {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "aggregate SSA component type changed",
                    ));
                }
                *cursor += 1;
                Ok(SemanticValueBindingV1::Value {
                    id: value.id,
                    ty: value.ty.clone(),
                })
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let length = usize::try_from(*length).map_err(|_| {
                    unsupported(0, None, None, "aggregate SSA array length is too large")
                })?;
                if length > MAX_SSA_VALUE_COMPONENTS_V1 {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "aggregate SSA array length is too large",
                    ));
                }
                let mut fields = Vec::with_capacity(length);
                for _ in 0..length {
                    fields.push(build(
                        types,
                        *element,
                        values,
                        cursor,
                        structural_nodes,
                        validate_scalar_types,
                    )?);
                }
                Ok(SemanticValueBindingV1::Aggregate(fields))
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let mut bindings = Vec::with_capacity(fields.fields().len());
                for field in fields.fields() {
                    bindings.push(build(
                        types,
                        *field,
                        values,
                        cursor,
                        structural_nodes,
                        validate_scalar_types,
                    )?);
                }
                Ok(SemanticValueBindingV1::Aggregate(bindings))
            }
            _ => Err(unsupported(
                0,
                None,
                None,
                "type has no bounded aggregate SSA representation",
            )),
        }
    }

    let mut cursor = 0;
    let mut structural_nodes = 0;
    let binding = build(
        types,
        ty,
        values,
        &mut cursor,
        &mut structural_nodes,
        validate_scalar_types,
    )?;
    if cursor != values.len() {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate SSA value has trailing components",
        ));
    }
    Ok(binding)
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

const fn semantic_operand_type(operand: &SemanticOperandV1) -> SemanticTypeIdV1 {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place.ty(),
        SemanticOperandV1::Constant(constant) => constant.ty(),
    }
}

fn checked_binary_result_type(
    types: &[SemanticTypeDeclV1],
    operand_type: SemanticTypeIdV1,
    result_type: SemanticTypeIdV1,
) -> Result<Type, &'static str> {
    let operand_shape = types
        .get(operand_type.index() as usize)
        .ok_or("semantic checked arithmetic operand type is missing")?
        .shape();
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) = operand_shape
    else {
        return Err("semantic checked arithmetic operand is not a plain integer");
    };
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err("semantic checked arithmetic integer width is unsupported");
    }
    let result_shape = types
        .get(result_type.index() as usize)
        .ok_or("semantic checked arithmetic result type is missing")?
        .shape();
    let SemanticTypeShapeV1::Tuple(fields) = result_shape else {
        return Err("semantic checked arithmetic result is not a tuple");
    };
    let [value_type, overflow_type] = fields.fields() else {
        return Err("semantic checked arithmetic result is not a two-field tuple");
    };
    if *value_type != operand_type {
        return Err("semantic checked arithmetic value result type differs from its operands");
    }
    if !matches!(
        types
            .get(overflow_type.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
    ) {
        return Err("semantic checked arithmetic overflow result is not bool");
    }
    lower_scalar_kind(SemanticScalarTypeV1::Integer {
        signed: *signed,
        bits: *bits,
    })
    .map_err(|_| "semantic checked arithmetic integer width is unsupported")
}

const fn lower_checked_binary(operation: SemanticCheckedBinaryOpV1) -> CheckedBinaryOperator {
    match operation {
        SemanticCheckedBinaryOpV1::Add => CheckedBinaryOperator::Add,
        SemanticCheckedBinaryOpV1::Subtract => CheckedBinaryOperator::Subtract,
        SemanticCheckedBinaryOpV1::Multiply => CheckedBinaryOperator::Multiply,
    }
}

const fn lower_f32_math_function(function: SemanticF32MathFunctionV1) -> F32MathFunction {
    match function {
        SemanticF32MathFunctionV1::Sqrt => F32MathFunction::Sqrt,
        SemanticF32MathFunctionV1::FusedMultiplyAdd => F32MathFunction::FusedMultiplyAdd,
        SemanticF32MathFunctionV1::Floor => F32MathFunction::Floor,
        SemanticF32MathFunctionV1::Ceil => F32MathFunction::Ceil,
        SemanticF32MathFunctionV1::Truncate => F32MathFunction::Truncate,
        SemanticF32MathFunctionV1::RoundTiesEven => F32MathFunction::RoundTiesEven,
        SemanticF32MathFunctionV1::Sin => F32MathFunction::Sin,
        SemanticF32MathFunctionV1::Cos => F32MathFunction::Cos,
        SemanticF32MathFunctionV1::Exp => F32MathFunction::Exp,
        SemanticF32MathFunctionV1::Exp2 => F32MathFunction::Exp2,
        SemanticF32MathFunctionV1::Ln => F32MathFunction::Ln,
        SemanticF32MathFunctionV1::Log2 => F32MathFunction::Log2,
        SemanticF32MathFunctionV1::Log10 => F32MathFunction::Log10,
    }
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
                }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
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

fn tiled_2d_geometry_valid(
    lanes_per_tile: u64,
    tile_rows: u64,
    tile_columns: u64,
    elements_per_lane: u64,
) -> bool {
    lanes_per_tile != 0
        && tile_rows != 0
        && tile_columns != 0
        && elements_per_lane != 0
        && lanes_per_tile.is_multiple_of(tile_columns)
        && lanes_per_tile.checked_mul(elements_per_lane) == tile_rows.checked_mul(tile_columns)
        && (lanes_per_tile / tile_columns).checked_mul(elements_per_lane) == Some(tile_rows)
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

#[cfg(test)]
mod resource_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBackendReprV1, SemanticFieldsShapeV1, SemanticLayoutIdentityV1,
        SemanticRustcVariantsV1, SemanticTypeIdentityV1, SemanticTypeLayoutDetailsV1,
        SemanticTypeLayoutV1,
    };

    fn unit_type() -> SemanticTypeDeclV1 {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        )
    }

    struct OperationSpanFixture {
        expected: [ExpectedSemanticKirBlockCoverageV1; 1],
        target: Vec<BasicBlock>,
        blocks: [SemanticKirBlockCorrespondenceV1; 1],
        statements: [SemanticKirStatementOperationSpanV1; 2],
        terminators: [SemanticKirTerminatorOperationSpanV1; 1],
    }

    fn operation_span_fixture() -> OperationSpanFixture {
        let semantic_function = SemanticFunctionIdV1::from_index(0);
        let semantic_block = SemanticBlockIdV1::from_index(7);
        let kernel_ir_block = BlockId(7);
        let expected = [ExpectedSemanticKirBlockCoverageV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block,
            source_statement_count: 2,
        }];
        let blocks = [SemanticKirBlockCorrespondenceV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block,
            source_statement_count: 2,
        }];
        let statements = [
            SemanticKirStatementOperationSpanV1 {
                semantic_function,
                semantic_block,
                statement_ordinal: 0,
                kernel_ir_block,
                first_operation_ordinal: 0,
                operation_count: 0,
            },
            SemanticKirStatementOperationSpanV1 {
                semantic_function,
                semantic_block,
                statement_ordinal: 1,
                kernel_ir_block,
                first_operation_ordinal: 0,
                operation_count: 2,
            },
        ];
        let terminators = [SemanticKirTerminatorOperationSpanV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block,
            first_operation_ordinal: 2,
            operation_count: 1,
        }];
        let operation = AmdGpuDiagnosticOperation::Trap.operation(None);
        let mut target = BasicBlock::new(kernel_ir_block);
        target.operations = vec![operation; 3];
        target.terminator = Some(Terminator::Return { values: vec![] });
        OperationSpanFixture {
            expected,
            target: vec![target],
            blocks,
            statements,
            terminators,
        }
    }

    #[test]
    fn zero_sized_arrays_are_bounded_by_structure_not_only_scalar_components() {
        let unit = unit_type();
        let array = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            unit.layout().clone(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(0),
                length: u64::MAX,
            },
        );
        let types = [unit, array];

        let lowering = lower_ssa_value_types(&types, SemanticTypeIdV1::from_index(1))
            .expect_err("huge zero-sized array must fail before iteration");
        assert!(lowering.to_string().contains("array length is too large"));

        let binding = binding_from_value_defs(&types, SemanticTypeIdV1::from_index(1), &[])
            .expect_err("huge zero-sized binding must fail before allocation");
        assert!(binding.to_string().contains("array length is too large"));
    }

    #[test]
    fn operation_spans_cover_zero_multi_and_terminator_emission_exactly_once() {
        let fixture = operation_span_fixture();
        assert!(validate_operation_correspondence_layout(
            &fixture.expected,
            &fixture.target,
            &fixture.blocks,
            &fixture.statements,
            &fixture.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn operation_span_validation_rejects_statement_omission() {
        let fixture = operation_span_fixture();
        assert!(!validate_operation_correspondence_layout(
            &fixture.expected,
            &fixture.target,
            &fixture.blocks,
            &fixture.statements[..1],
            &fixture.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn operation_span_validation_rejects_overlap() {
        let mut fixture = operation_span_fixture();
        fixture.statements[0].operation_count = 1;
        assert!(!validate_operation_correspondence_layout(
            &fixture.expected,
            &fixture.target,
            &fixture.blocks,
            &fixture.statements,
            &fixture.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn operation_span_validation_rejects_terminator_gaps_and_trailing_operations() {
        let mut gap = operation_span_fixture();
        gap.terminators[0].first_operation_ordinal = 1;
        assert!(!validate_operation_correspondence_layout(
            &gap.expected,
            &gap.target,
            &gap.blocks,
            &gap.statements,
            &gap.terminators,
            &[],
            None,
        ));

        let mut trailing = operation_span_fixture();
        trailing.target[0]
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        assert!(!validate_operation_correspondence_layout(
            &trailing.expected,
            &trailing.target,
            &trailing.blocks,
            &trailing.statements,
            &trailing.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn operation_span_validation_rejects_target_block_substitution() {
        let mut fixture = operation_span_fixture();
        fixture.target[0].id = BlockId(8);
        assert!(!validate_operation_correspondence_layout(
            &fixture.expected,
            &fixture.target,
            &fixture.blocks,
            &fixture.statements,
            &fixture.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn operation_span_validation_rejects_source_substitution() {
        let mut fixture = operation_span_fixture();
        fixture.statements[1].statement_ordinal = 0;
        assert!(!validate_operation_correspondence_layout(
            &fixture.expected,
            &fixture.target,
            &fixture.blocks,
            &fixture.statements,
            &fixture.terminators,
            &[],
            None,
        ));
    }

    #[test]
    fn synthetic_trap_rule_has_exact_block_and_operation_coverage() {
        let semantic_function = SemanticFunctionIdV1::from_index(0);
        let semantic_block = SemanticBlockIdV1::from_index(0);
        let expected = [ExpectedSemanticKirBlockCoverageV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block: BlockId(0),
            source_statement_count: 0,
        }];
        let blocks = [SemanticKirBlockCorrespondenceV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block: BlockId(0),
            source_statement_count: 0,
        }];
        let terminators = [SemanticKirTerminatorOperationSpanV1 {
            semantic_function,
            semantic_block,
            kernel_ir_block: BlockId(0),
            first_operation_ordinal: 0,
            operation_count: 0,
        }];
        let mut source = BasicBlock::new(BlockId(0));
        source.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        let mut synthetic_block = BasicBlock::new(BlockId(1));
        synthetic_block
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        synthetic_block.terminator = Some(Terminator::Unreachable);
        let target = [source, synthetic_block];
        let synthetic = [SemanticKirSyntheticOperationSpanV1 {
            rule: SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
            kernel_ir_block: BlockId(1),
            first_operation_ordinal: 0,
            operation_count: 1,
        }];

        assert!(validate_operation_correspondence_layout(
            &expected,
            &target,
            &blocks,
            &[],
            &terminators,
            &synthetic,
            Some(SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap),
        ));
        assert!(!validate_operation_correspondence_layout(
            &expected,
            &target,
            &blocks,
            &[],
            &terminators,
            &[],
            Some(SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap),
        ));

        let mut wrong_trap = target.clone();
        wrong_trap[1].operations[0] =
            Operation::new(Vec::new(), OperationKind::Constant(Constant::Bool(false)));
        assert!(!validate_operation_correspondence_layout(
            &expected,
            &wrong_trap,
            &blocks,
            &[],
            &terminators,
            &synthetic,
            Some(SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap),
        ));
    }
}
