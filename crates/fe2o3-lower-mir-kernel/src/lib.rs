#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod production_correspondence_evidence_v4;
mod production_correspondence_evidence_v5;
mod production_formal_memory_evidence_v4;
mod production_formal_memory_v1;
mod production_lineage_evidence_v3;
mod production_semantic_kir_v1;
pub use production_semantic_kir_v1::ProductionMemoryDischargeFailureV1;

pub use production_correspondence_evidence_v4::*;
pub use production_correspondence_evidence_v5::*;
pub use production_formal_memory_evidence_v4::*;
pub use production_formal_memory_v1::*;
pub use production_lineage_evidence_v3::*;
pub use production_semantic_kir_v1::*;

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{AlgorithmOp, KernelError};
use dialect_mir::{
    MAX_EXECUTABLE_BLOCK_PARAMETERS, MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_FUNCTIONS,
    MAX_EXECUTABLE_STATEMENTS, MirTypeId,
    pliron::{
        MirBlockOp, MirFunctionOp, MirModuleOp, MirModuleSnapshotError, MirReturnOp,
        MirSemanticOperationKind, MirSemanticSpanProvenance, MirSemanticStatementOp,
        MirSemanticTerminatorOp, MirSnapshotOperation, register_mir_dialect,
    },
};
use fe2o3_pliron::{
    ContextIdentity, ContextIdentityError, ensure_context_identity, require_context_identity,
};
use pliron::{
    context::{Context, Ptr},
    dialect::{Dialect, DialectName},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
};

/// Stable detached lowering service name.
pub const PASS_NAME: &str = "fe2o3-lower-mir-kernel";

const PASS_REGISTRATION_MARKER_KEY: &str = "fe2o3_lower_mir_kernel_pass_registration_v1";

/// Deterministic source-then-target dialect registration order.
pub const DIALECT_REGISTRATION_ORDER: [&str; 2] =
    [dialect_mir::DIALECT, dialect_kernel::DIALECT_NAME];

/// Hard bound on source modules per lowering invocation.
pub const MAX_SOURCE_MODULES: usize = 1;

/// Hard bound on source functions per lowering invocation.
pub const MAX_SOURCE_FUNCTIONS: usize = MAX_EXECUTABLE_FUNCTIONS;

/// Hard bound on total source CFG blocks per lowering invocation.
pub const MAX_SOURCE_BLOCKS: usize = MAX_EXECUTABLE_BLOCKS;

/// Hard bound on all inspected source operations, including structural roots.
pub const MAX_SOURCE_OPERATIONS: usize = MAX_EXECUTABLE_STATEMENTS;

/// Hard bound on target-neutral structured iteration rank.
pub const MAX_STRUCTURED_RANK: u32 = dialect_kernel::MAX_ITERATION_RANK;

/// Hard bound on emitted kernel algorithm roots.
pub const MAX_REWRITES: usize = MAX_SOURCE_FUNCTIONS;

#[derive(Debug)]
struct PassRegistrationMarker {
    context_identity: ContextIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PassRegistrationOutcome {
    Registered,
    AlreadyRegistered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PassRegistrationError {
    MarkerCollision,
    CorruptMarker,
    ContextIdentityExhausted,
    InvalidDialectName,
    KernelDialect(dialect_kernel::RegistrationError),
}

impl fmt::Display for PassRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerCollision => {
                formatter.write_str("MIR-to-kernel pass registration marker collision")
            }
            Self::CorruptMarker => {
                formatter.write_str("MIR-to-kernel pass registration marker is corrupt")
            }
            Self::ContextIdentityExhausted => {
                formatter.write_str("MIR-to-kernel context identity space is exhausted")
            }
            Self::InvalidDialectName => {
                formatter.write_str("a fixed lowering dialect name is invalid")
            }
            Self::KernelDialect(error) => {
                write!(formatter, "kernel dialect registration failed: {error:?}")
            }
        }
    }
}

impl Error for PassRegistrationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationState {
    Absent,
    Registered(ContextIdentity),
}

fn registration_marker_key() -> Result<Identifier, PassRegistrationError> {
    PASS_REGISTRATION_MARKER_KEY
        .try_into()
        .map_err(|_| PassRegistrationError::InvalidDialectName)
}

fn registration_state(context: &Context) -> Result<RegistrationState, PassRegistrationError> {
    let key = registration_marker_key()?;
    let Some(index) = context.aux_data_map.get(&key).copied() else {
        return Ok(RegistrationState::Absent);
    };
    let marker = context
        .aux_data
        .get(index)
        .ok_or(PassRegistrationError::CorruptMarker)?
        .downcast_ref::<PassRegistrationMarker>()
        .ok_or(PassRegistrationError::MarkerCollision)?;
    let context_identity = require_context_identity(context).map_err(map_context_identity_error)?;
    if marker.context_identity != context_identity {
        return Err(PassRegistrationError::CorruptMarker);
    }
    Ok(RegistrationState::Registered(context_identity))
}

fn map_context_identity_error(error: ContextIdentityError) -> PassRegistrationError {
    match error {
        ContextIdentityError::MarkerCollision => PassRegistrationError::MarkerCollision,
        ContextIdentityError::CorruptMarker => PassRegistrationError::CorruptMarker,
        ContextIdentityError::IdentitySpaceExhausted => {
            PassRegistrationError::ContextIdentityExhausted
        }
    }
}

fn register_pass(context: &mut Context) -> Result<PassRegistrationOutcome, PassRegistrationError> {
    match registration_state(context)? {
        RegistrationState::Registered(_) => {
            return Ok(PassRegistrationOutcome::AlreadyRegistered);
        }
        RegistrationState::Absent => {}
    }

    let context_identity = ensure_context_identity(context).map_err(map_context_identity_error)?;

    let mir_name = DialectName::try_new(DIALECT_REGISTRATION_ORDER[0])
        .map_err(|_| PassRegistrationError::InvalidDialectName)?;
    Dialect::register(context, &mir_name);
    register_mir_dialect(context);

    let kernel_name = DialectName::try_new(DIALECT_REGISTRATION_ORDER[1])
        .map_err(|_| PassRegistrationError::InvalidDialectName)?;
    dialect_kernel::register_dialect(context, &kernel_name)
        .map_err(PassRegistrationError::KernelDialect)?;

    let marker = context
        .aux_data
        .insert(Box::new(PassRegistrationMarker { context_identity }));
    context
        .aux_data_map
        .insert(registration_marker_key()?, marker);
    Ok(PassRegistrationOutcome::Registered)
}

/// Source resource category controlled by [`LoweringLimits`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitKind {
    /// Number of source module roots.
    Modules,
    /// Number of direct source functions.
    Functions,
    /// Total number of source CFG blocks.
    Blocks,
    /// Total source operations, including module and function roots.
    Operations,
    /// Number of emitted kernel roots.
    Rewrites,
}

/// Invalid bounded lowering configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// A resource limit was zero or exceeded its implementation hard cap.
    LimitOutOfBounds {
        /// Rejected resource category.
        kind: LimitKind,
        /// Rejected value.
        value: usize,
        /// Largest accepted value.
        hard_limit: usize,
    },
    /// The requested structured rank was zero or exceeded the kernel dialect cap.
    RankOutOfBounds(u32),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitOutOfBounds {
                kind,
                value,
                hard_limit,
            } => write!(
                formatter,
                "{kind:?} limit {value} is outside 1..={hard_limit}"
            ),
            Self::RankOutOfBounds(rank) => write!(
                formatter,
                "structured rank {rank} is outside 1..={MAX_STRUCTURED_RANK}"
            ),
        }
    }
}

impl Error for ConfigError {}

/// Bounded source traversal and rewrite limits for one invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweringLimits {
    max_modules: usize,
    max_functions: usize,
    max_blocks: usize,
    max_operations: usize,
    max_rewrites: usize,
}

impl LoweringLimits {
    /// Creates non-zero limits no larger than the implementation hard caps.
    pub fn new(
        max_modules: usize,
        max_functions: usize,
        max_blocks: usize,
        max_operations: usize,
        max_rewrites: usize,
    ) -> Result<Self, ConfigError> {
        check_config_limit(LimitKind::Modules, max_modules, MAX_SOURCE_MODULES)?;
        check_config_limit(LimitKind::Functions, max_functions, MAX_SOURCE_FUNCTIONS)?;
        check_config_limit(LimitKind::Blocks, max_blocks, MAX_SOURCE_BLOCKS)?;
        check_config_limit(LimitKind::Operations, max_operations, MAX_SOURCE_OPERATIONS)?;
        check_config_limit(LimitKind::Rewrites, max_rewrites, MAX_REWRITES)?;
        Ok(Self {
            max_modules,
            max_functions,
            max_blocks,
            max_operations,
            max_rewrites,
        })
    }

    /// Returns the source module limit.
    pub const fn max_modules(self) -> usize {
        self.max_modules
    }

    /// Returns the direct source function limit.
    pub const fn max_functions(self) -> usize {
        self.max_functions
    }

    /// Returns the total source block limit.
    pub const fn max_blocks(self) -> usize {
        self.max_blocks
    }

    /// Returns the total inspected source operation limit.
    pub const fn max_operations(self) -> usize {
        self.max_operations
    }

    /// Returns the emitted kernel-root limit.
    pub const fn max_rewrites(self) -> usize {
        self.max_rewrites
    }
}

impl Default for LoweringLimits {
    fn default() -> Self {
        Self {
            max_modules: MAX_SOURCE_MODULES,
            max_functions: 64,
            max_blocks: 1_024,
            max_operations: 16_384,
            max_rewrites: 64,
        }
    }
}

fn check_config_limit(kind: LimitKind, value: usize, hard_limit: usize) -> Result<(), ConfigError> {
    if value == 0 || value > hard_limit {
        return Err(ConfigError::LimitOutOfBounds {
            kind,
            value,
            hard_limit,
        });
    }
    Ok(())
}

/// Immutable bounded configuration for one MIR-to-kernel invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringConfig {
    limits: LoweringLimits,
    iteration_rank: u32,
}

impl LoweringConfig {
    /// Creates a configuration with a target-neutral structured rank.
    pub fn new(limits: LoweringLimits, iteration_rank: u32) -> Result<Self, ConfigError> {
        if iteration_rank == 0 || iteration_rank > MAX_STRUCTURED_RANK {
            return Err(ConfigError::RankOutOfBounds(iteration_rank));
        }
        Ok(Self {
            limits,
            iteration_rank,
        })
    }

    /// Returns the traversal and rewrite limits.
    pub const fn limits(&self) -> LoweringLimits {
        self.limits
    }

    /// Returns the target-neutral structured rank used for every admitted function.
    pub const fn iteration_rank(&self) -> u32 {
        self.iteration_rank
    }
}

/// Pointer-independent evidence for one admitted MIR operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOperationEvidence {
    /// Canonical first operation of a MIR CFG block.
    BlockMarker {
        /// Verified MIR block identifier.
        block_id: u32,
    },
    /// Place-based MIR return terminator.
    Return,
    /// Exact optimized-rustc return retained through the typed MIR boundary.
    SemanticReturn {
        /// Stable identity of the exact rustc terminator.
        identity: [u64; 4],
        /// Exact expansion and resolved call-site coordinates reported by rustc.
        provenance: MirSemanticSpanProvenance,
    },
}

/// Pointer-independent source evidence for one MIR CFG block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBlockEvidence {
    ordinal: usize,
    block_id: u32,
    operations: Vec<SourceOperationEvidence>,
}

impl SourceBlockEvidence {
    /// Returns the block's zero-based source order.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the verified canonical MIR block identifier.
    pub const fn block_id(&self) -> u32 {
        self.block_id
    }

    /// Returns admitted MIR operations in exact source order.
    pub fn operations(&self) -> &[SourceOperationEvidence] {
        &self.operations
    }
}

/// Pointer-independent source evidence for one MIR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFunctionEvidence {
    ordinal: usize,
    identity: String,
    argument_type_ids: Vec<MirTypeId>,
    blocks: Vec<SourceBlockEvidence>,
}

impl SourceFunctionEvidence {
    /// Returns the function's zero-based order in the source module.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the exact verified MIR function identity attribute.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns verified MIR type-table references in argument order.
    pub fn argument_type_ids(&self) -> &[MirTypeId] {
        &self.argument_type_ids
    }

    /// Returns source block evidence in canonical CFG order.
    pub fn blocks(&self) -> &[SourceBlockEvidence] {
        &self.blocks
    }
}

/// Pointer-independent observation of the admitted MIR module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceModuleEvidence {
    identity: String,
    functions: Vec<SourceFunctionEvidence>,
    block_count: usize,
    operation_count: usize,
}

impl SourceModuleEvidence {
    /// Returns the exact verified MIR module identity attribute.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns source function evidence in module order.
    pub fn functions(&self) -> &[SourceFunctionEvidence] {
        &self.functions
    }

    /// Returns the total number of admitted source CFG blocks.
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    /// Returns all inspected operations, including module and function roots.
    pub const fn operation_count(&self) -> usize {
        self.operation_count
    }
}

/// One deterministic target-neutral materialization step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweringStep {
    source_function_ordinal: usize,
    iteration_rank: u32,
}

impl LoweringStep {
    /// Returns the source function represented by this step.
    pub const fn source_function_ordinal(self) -> usize {
        self.source_function_ordinal
    }

    /// Returns the emitted structured algorithm rank.
    pub const fn iteration_rank(self) -> u32 {
        self.iteration_rank
    }
}

/// Deterministic pointer-independent record of one successful lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringRecord {
    source: SourceModuleEvidence,
    steps: Vec<LoweringStep>,
}

impl LoweringRecord {
    /// Returns the source identity and structure observation.
    pub const fn source(&self) -> &SourceModuleEvidence {
        &self.source
    }

    /// Returns target materialization steps in source function order.
    pub fn steps(&self) -> &[LoweringStep] {
        &self.steps
    }

    /// Returns the exact number of emitted kernel roots.
    pub fn rewrite_count(&self) -> usize {
        self.steps.len()
    }
}

/// Pointer-independent observation of one successful bounded lowering.
///
/// This value contains no Pliron arena handle and can be retained, compared,
/// or moved independently of the private conformance context that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringResult {
    config: LoweringConfig,
    record: LoweringRecord,
}

impl LoweringResult {
    /// Returns the immutable configuration used to produce this result.
    pub const fn config(&self) -> &LoweringConfig {
        &self.config
    }

    /// Returns the pointer-independent deterministic lowering record.
    pub const fn record(&self) -> &LoweringRecord {
        &self.record
    }

    /// This observation grants no proof, publication, target, or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MaterializedLoweringResult {
    source_root: Ptr<Operation>,
    observation: LoweringResult,
    operations: Vec<Ptr<Operation>>,
    context_identity: ContextIdentity,
}

impl MaterializedLoweringResult {
    fn validate(&self, context: &Context) -> Result<(), PostconditionError> {
        validate_postconditions(context, self)
    }
}

/// A failed output invariant, indicating stale evidence or mutated IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostconditionError {
    /// The supplied context does not own this result's arena pointers.
    ContextMismatch,
    /// The live source no longer satisfies the bounded input contract.
    SourceNoLongerValid,
    /// The live source differs from the recorded identity or structure.
    SourceEvidenceMismatch,
    /// Lowering-step and emitted-operation counts differ.
    OperationCountMismatch,
    /// The result escaped the configured or hard rewrite bound.
    RewriteBoundExceeded,
    /// An emitted operation failed kernel-dialect verification.
    InvalidKernelOperation {
        /// Zero-based emitted operation index.
        index: usize,
    },
    /// An emitted operation does not match its deterministic lowering step.
    UnexpectedKernelOperation {
        /// Zero-based emitted operation index.
        index: usize,
    },
}

impl fmt::Display for PostconditionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextMismatch => {
                formatter.write_str("lowering result belongs to a different Pliron context")
            }
            Self::SourceNoLongerValid => {
                formatter.write_str("the lowering source is no longer valid")
            }
            Self::SourceEvidenceMismatch => {
                formatter.write_str("the lowering source no longer matches recorded evidence")
            }
            Self::OperationCountMismatch => {
                formatter.write_str("lowering step and kernel operation counts differ")
            }
            Self::RewriteBoundExceeded => {
                formatter.write_str("lowering result exceeds its rewrite bound")
            }
            Self::InvalidKernelOperation { index } => {
                write!(
                    formatter,
                    "emitted kernel operation {index} failed verification"
                )
            }
            Self::UnexpectedKernelOperation { index } => write!(
                formatter,
                "emitted kernel operation {index} does not match its lowering step"
            ),
        }
    }
}

impl Error for PostconditionError {}

/// Source entity category used in malformed-input diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEntityKind {
    /// MIR module root.
    Module,
    /// MIR function root.
    Function,
    /// MIR block marker.
    BlockMarker,
    /// MIR return terminator.
    Return,
    /// Typed rustc statement observation.
    SemanticStatement,
    /// Typed rustc terminator observation.
    SemanticTerminator,
}

/// Terminal checked-lowering failure. No variant permits fallback execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringError {
    /// Private pass registration did not complete in this context.
    PassNotRegistered,
    /// The pass registration marker is foreign or corrupt.
    RegistrationCorrupt,
    /// The source root is not `mir.module`.
    UnsupportedSourceOperation,
    /// A source count exceeded the configured limit.
    SourceLimitExceeded {
        /// Exhausted resource category.
        kind: LimitKind,
        /// First rejected count.
        observed: usize,
        /// Configured maximum count.
        limit: usize,
    },
    /// A known source operation had an unbounded or malformed closed shape.
    MalformedSourceEntity(SourceEntityKind),
    /// A direct module child was not `mir.func`.
    UnsupportedModuleChild {
        /// Zero-based child operation index.
        ordinal: usize,
    },
    /// A CFG block contained an operation outside the supported MIR shell.
    UnsupportedMirOperation {
        /// Zero-based source function index.
        function: usize,
        /// Zero-based CFG block index.
        block: usize,
        /// Zero-based operation index in the block.
        operation: usize,
    },
    /// A typed rustc MIR operation is preserved exactly but not yet modeled by
    /// this target-neutral lowering version.
    UnsupportedRustSemanticOperation {
        /// Zero-based source function index.
        function: usize,
        /// Zero-based CFG block index.
        block: usize,
        /// Exact rustc operation ordinal within the block.
        ordinal: u32,
        /// Typed rustc MIR classification.
        kind: MirSemanticOperationKind,
        /// Exact rustc expansion and call-site coordinates for the rejection.
        provenance: MirSemanticSpanProvenance,
    },
    /// The source module has no function to transform.
    EmptyModule,
    /// Pliron or MIR-dialect verification rejected the bounded source.
    SourceVerificationFailed,
    /// A function argument was not a bounded `mir.type_ref`.
    UnsupportedArgumentType {
        /// Zero-based source function index.
        function: usize,
        /// Zero-based function argument index.
        argument: usize,
    },
    /// Required deterministic rewrites exceeded the request limit.
    RewriteLimitExceeded {
        /// Number of kernel roots required.
        required: usize,
        /// Caller-provided rewrite limit.
        limit: usize,
    },
    /// Kernel algorithm construction failed despite validated configuration.
    TargetConstructionFailed(KernelError),
    /// Emitted target-neutral kernel IR failed postcondition validation.
    Postcondition(PostconditionError),
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassNotRegistered => formatter.write_str("lowering pass is not registered"),
            Self::RegistrationCorrupt => {
                formatter.write_str("lowering pass registration is corrupt")
            }
            Self::UnsupportedSourceOperation => {
                formatter.write_str("source operation is not mir.module")
            }
            Self::SourceLimitExceeded {
                kind,
                observed,
                limit,
            } => write!(
                formatter,
                "source {kind:?} count {observed} exceeds configured limit {limit}"
            ),
            Self::MalformedSourceEntity(entity) => {
                write!(formatter, "malformed bounded source entity: {entity:?}")
            }
            Self::UnsupportedModuleChild { ordinal } => {
                write!(formatter, "source module child {ordinal} is not mir.func")
            }
            Self::UnsupportedMirOperation {
                function,
                block,
                operation,
            } => write!(
                formatter,
                "unsupported MIR operation at function {function}, block {block}, operation {operation}"
            ),
            Self::UnsupportedRustSemanticOperation {
                function,
                block,
                ordinal,
                kind,
                provenance,
            } => write!(
                formatter,
                "unsupported typed rustc MIR operation {kind:?} at function {function}, block {block}, ordinal {ordinal}, expansion coordinates {:?}, call-site coordinates {:?}",
                provenance.expansion().coordinates(),
                provenance.call_site().coordinates()
            ),
            Self::EmptyModule => formatter.write_str("source MIR module has no functions"),
            Self::SourceVerificationFailed => {
                formatter.write_str("source MIR module failed verification")
            }
            Self::UnsupportedArgumentType { function, argument } => write!(
                formatter,
                "source function {function} argument {argument} is not mir.type_ref"
            ),
            Self::RewriteLimitExceeded { required, limit } => write!(
                formatter,
                "lowering requires {required} rewrites but the limit is {limit}"
            ),
            Self::TargetConstructionFailed(error) => {
                write!(formatter, "kernel algorithm construction failed: {error}")
            }
            Self::Postcondition(error) => write!(formatter, "lowering postcondition: {error}"),
        }
    }
}

impl Error for LoweringError {}

include!("conformance_lowering_v1.rs");

fn require_registration(context: &Context) -> Result<ContextIdentity, LoweringError> {
    match registration_state(context) {
        Ok(RegistrationState::Registered(context_identity)) => Ok(context_identity),
        Ok(RegistrationState::Absent) => Err(LoweringError::PassNotRegistered),
        Err(_) => Err(LoweringError::RegistrationCorrupt),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceCounts {
    functions: usize,
    blocks: usize,
    operations: usize,
}

fn inspect_source(
    context: &Context,
    source: Ptr<Operation>,
    config: &LoweringConfig,
) -> Result<SourceModuleEvidence, LoweringError> {
    let source_ref = source
        .try_deref(context)
        .map_err(|_| LoweringError::SourceVerificationFailed)?;
    drop(source_ref);
    catch_unwind(AssertUnwindSafe(|| {
        inspect_live_source(context, source, config)
    }))
    .unwrap_or(Err(LoweringError::SourceVerificationFailed))
}

fn inspect_live_source(
    context: &Context,
    source: Ptr<Operation>,
    config: &LoweringConfig,
) -> Result<SourceModuleEvidence, LoweringError> {
    if !Operation::is_op::<MirModuleOp>(source, context) {
        return Err(LoweringError::UnsupportedSourceOperation);
    }
    let counts = preflight_source(context, source, config.limits)?;
    verify_operation(source, context).map_err(|_| LoweringError::SourceVerificationFailed)?;
    collect_source_evidence(context, source, counts)
}

fn preflight_source(
    context: &Context,
    source: Ptr<Operation>,
    limits: LoweringLimits,
) -> Result<SourceCounts, LoweringError> {
    if limits.max_modules < 1 {
        return Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Modules,
            observed: 1,
            limit: limits.max_modules,
        });
    }
    verify_closed_shape(context, source, SourceEntityKind::Module, 1, 2, 0)?;
    let source_ref = source.deref(context);
    let region = source_ref.get_region(0);
    let mut module_blocks = region.deref(context).iter(context);
    let Some(body) = module_blocks.next() else {
        return Err(LoweringError::MalformedSourceEntity(
            SourceEntityKind::Module,
        ));
    };
    if module_blocks.next().is_some() {
        return Err(LoweringError::MalformedSourceEntity(
            SourceEntityKind::Module,
        ));
    }
    if body.deref(context).get_num_arguments() != 0 {
        return Err(LoweringError::MalformedSourceEntity(
            SourceEntityKind::Module,
        ));
    }
    drop(source_ref);

    let mut counts = SourceCounts {
        operations: 1,
        ..SourceCounts::default()
    };
    check_observed_limit(
        LimitKind::Operations,
        counts.operations,
        limits.max_operations,
    )?;

    for (function_index, operation) in body.deref(context).iter(context).enumerate() {
        counts.functions += 1;
        check_observed_limit(LimitKind::Functions, counts.functions, limits.max_functions)?;
        charge_operation(&mut counts, limits.max_operations)?;
        if !Operation::is_op::<MirFunctionOp>(operation, context) {
            return Err(LoweringError::UnsupportedModuleChild {
                ordinal: function_index,
            });
        }
        verify_closed_shape(context, operation, SourceEntityKind::Function, 1, 3, 0)?;

        let function_ref = operation.deref(context);
        let function_region = function_ref.get_region(0);
        drop(function_ref);
        for (block_index, block) in function_region.deref(context).iter(context).enumerate() {
            counts.blocks += 1;
            check_observed_limit(LimitKind::Blocks, counts.blocks, limits.max_blocks)?;
            if block.deref(context).get_num_arguments() > MAX_EXECUTABLE_BLOCK_PARAMETERS {
                return Err(LoweringError::SourceVerificationFailed);
            }
            for (operation_index, block_operation) in block.deref(context).iter(context).enumerate()
            {
                charge_operation(&mut counts, limits.max_operations)?;
                if Operation::is_op::<MirBlockOp>(block_operation, context) {
                    verify_closed_shape(
                        context,
                        block_operation,
                        SourceEntityKind::BlockMarker,
                        0,
                        1,
                        0,
                    )?;
                } else if Operation::is_op::<MirReturnOp>(block_operation, context) {
                    verify_closed_shape(
                        context,
                        block_operation,
                        SourceEntityKind::Return,
                        0,
                        0,
                        0,
                    )?;
                } else if let Some(statement) =
                    Operation::get_op::<MirSemanticStatementOp>(block_operation, context)
                {
                    verify_closed_shape(
                        context,
                        block_operation,
                        SourceEntityKind::SemanticStatement,
                        0,
                        5,
                        0,
                    )?;
                    let semantic = statement
                        .semantic_snapshot(context)
                        .ok_or(LoweringError::SourceVerificationFailed)?;
                    return Err(LoweringError::UnsupportedRustSemanticOperation {
                        function: function_index,
                        block: block_index,
                        ordinal: semantic.ordinal(),
                        kind: semantic.kind(),
                        provenance: semantic.provenance(),
                    });
                } else if let Some(terminator) =
                    Operation::get_op::<MirSemanticTerminatorOp>(block_operation, context)
                {
                    let semantic = terminator
                        .semantic_snapshot(context)
                        .ok_or(LoweringError::SourceVerificationFailed)?;
                    verify_closed_shape(
                        context,
                        block_operation,
                        SourceEntityKind::SemanticTerminator,
                        0,
                        6,
                        semantic.successors().len(),
                    )?;
                    if semantic.kind() != MirSemanticOperationKind::TerminatorReturn
                        || !semantic.successors().is_empty()
                    {
                        return Err(LoweringError::UnsupportedRustSemanticOperation {
                            function: function_index,
                            block: block_index,
                            ordinal: semantic.ordinal(),
                            kind: semantic.kind(),
                            provenance: semantic.provenance(),
                        });
                    }
                } else {
                    return Err(LoweringError::UnsupportedMirOperation {
                        function: function_index,
                        block: block_index,
                        operation: operation_index,
                    });
                }
            }
        }
    }

    if counts.functions == 0 {
        return Err(LoweringError::EmptyModule);
    }
    Ok(counts)
}

fn verify_closed_shape(
    context: &Context,
    operation: Ptr<Operation>,
    kind: SourceEntityKind,
    regions: usize,
    attributes: usize,
    successors: usize,
) -> Result<(), LoweringError> {
    let operation = operation.deref(context);
    if operation.get_num_operands() != 0
        || operation.get_num_results() != 0
        || operation.get_num_successors() != successors
        || operation.num_regions() != regions
        || operation.attributes.0.len() != attributes
    {
        return Err(LoweringError::MalformedSourceEntity(kind));
    }
    Ok(())
}

fn charge_operation(counts: &mut SourceCounts, limit: usize) -> Result<(), LoweringError> {
    counts.operations += 1;
    check_observed_limit(LimitKind::Operations, counts.operations, limit)
}

fn check_observed_limit(
    kind: LimitKind,
    observed: usize,
    limit: usize,
) -> Result<(), LoweringError> {
    if observed > limit {
        return Err(LoweringError::SourceLimitExceeded {
            kind,
            observed,
            limit,
        });
    }
    Ok(())
}

fn collect_source_evidence(
    context: &Context,
    source: Ptr<Operation>,
    counts: SourceCounts,
) -> Result<SourceModuleEvidence, LoweringError> {
    let module = MirModuleOp::from_operation(source);
    let identity = module
        .get_attr_module_identity(context)
        .ok_or(LoweringError::SourceVerificationFailed)?
        .as_str()
        .to_owned();
    let mut functions = Vec::with_capacity(counts.functions);

    let body = module
        .body(context)
        .map_err(|_| LoweringError::SourceVerificationFailed)?;

    let snapshots = body
        .semantic_functions(context)
        .map_err(|error| match error {
            MirModuleSnapshotError::UnsupportedArgumentType { function, argument } => {
                LoweringError::UnsupportedArgumentType { function, argument }
            }
            MirModuleSnapshotError::Handle(_) | MirModuleSnapshotError::MalformedModule => {
                LoweringError::SourceVerificationFailed
            }
        })?;

    for (function_index, function) in snapshots.into_iter().enumerate() {
        let mut blocks = Vec::with_capacity(function.blocks().len());
        for (block_index, block) in function.blocks().iter().enumerate() {
            let mut operations = Vec::with_capacity(block.operations().len());
            for operation in block.operations() {
                let evidence = match operation {
                    MirSnapshotOperation::BlockMarker(block_id) => {
                        SourceOperationEvidence::BlockMarker {
                            block_id: block_id.0,
                        }
                    }
                    MirSnapshotOperation::Return => SourceOperationEvidence::Return,
                    MirSnapshotOperation::SemanticStatement(semantic) => {
                        return Err(LoweringError::UnsupportedRustSemanticOperation {
                            function: function_index,
                            block: block_index,
                            ordinal: semantic.ordinal(),
                            kind: semantic.kind(),
                            provenance: semantic.provenance(),
                        });
                    }
                    MirSnapshotOperation::SemanticTerminator(semantic) => {
                        if semantic.kind() != MirSemanticOperationKind::TerminatorReturn
                            || !semantic.successors().is_empty()
                        {
                            return Err(LoweringError::UnsupportedRustSemanticOperation {
                                function: function_index,
                                block: block_index,
                                ordinal: semantic.ordinal(),
                                kind: semantic.kind(),
                                provenance: semantic.provenance(),
                            });
                        }
                        SourceOperationEvidence::SemanticReturn {
                            identity: semantic.identity(),
                            provenance: semantic.provenance(),
                        }
                    }
                };
                operations.push(evidence);
            }
            blocks.push(SourceBlockEvidence {
                ordinal: block_index,
                block_id: block.block_id().0,
                operations,
            });
        }
        functions.push(SourceFunctionEvidence {
            ordinal: function_index,
            identity: function.identity().to_owned(),
            argument_type_ids: function.argument_type_ids().to_vec(),
            blocks,
        });
    }

    Ok(SourceModuleEvidence {
        identity,
        functions,
        block_count: counts.blocks,
        operation_count: counts.operations,
    })
}

fn build_steps(
    source: &SourceModuleEvidence,
    config: &LoweringConfig,
) -> Result<Vec<LoweringStep>, LoweringError> {
    let required = source.functions.len();
    if required > config.limits.max_rewrites {
        return Err(LoweringError::RewriteLimitExceeded {
            required,
            limit: config.limits.max_rewrites,
        });
    }
    Ok(source
        .functions
        .iter()
        .map(|function| LoweringStep {
            source_function_ordinal: function.ordinal,
            iteration_rank: config.iteration_rank,
        })
        .collect())
}

fn materialize_steps(
    context: &mut Context,
    steps: &[LoweringStep],
) -> Result<Vec<Ptr<Operation>>, LoweringError> {
    steps
        .iter()
        .map(|step| {
            AlgorithmOp::new(context, step.iteration_rank)
                .map(|operation| operation.get_operation())
                .map_err(LoweringError::TargetConstructionFailed)
        })
        .collect()
}

fn validate_postconditions(
    context: &Context,
    result: &MaterializedLoweringResult,
) -> Result<(), PostconditionError> {
    match registration_state(context) {
        Ok(RegistrationState::Registered(context_identity))
            if context_identity == result.context_identity => {}
        _ => return Err(PostconditionError::ContextMismatch),
    }
    let source = inspect_source(context, result.source_root, &result.observation.config)
        .map_err(|_| PostconditionError::SourceNoLongerValid)?;
    if source != result.observation.record.source {
        return Err(PostconditionError::SourceEvidenceMismatch);
    }
    if result.observation.record.steps.len() != result.operations.len()
        || result.observation.record.steps.len() != result.observation.record.source.functions.len()
    {
        return Err(PostconditionError::OperationCountMismatch);
    }
    if result.operations.len() > result.observation.config.limits.max_rewrites
        || result.operations.len() > MAX_REWRITES
    {
        return Err(PostconditionError::RewriteBoundExceeded);
    }

    for (index, (step, operation)) in result
        .observation
        .record
        .steps
        .iter()
        .zip(&result.operations)
        .enumerate()
    {
        let operation_ref = operation
            .try_deref(context)
            .map_err(|_| PostconditionError::InvalidKernelOperation { index })?;
        drop(operation_ref);
        catch_unwind(AssertUnwindSafe(|| verify_operation(*operation, context)))
            .map_err(|_| PostconditionError::InvalidKernelOperation { index })?
            .map_err(|_| PostconditionError::InvalidKernelOperation { index })?;
        let matches = catch_unwind(AssertUnwindSafe(|| {
            kernel_operation_matches(context, *operation, step.iteration_rank)
        }))
        .map_err(|_| PostconditionError::InvalidKernelOperation { index })?;
        if step.source_function_ordinal != index
            || step.iteration_rank != result.observation.config.iteration_rank
            || !matches
        {
            return Err(PostconditionError::UnexpectedKernelOperation { index });
        }
    }
    Ok(())
}

fn kernel_operation_matches(context: &Context, operation: Ptr<Operation>, rank: u32) -> bool {
    let Some(algorithm) = Operation::get_op::<AlgorithmOp>(operation, context) else {
        return false;
    };
    let operation_ref = operation.deref(context);
    let closed_shape = operation_ref.get_num_operands() == 0
        && operation_ref.get_num_results() == 1
        && operation_ref.get_num_successors() == 0
        && operation_ref.num_regions() == 0
        && operation_ref.attributes.0.len() == 1;
    drop(operation_ref);
    closed_shape
        && algorithm
            .iteration_domain(context)
            .is_some_and(|domain| domain.rank() == rank)
}

#[cfg(test)]
mod raw_owner_safety_tests {
    use super::*;
    use dialect_kernel::IterationDomainAttr;
    use dialect_mir::{
        MirBlockId,
        pliron::{MirBlockIdAttr, MirDialectLimits, MirIdentityAttr, MirLimitsAttr},
    };
    use pliron::{
        basic_block::BasicBlock,
        builtin::{attributes::TypeAttr, types::FunctionType, types::UnitType},
        r#type::TypeHandle,
    };
    use std::{any::Any, panic::AssertUnwindSafe};

    fn config() -> LoweringConfig {
        LoweringConfig::new(
            LoweringLimits::new(1, 2, 2, 16, 2).expect("bounded limits"),
            1,
        )
        .expect("bounded rank")
    }

    fn populated_context(identity: &str) -> (Context, MaterializedLoweringResult) {
        let mut context = Context::new();
        register_pass(&mut context).expect("private registration");
        let input = MirKernelLoweringConformanceInputV1::new(
            identity,
            vec![MirKernelLoweringConformanceFunctionV1::new(
                format!("{identity}::kernel"),
                vec![MirTypeId(1)],
            )],
        );
        let source = materialize_conformance_input_v1(&mut context, &input).expect("valid input");
        let mut service = MirKernelLoweringPass::new(config());
        service
            .run_checked(source, &mut context)
            .expect("private lowering");
        let result = service.take_result().expect("materialized result");
        (context, result)
    }

    fn module_with_functions(context: &mut Context, identity: &str, count: usize) -> MirModuleOp {
        let limits = MirDialectLimits::new(count.max(1), 4, 128).expect("bounded MIR limits");
        let module = MirModuleOp::try_new(context, identity, limits).expect("valid MIR module");
        for index in 0..count {
            module
                .append_function(context, format!("{identity}::function_{index}"), &[])
                .expect("valid MIR function");
        }
        module
    }

    fn raw_region_head(
        operation: Ptr<Operation>,
        context: &Context,
    ) -> Ptr<pliron::basic_block::BasicBlock> {
        operation
            .deref(context)
            .get_region(0)
            .deref(context)
            .get_head()
            .expect("operation region has a head block")
    }

    fn raw_module_body(module: &MirModuleOp, context: &Context) -> Ptr<BasicBlock> {
        raw_region_head(module.get_operation(), context)
    }

    fn raw_function_entry(function: &MirFunctionOp, context: &Context) -> Ptr<BasicBlock> {
        raw_region_head(function.get_operation(), context)
    }

    fn take_registration_marker(context: &mut Context) -> Box<dyn Any> {
        let key = registration_marker_key().expect("fixed key");
        let index = context
            .aux_data_map
            .remove(&key)
            .expect("marker is indexed");
        context.aux_data.remove(index).expect("marker is live")
    }

    fn install_registration_marker(context: &mut Context, marker: Box<dyn Any>) {
        let index = context.aux_data.insert(marker);
        context
            .aux_data_map
            .insert(registration_marker_key().expect("fixed key"), index);
    }

    #[test]
    fn equal_slot_foreign_context_rejects_result_before_pointer_dereference() {
        let (owner_context, owner_result) = populated_context("owner");
        let (foreign_context, foreign_result) = populated_context("foreign");

        assert_eq!(
            format!("{:?}", owner_result.source_root),
            format!("{:?}", foreign_result.source_root)
        );
        assert_ne!(owner_result.source_root, foreign_result.source_root);
        assert_eq!(
            owner_result.operations.len(),
            foreign_result.operations.len()
        );
        for (owner_operation, foreign_operation) in owner_result
            .operations
            .iter()
            .zip(foreign_result.operations.iter())
        {
            assert_eq!(
                format!("{owner_operation:?}"),
                format!("{foreign_operation:?}")
            );
            assert_ne!(owner_operation, foreign_operation);
        }
        assert_eq!(owner_result.validate(&owner_context), Ok(()));
        let rejection =
            std::panic::catch_unwind(AssertUnwindSafe(|| owner_result.validate(&foreign_context)))
                .expect("foreign validation must not unwind");
        assert_eq!(rejection, Err(PostconditionError::ContextMismatch));
    }

    #[test]
    fn transplanted_session_marker_cannot_transfer_result_ownership() {
        let (mut owner_context, owner_result) = populated_context("owner");
        let (mut foreign_context, _) = populated_context("foreign");
        let owner_marker = take_registration_marker(&mut owner_context);
        drop(take_registration_marker(&mut foreign_context));
        install_registration_marker(&mut foreign_context, owner_marker);

        assert_eq!(
            registration_state(&foreign_context),
            Err(PassRegistrationError::CorruptMarker)
        );
        let rejection =
            std::panic::catch_unwind(AssertUnwindSafe(|| owner_result.validate(&foreign_context)))
                .expect("transplanted-marker validation must not unwind");
        assert_eq!(rejection, Err(PostconditionError::ContextMismatch));
    }

    #[test]
    fn erased_private_handles_return_typed_errors_without_unwinding() {
        let (mut source_context, source_result) = populated_context("source");
        Operation::erase(source_result.source_root, &mut source_context);
        let source_rejection =
            std::panic::catch_unwind(AssertUnwindSafe(|| source_result.validate(&source_context)))
                .expect("erased source validation must not unwind");
        assert_eq!(
            source_rejection,
            Err(PostconditionError::SourceNoLongerValid)
        );

        let (mut output_context, output_result) = populated_context("output");
        Operation::erase(output_result.operations[0], &mut output_context);
        let output_rejection =
            std::panic::catch_unwind(AssertUnwindSafe(|| output_result.validate(&output_context)))
                .expect("erased output validation must not unwind");
        assert_eq!(
            output_rejection,
            Err(PostconditionError::InvalidKernelOperation { index: 0 })
        );
    }

    #[test]
    fn registration_rejects_missing_colliding_and_corrupt_markers() {
        let mut missing = Context::new();
        let source = module_with_functions(&mut missing, "missing", 1);
        let mut service = MirKernelLoweringPass::new(config());
        assert_eq!(
            service.run_checked(source.get_operation(), &mut missing),
            Err(LoweringError::PassNotRegistered)
        );

        let mut collision = Context::new();
        let key = registration_marker_key().expect("fixed key");
        let hostile = collision.aux_data.insert(Box::new(17_u32));
        collision.aux_data_map.insert(key, hostile);
        assert_eq!(
            register_pass(&mut collision),
            Err(PassRegistrationError::MarkerCollision)
        );
        collision.aux_data.remove(hostile);
        assert_eq!(
            register_pass(&mut collision),
            Err(PassRegistrationError::CorruptMarker)
        );

        let mut kernel_collision = Context::new();
        let kernel_key: Identifier = "fe2o3_dialect_kernel_registration_v1"
            .try_into()
            .expect("fixed kernel marker key");
        let hostile = kernel_collision.aux_data.insert(Box::new(23_u32));
        kernel_collision.aux_data_map.insert(kernel_key, hostile);
        assert_eq!(
            register_pass(&mut kernel_collision),
            Err(PassRegistrationError::KernelDialect(
                dialect_kernel::RegistrationError::MarkerCollision
            ))
        );
    }

    #[test]
    fn unsupported_raw_operations_and_foreign_argument_types_fail_closed() {
        let mut context = Context::new();
        register_pass(&mut context).expect("private registration");
        let mut service = MirKernelLoweringPass::new(config());
        let foreign = AlgorithmOp::new(&mut context, 1).expect("valid foreign operation");
        assert_eq!(
            service.run_checked(foreign.get_operation(), &mut context),
            Err(LoweringError::UnsupportedSourceOperation)
        );
        assert!(service.last_result.is_none());

        let limits = MirDialectLimits::new(1, 1, 64).expect("bounded MIR limits");
        let module =
            MirModuleOp::try_new(&mut context, "foreign-type", limits).expect("valid MIR module");
        let unit: TypeHandle = UnitType::get(&context).into();
        let signature = FunctionType::get(&context, vec![unit], vec![]);
        let operation = Operation::new(
            &mut context,
            MirFunctionOp::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            1,
        );
        let function = MirFunctionOp::from_operation(operation);
        function.set_attr_function_identity(&context, MirIdentityAttr::new("foreign-type::kernel"));
        function.set_attr_function_limits(&context, MirLimitsAttr::new(limits));
        function.set_attr_function_signature(&context, TypeAttr::new(signature.into()));
        let entry = BasicBlock::new(
            &mut context,
            Some("bb0".try_into().expect("valid label")),
            vec![unit],
        );
        entry.insert_at_front(operation.deref(&context).get_region(0), &context);
        MirBlockOp::new(&mut context, MirBlockId(0))
            .get_operation()
            .insert_at_back(entry, &context);
        MirReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        operation.insert_at_back(raw_module_body(&module, &context), &context);

        assert_eq!(
            service.run_checked(module.get_operation(), &mut context),
            Err(LoweringError::UnsupportedArgumentType {
                function: 0,
                argument: 0,
            })
        );
        assert!(service.last_result.is_none());
    }

    #[test]
    fn verifier_failure_clears_a_prior_success() {
        let mut context = Context::new();
        register_pass(&mut context).expect("private registration");
        let source = module_with_functions(&mut context, "prior-success", 1);
        let mut service = MirKernelLoweringPass::new(config());
        service
            .run_checked(source.get_operation(), &mut context)
            .expect("initial lowering");
        assert!(service.last_result.is_some());

        let function = Operation::get_op::<MirFunctionOp>(
            raw_module_body(&source, &context)
                .deref(&context)
                .get_head()
                .expect("function"),
            &context,
        )
        .expect("MIR function");
        let marker = MirBlockOp::from_operation(
            raw_function_entry(&function, &context)
                .deref(&context)
                .get_head()
                .expect("block marker"),
        );
        marker.set_attr_block_id(&context, MirBlockIdAttr::new(MirBlockId(1)));

        assert_eq!(
            service.run_checked(source.get_operation(), &mut context),
            Err(LoweringError::SourceVerificationFailed)
        );
        assert!(service.last_result.is_none());
    }

    #[test]
    fn validation_rejects_stale_source_and_live_mutated_output() {
        let (context, result) = populated_context("stale");
        let source = MirModuleOp::from_operation(result.source_root);
        source.set_attr_module_identity(&context, MirIdentityAttr::new("stale::changed"));
        assert_eq!(
            result.validate(&context),
            Err(PostconditionError::SourceEvidenceMismatch)
        );
        source.set_attr_module_identity(&context, MirIdentityAttr::new("stale"));

        let target = AlgorithmOp::from_operation(result.operations[0]);
        target.set_iteration_domain(
            &context,
            IterationDomainAttr::new(2).expect("different valid rank"),
        );
        assert_eq!(
            result.validate(&context),
            Err(PostconditionError::InvalidKernelOperation { index: 0 })
        );
    }
}
