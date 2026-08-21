#![forbid(unsafe_code)]

//! Feature-gated Pliron ownership shell for the `mir` dialect.
//!
//! This module is an in-memory representation only. Canonical MIR records,
//! wire encodings, and artifact identities remain owned by
//! [`fe2o3_mir_model`].

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
};

use ::pliron::{
    attribute::Attribute,
    basic_block::BasicBlock,
    builtin::{
        attributes::{StringAttr, TypeAttr},
        op_interfaces::{
            IsTerminatorInterface, IsolatedFromAboveInterface, NOpdsInterface, NRegionsInterface,
            NResultsInterface, NoTerminatorInterface, OneRegionInterface, RegionKind,
            RegionKindInterface, SingleBlockRegionInterface,
        },
        type_interfaces::FunctionTypeInterface,
        types::FunctionType,
    },
    combine::Parser,
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{op_interface_impl, pliron_attr, pliron_op, pliron_type},
    linked_list::{ContainsLinkedList, LinkedList},
    location::Located,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::{Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    result::Result as PlironResult,
    r#type::{Type, TypeHandle, Typed, TypedHandle},
    verify_err, verify_err_noloc,
};
use fe2o3_mir_model::{
    MAX_EXECUTABLE_BLOCK_PARAMETERS, MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_FUNCTIONS,
    MAX_EXECUTABLE_IDENTITY_BYTES, MAX_EXECUTABLE_TYPES, MirBlockId, MirTypeId,
    semantic_mir_v1::{
        HARD_MAX_BLOCKS_V1, HARD_MAX_FUNCTIONS_V1, HARD_MAX_STATEMENTS_V1,
        HARD_MAX_SWITCH_TARGETS_V1, SemanticBlockIdV1, SemanticEdgeRoleV1, SemanticFunctionIdV1,
    },
};
use fe2o3_pliron_owner_core::{
    ContextIdentity, ContextIdentityError, DialectRegistration, DialectRegistrationService,
    NameError, RegistrationHookError, ensure_context_identity, require_context_identity,
};

use crate::DIALECT;

/// Maximum ordered CFG targets retained on one imported rustc terminator.
pub const MAX_IMPORTED_MIR_SUCCESSORS: usize = 256;

/// Maximum canonical decimal bytes for all ordered `u32` CFG targets.
pub const MAX_IMPORTED_MIR_SUCCESSOR_TEXT_BYTES: usize =
    MAX_IMPORTED_MIR_SUCCESSORS * 10 + (MAX_IMPORTED_MIR_SUCCESSORS - 1);

/// Maximum canonical text bytes for ordered `role:block` production arcs.
pub const MAX_PRODUCTION_LOCATOR_ARC_TEXT_BYTES_V1: usize =
    MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1 * 14 - 1;

/// Semantic-v1 schema cap for the largest possible switch successor list.
pub const MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1: usize = HARD_MAX_SWITCH_TARGETS_V1 as usize + 1;

/// Independent implementation cap for constructing and verifying a Pliron tree.
pub const MAX_PRODUCTION_PLIRON_TREE_WORK_V1: u64 = 16_000_000;

/// Named middle-end resource charged before any production Pliron allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirProductionPlironResourceV1 {
    TreeWork,
}

/// Explicit resource limits for the production locator Pliron representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirProductionPlironLimitsV1 {
    max_tree_work: u64,
}

impl MirProductionPlironLimitsV1 {
    /// Creates a nonzero implementation budget bounded by the production cap.
    pub fn new(max_tree_work: u64) -> Result<Self, MirProductionLocatorErrorV1> {
        if max_tree_work == 0 || max_tree_work > MAX_PRODUCTION_PLIRON_TREE_WORK_V1 {
            return Err(MirProductionLocatorErrorV1::InvalidMiddleEndResourceLimit {
                resource: MirProductionPlironResourceV1::TreeWork,
                value: max_tree_work,
                hard_limit: MAX_PRODUCTION_PLIRON_TREE_WORK_V1,
            });
        }
        Ok(Self { max_tree_work })
    }

    /// Returns the maximum charged construction and verification work.
    pub const fn max_tree_work(self) -> u64 {
        self.max_tree_work
    }
}

impl Default for MirProductionPlironLimitsV1 {
    fn default() -> Self {
        Self {
            max_tree_work: MAX_PRODUCTION_PLIRON_TREE_WORK_V1,
        }
    }
}

/// A caller-asserted SHA-256 locator for one admitted semantic MIR module.
///
/// This value is inert. It identifies source evidence but does not authenticate,
/// admit, or grant authority over that evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirProductionSemanticSha256V1([u8; 32]);

impl MirProductionSemanticSha256V1 {
    /// Wraps caller-asserted SHA-256 bytes without authenticating them.
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One typed successor arc in exact semantic-MIR order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirProductionSuccessorArcV1 {
    role: SemanticEdgeRoleV1,
    target: SemanticBlockIdV1,
}

impl MirProductionSuccessorArcV1 {
    /// Creates one typed locator arc.
    pub const fn new(role: SemanticEdgeRoleV1, target: SemanticBlockIdV1) -> Self {
        Self { role, target }
    }

    /// Returns the semantic role of this arc.
    pub const fn role(self) -> SemanticEdgeRoleV1 {
        self.role
    }

    /// Returns the target block locator.
    pub const fn target(self) -> SemanticBlockIdV1 {
        self.target
    }
}

impl Hash for MirProductionSuccessorArcV1 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        production_edge_role_code(self.role).hash(state);
        self.target.index().hash(state);
    }
}

/// One statement position within a semantic MIR block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirProductionStatementLocatorV1 {
    ordinal: u32,
}

impl MirProductionStatementLocatorV1 {
    /// Creates a statement locator. Its enclosing block validates density.
    pub const fn new(ordinal: u32) -> Self {
        Self { ordinal }
    }

    /// Returns the zero-based statement ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

/// The unique terminator position and its exact ordered successor arcs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProductionTerminatorLocatorV1 {
    successors: Vec<MirProductionSuccessorArcV1>,
}

impl MirProductionTerminatorLocatorV1 {
    /// Creates a bounded, order-preserving terminator locator.
    pub fn try_new(
        successors: Vec<MirProductionSuccessorArcV1>,
    ) -> Result<Self, MirProductionLocatorErrorV1> {
        if successors.len() > MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManySuccessors {
                count: successors.len(),
                limit: MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1,
            });
        }
        Ok(Self { successors })
    }

    /// Returns successor arcs in exact semantic-MIR order.
    pub fn successors(&self) -> &[MirProductionSuccessorArcV1] {
        &self.successors
    }
}

/// Closed locator-only representation of one semantic MIR block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProductionBlockLocatorV1 {
    block_id: SemanticBlockIdV1,
    statements: Vec<MirProductionStatementLocatorV1>,
    terminator: MirProductionTerminatorLocatorV1,
}

impl MirProductionBlockLocatorV1 {
    /// Creates a block with dense statement ordinals and exactly one terminator.
    pub fn try_new(
        block_id: SemanticBlockIdV1,
        statements: Vec<MirProductionStatementLocatorV1>,
        terminator: MirProductionTerminatorLocatorV1,
    ) -> Result<Self, MirProductionLocatorErrorV1> {
        if u64::from(block_id.index()) >= HARD_MAX_BLOCKS_V1 {
            return Err(MirProductionLocatorErrorV1::BlockIdOutOfRange(
                block_id.index(),
            ));
        }
        if statements.len() as u64 > HARD_MAX_STATEMENTS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManyStatementsInBlock {
                count: statements.len(),
                limit: HARD_MAX_STATEMENTS_V1 as usize,
            });
        }
        for (expected, statement) in statements.iter().enumerate() {
            let expected = expected as u32;
            if statement.ordinal != expected {
                return Err(MirProductionLocatorErrorV1::NonCanonicalStatementOrdinal {
                    expected,
                    found: statement.ordinal,
                });
            }
        }
        Ok(Self {
            block_id,
            statements,
            terminator,
        })
    }

    /// Returns this block's typed semantic locator.
    pub const fn block_id(&self) -> SemanticBlockIdV1 {
        self.block_id
    }

    /// Returns statements in canonical ordinal order.
    pub fn statements(&self) -> &[MirProductionStatementLocatorV1] {
        &self.statements
    }

    /// Returns the unique block terminator locator.
    pub const fn terminator(&self) -> &MirProductionTerminatorLocatorV1 {
        &self.terminator
    }
}

/// Closed locator-only representation of one semantic MIR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProductionFunctionLocatorV1 {
    function_id: SemanticFunctionIdV1,
    entry_block_id: SemanticBlockIdV1,
    blocks: Vec<MirProductionBlockLocatorV1>,
}

impl MirProductionFunctionLocatorV1 {
    /// Creates a function in exact source block order with dense block locators.
    ///
    /// Pliron materialization may move the nonzero entry block to the physical
    /// head, but the block IDs retain and reconstruct this source order.
    pub fn try_new(
        function_id: SemanticFunctionIdV1,
        entry_block_id: SemanticBlockIdV1,
        blocks: Vec<MirProductionBlockLocatorV1>,
    ) -> Result<Self, MirProductionLocatorErrorV1> {
        if u64::from(function_id.index()) >= HARD_MAX_FUNCTIONS_V1 {
            return Err(MirProductionLocatorErrorV1::FunctionIdOutOfRange(
                function_id.index(),
            ));
        }
        if blocks.is_empty() {
            return Err(MirProductionLocatorErrorV1::EmptyFunction);
        }
        if blocks.len() as u64 > HARD_MAX_BLOCKS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManyBlocks {
                count: blocks.len(),
                limit: HARD_MAX_BLOCKS_V1 as usize,
            });
        }
        if entry_block_id.index() as usize >= blocks.len() {
            return Err(MirProductionLocatorErrorV1::EntryBlockMissing(
                entry_block_id.index(),
            ));
        }
        for (position, block) in blocks.iter().enumerate() {
            let expected = position as u32;
            if block.block_id.index() != expected {
                return Err(MirProductionLocatorErrorV1::NonCanonicalBlockId {
                    expected,
                    found: block.block_id.index(),
                });
            }
            for arc in block.terminator.successors() {
                if arc.target.index() as usize >= blocks.len() {
                    return Err(MirProductionLocatorErrorV1::DanglingSuccessor {
                        block: block.block_id.index(),
                        target: arc.target.index(),
                    });
                }
            }
        }
        Ok(Self {
            function_id,
            entry_block_id,
            blocks,
        })
    }

    /// Returns this function's typed semantic locator.
    pub const fn function_id(&self) -> SemanticFunctionIdV1 {
        self.function_id
    }

    /// Returns the exact entry block locator.
    pub const fn entry_block_id(&self) -> SemanticBlockIdV1 {
        self.entry_block_id
    }

    /// Returns blocks in exact source order (ascending semantic block ID).
    pub fn blocks(&self) -> &[MirProductionBlockLocatorV1] {
        &self.blocks
    }
}

/// Closed, pointer-independent production locator graph for one semantic module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProductionModuleLocatorV1 {
    semantic_sha256: MirProductionSemanticSha256V1,
    functions: Vec<MirProductionFunctionLocatorV1>,
}

impl MirProductionModuleLocatorV1 {
    /// Creates a module with dense function IDs and bounded global statements.
    pub fn try_new(
        semantic_sha256: MirProductionSemanticSha256V1,
        functions: Vec<MirProductionFunctionLocatorV1>,
    ) -> Result<Self, MirProductionLocatorErrorV1> {
        if functions.is_empty() {
            return Err(MirProductionLocatorErrorV1::EmptyModule);
        }
        if functions.len() as u64 > HARD_MAX_FUNCTIONS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManyFunctions {
                count: functions.len(),
                limit: HARD_MAX_FUNCTIONS_V1 as usize,
            });
        }
        let mut statement_count = 0_usize;
        let mut block_count = 0_usize;
        for (expected, function) in functions.iter().enumerate() {
            let expected = expected as u32;
            if function.function_id.index() != expected {
                return Err(MirProductionLocatorErrorV1::NonCanonicalFunctionId {
                    expected,
                    found: function.function_id.index(),
                });
            }
            block_count = block_count.checked_add(function.blocks.len()).ok_or(
                MirProductionLocatorErrorV1::TooManyBlocks {
                    count: usize::MAX,
                    limit: HARD_MAX_BLOCKS_V1 as usize,
                },
            )?;
            for block in &function.blocks {
                statement_count = statement_count.checked_add(block.statements.len()).ok_or(
                    MirProductionLocatorErrorV1::TooManyStatements {
                        count: usize::MAX,
                        limit: HARD_MAX_STATEMENTS_V1 as usize,
                    },
                )?;
            }
        }
        if block_count as u64 > HARD_MAX_BLOCKS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManyBlocks {
                count: block_count,
                limit: HARD_MAX_BLOCKS_V1 as usize,
            });
        }
        if statement_count as u64 > HARD_MAX_STATEMENTS_V1 {
            return Err(MirProductionLocatorErrorV1::TooManyStatements {
                count: statement_count,
                limit: HARD_MAX_STATEMENTS_V1 as usize,
            });
        }
        Ok(Self {
            semantic_sha256,
            functions,
        })
    }

    /// Returns the exact caller-asserted semantic module digest.
    pub const fn semantic_sha256(&self) -> MirProductionSemanticSha256V1 {
        self.semantic_sha256
    }

    /// Returns functions in canonical function-ID order.
    pub fn functions(&self) -> &[MirProductionFunctionLocatorV1] {
        &self.functions
    }
}

const fn canonical_production_block_id(position: usize, entry: u32) -> u32 {
    if position == 0 {
        entry
    } else {
        let ascending = (position - 1) as u32;
        if ascending < entry {
            ascending
        } else {
            ascending + 1
        }
    }
}

/// Rejections from the closed production locator API and its owner-bound handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirProductionLocatorErrorV1 {
    EmptyModule,
    EmptyFunction,
    TooManyFunctions {
        count: usize,
        limit: usize,
    },
    TooManyBlocks {
        count: usize,
        limit: usize,
    },
    TooManyStatements {
        count: usize,
        limit: usize,
    },
    TooManyStatementsInBlock {
        count: usize,
        limit: usize,
    },
    TooManySuccessors {
        count: usize,
        limit: usize,
    },
    FunctionIdOutOfRange(u32),
    BlockIdOutOfRange(u32),
    NonCanonicalFunctionId {
        expected: u32,
        found: u32,
    },
    NonCanonicalBlockId {
        expected: u32,
        found: u32,
    },
    NonCanonicalStatementOrdinal {
        expected: u32,
        found: u32,
    },
    EntryBlockMissing(u32),
    DanglingSuccessor {
        block: u32,
        target: u32,
    },
    InvalidMiddleEndResourceLimit {
        resource: MirProductionPlironResourceV1,
        value: u64,
        hard_limit: u64,
    },
    MiddleEndResourceLimitExceeded {
        resource: MirProductionPlironResourceV1,
        actual: u64,
        limit: u64,
    },
    ContextIdentity(ContextIdentityError),
    ForeignContext,
    StaleHandle,
    WrongKind,
    VerificationFailed,
    MalformedSnapshot,
    UpstreamPanicked,
}

impl fmt::Display for MirProductionLocatorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyModule => formatter.write_str("production locator module is empty"),
            Self::EmptyFunction => formatter.write_str("production locator function is empty"),
            Self::TooManyFunctions { count, limit } => write!(
                formatter,
                "production locator has {count} functions, exceeding {limit}"
            ),
            Self::TooManyBlocks { count, limit } => write!(
                formatter,
                "production locator has {count} blocks, exceeding {limit}"
            ),
            Self::TooManyStatements { count, limit } => write!(
                formatter,
                "production locator has {count} statements, exceeding {limit}"
            ),
            Self::TooManyStatementsInBlock { count, limit } => write!(
                formatter,
                "production locator block has {count} statements, exceeding {limit}"
            ),
            Self::TooManySuccessors { count, limit } => write!(
                formatter,
                "production locator terminator has {count} successors, exceeding {limit}"
            ),
            Self::FunctionIdOutOfRange(id) => {
                write!(
                    formatter,
                    "production function locator {id} is out of range"
                )
            }
            Self::BlockIdOutOfRange(id) => {
                write!(formatter, "production block locator {id} is out of range")
            }
            Self::NonCanonicalFunctionId { expected, found } => write!(
                formatter,
                "production function locator {found} is non-canonical; expected {expected}"
            ),
            Self::NonCanonicalBlockId { expected, found } => write!(
                formatter,
                "production block locator {found} is non-canonical; expected {expected}"
            ),
            Self::NonCanonicalStatementOrdinal { expected, found } => write!(
                formatter,
                "production statement ordinal {found} is non-canonical; expected {expected}"
            ),
            Self::EntryBlockMissing(id) => {
                write!(formatter, "production entry block locator {id} is missing")
            }
            Self::DanglingSuccessor { block, target } => write!(
                formatter,
                "production block {block} has dangling successor locator {target}"
            ),
            Self::InvalidMiddleEndResourceLimit {
                resource,
                value,
                hard_limit,
            } => write!(
                formatter,
                "invalid production Pliron {resource:?} limit {value}; expected 1..={hard_limit}"
            ),
            Self::MiddleEndResourceLimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "production Pliron {resource:?} work {actual} exceeds middle-end limit {limit}"
            ),
            Self::ContextIdentity(_) => {
                formatter.write_str("production locator context identity validation failed")
            }
            Self::ForeignContext => {
                formatter.write_str("production locator handle belongs to another context")
            }
            Self::StaleHandle => formatter.write_str("production locator handle is stale"),
            Self::WrongKind => formatter.write_str("production locator handle has the wrong kind"),
            Self::VerificationFailed => {
                formatter.write_str("production locator verification failed")
            }
            Self::MalformedSnapshot => {
                formatter.write_str("verified production locator structure changed")
            }
            Self::UpstreamPanicked => {
                formatter.write_str("production locator operation was rejected after a panic")
            }
        }
    }
}

impl Error for MirProductionLocatorErrorV1 {}

/// Hard limit categories for the in-memory MIR shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDialectLimitKind {
    Functions,
    BlocksPerFunction,
    IdentityBytes,
}

/// Construction limits persisted on every module and function operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirDialectLimits {
    max_functions: usize,
    max_blocks_per_function: usize,
    max_identity_bytes: usize,
}

impl MirDialectLimits {
    /// Creates non-zero limits bounded by the frozen MIR model limits.
    pub fn new(
        max_functions: usize,
        max_blocks_per_function: usize,
        max_identity_bytes: usize,
    ) -> Result<Self, MirDialectBuildError> {
        check_limit(
            MirDialectLimitKind::Functions,
            max_functions,
            MAX_EXECUTABLE_FUNCTIONS,
        )?;
        check_limit(
            MirDialectLimitKind::BlocksPerFunction,
            max_blocks_per_function,
            MAX_EXECUTABLE_BLOCKS,
        )?;
        check_limit(
            MirDialectLimitKind::IdentityBytes,
            max_identity_bytes,
            MAX_EXECUTABLE_IDENTITY_BYTES,
        )?;
        Ok(Self {
            max_functions,
            max_blocks_per_function,
            max_identity_bytes,
        })
    }

    pub const fn max_functions(self) -> usize {
        self.max_functions
    }

    pub const fn max_blocks_per_function(self) -> usize {
        self.max_blocks_per_function
    }

    pub const fn max_identity_bytes(self) -> usize {
        self.max_identity_bytes
    }
}

impl Default for MirDialectLimits {
    fn default() -> Self {
        Self {
            max_functions: MAX_EXECUTABLE_FUNCTIONS,
            max_blocks_per_function: MAX_EXECUTABLE_BLOCKS,
            max_identity_bytes: MAX_EXECUTABLE_IDENTITY_BYTES,
        }
    }
}

fn check_limit(
    kind: MirDialectLimitKind,
    value: usize,
    hard_limit: usize,
) -> Result<(), MirDialectBuildError> {
    if value == 0 || value > hard_limit {
        return Err(MirDialectBuildError::InvalidLimit {
            kind,
            value,
            hard_limit,
        });
    }
    Ok(())
}

fn ensure_mir_context_owner(
    context: &mut Context,
) -> Result<ContextIdentity, MirDialectBuildError> {
    match catch_unwind(AssertUnwindSafe(|| ensure_context_identity(context))) {
        Ok(Ok(owner)) => Ok(owner),
        Ok(Err(error)) => Err(MirDialectBuildError::ContextIdentity(error)),
        Err(_) => Err(MirDialectBuildError::UpstreamPanicked),
    }
}

/// Errors from the bounded construction surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirDialectBuildError {
    InvalidLimit {
        kind: MirDialectLimitKind,
        value: usize,
        hard_limit: usize,
    },
    EmptyIdentity,
    IdentityTooLong {
        bytes: usize,
        limit: usize,
    },
    TooManyArguments {
        count: usize,
        limit: usize,
    },
    TypeIdOutOfRange(MirTypeId),
    InvalidSemanticIdentity,
    InvalidSemanticKind(u16),
    InvalidSemanticSpan,
    TooManySemanticSuccessors {
        count: usize,
        limit: usize,
    },
    SemanticSuccessorTextTooLong {
        bytes: usize,
        limit: usize,
    },
    InvalidSemanticSuccessorTarget,
    SemanticSuccessorArityUnsupported,
    InvalidSemanticOperationOrder,
    FunctionLimitExceeded {
        limit: usize,
    },
    BlockLimitExceeded {
        limit: usize,
    },
    ContextIdentity(ContextIdentityError),
    UpstreamPanicked,
    MalformedOperation(&'static str),
}

impl fmt::Display for MirDialectBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimit {
                kind,
                value,
                hard_limit,
            } => write!(
                formatter,
                "invalid {kind:?} limit {value}; expected 1..={hard_limit}"
            ),
            Self::EmptyIdentity => formatter.write_str("MIR identity must not be empty"),
            Self::IdentityTooLong { bytes, limit } => write!(
                formatter,
                "MIR identity has {bytes} bytes, exceeding the limit {limit}"
            ),
            Self::TooManyArguments { count, limit } => write!(
                formatter,
                "MIR function has {count} arguments, exceeding the limit {limit}"
            ),
            Self::TypeIdOutOfRange(id) => {
                write!(formatter, "MIR type id {} is outside the model table", id.0)
            }
            Self::InvalidSemanticIdentity => {
                formatter.write_str("MIR semantic identity must be nonzero")
            }
            Self::InvalidSemanticKind(kind) => {
                write!(formatter, "MIR semantic operation kind {kind} is invalid")
            }
            Self::InvalidSemanticSpan => formatter.write_str("MIR semantic source span is invalid"),
            Self::TooManySemanticSuccessors { count, limit } => write!(
                formatter,
                "MIR semantic terminator has {count} successors, exceeding {limit}"
            ),
            Self::SemanticSuccessorTextTooLong { bytes, limit } => write!(
                formatter,
                "MIR semantic successor text has {bytes} bytes, exceeding {limit}"
            ),
            Self::InvalidSemanticSuccessorTarget => {
                formatter.write_str("MIR semantic successor target is not in the same function")
            }
            Self::SemanticSuccessorArityUnsupported => formatter.write_str(
                "MIR semantic successor target has block arguments but no edge operands",
            ),
            Self::InvalidSemanticOperationOrder => {
                formatter.write_str("MIR semantic operation order is invalid")
            }
            Self::FunctionLimitExceeded { limit } => {
                write!(formatter, "MIR module function limit {limit} is exhausted")
            }
            Self::BlockLimitExceeded { limit } => {
                write!(formatter, "MIR function block limit {limit} is exhausted")
            }
            Self::ContextIdentity(_) => {
                formatter.write_str("MIR context identity validation failed")
            }
            Self::UpstreamPanicked => {
                formatter.write_str("MIR construction was rejected after an upstream panic")
            }
            Self::MalformedOperation(reason) => {
                write!(formatter, "malformed MIR operation: {reason}")
            }
        }
    }
}

impl Error for MirDialectBuildError {}

/// An opaque CFG block capability bound to one Pliron context.
///
/// The upstream pointer remains private and can only be used through methods
/// that authenticate its context owner and liveness first.
#[derive(Clone, Eq, PartialEq)]
pub struct MirBlockHandle {
    owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    parent: Ptr<Operation>,
    role: MirBlockRole,
}

impl fmt::Debug for MirBlockHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MirBlockHandle")
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

/// Bounded failures from an owner-aware MIR block handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBlockHandleError {
    ContextIdentity(ContextIdentityError),
    ForeignContext,
    StaleHandle,
    TransplantedHandle,
    WrongKind,
    MalformedBlock,
    VerificationFailed,
    UpstreamPanicked,
}

impl fmt::Display for MirBlockHandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextIdentity(_) => {
                formatter.write_str("MIR block context identity validation failed")
            }
            Self::ForeignContext => {
                formatter.write_str("MIR block handle belongs to another context")
            }
            Self::StaleHandle => formatter.write_str("MIR block handle is stale"),
            Self::TransplantedHandle => {
                formatter.write_str("MIR block handle was transplanted to another parent")
            }
            Self::WrongKind => formatter.write_str("MIR block handle has the wrong block kind"),
            Self::MalformedBlock => formatter.write_str("MIR block marker is malformed"),
            Self::VerificationFailed => formatter.write_str("MIR block verification failed"),
            Self::UpstreamPanicked => {
                formatter.write_str("MIR block access was rejected after an upstream panic")
            }
        }
    }
}

impl Error for MirBlockHandleError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirBlockRole {
    ModuleBody,
    FunctionEntry,
    FunctionBlock,
}

/// An opaque module-body capability bound to one Pliron context.
///
/// The handle identifies the single block containing a module's functions
/// without exposing Pliron's raw block pointer.
#[derive(Clone, Eq, PartialEq)]
pub struct MirModuleBodyHandle {
    owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    parent: Ptr<Operation>,
}

/// Pointer-independent source coordinates retained from rustc.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirSemanticSourceSpan {
    file_identity: [u64; 4],
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

/// Exact source provenance for one rustc MIR operation.
///
/// `expansion` is rustc's operation span as stored in optimized MIR. `call_site`
/// is the recursively resolved source call-site span used for user diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirSemanticSpanProvenance {
    expansion: MirSemanticSourceSpan,
    call_site: MirSemanticSourceSpan,
}

impl MirSemanticSpanProvenance {
    pub fn new(
        expansion: MirSemanticSourceSpan,
        call_site: MirSemanticSourceSpan,
    ) -> Result<Self, MirDialectBuildError> {
        expansion.validate()?;
        call_site.validate()?;
        Ok(Self {
            expansion,
            call_site,
        })
    }

    pub const fn expansion(self) -> MirSemanticSourceSpan {
        self.expansion
    }

    pub const fn call_site(self) -> MirSemanticSourceSpan {
        self.call_site
    }

    fn validate(self) -> Result<(), MirDialectBuildError> {
        self.expansion.validate()?;
        self.call_site.validate()
    }
}

impl MirSemanticSourceSpan {
    /// Creates one exact, non-empty source span.
    pub fn new(
        file_identity: [u64; 4],
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Result<Self, MirDialectBuildError> {
        let span = Self {
            file_identity,
            start_line,
            start_column,
            end_line,
            end_column,
        };
        span.validate()?;
        Ok(span)
    }

    /// Returns the stable source-file identity as four little-endian words.
    pub const fn file_identity(self) -> [u64; 4] {
        self.file_identity
    }

    /// Returns `(start_line, start_column, end_line, end_column)`.
    pub const fn coordinates(self) -> [u32; 4] {
        [
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column,
        ]
    }

    fn validate(self) -> Result<(), MirDialectBuildError> {
        if self.file_identity == [0; 4]
            || self.start_line == 0
            || self.start_column == 0
            || self.end_line == 0
            || self.end_column == 0
            || (self.start_line, self.start_column) > (self.end_line, self.end_column)
        {
            return Err(MirDialectBuildError::InvalidSemanticSpan);
        }
        Ok(())
    }
}

/// Typed classification of an exact rustc MIR operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum MirSemanticOperationKind {
    StatementAssign = 1,
    StatementFakeRead = 2,
    StatementSetDiscriminant = 3,
    StatementDeinit = 4,
    StatementStorageLive = 5,
    StatementStorageDead = 6,
    StatementRetag = 7,
    StatementPlaceMention = 8,
    StatementAscribeUserType = 9,
    StatementCoverage = 10,
    StatementIntrinsic = 11,
    StatementConstEvalCounter = 12,
    StatementNop = 13,
    StatementBackwardIncompatibleDropHint = 14,
    TerminatorGoto = 256,
    TerminatorSwitchInt = 257,
    TerminatorReturn = 258,
    TerminatorUnreachable = 259,
    TerminatorDrop = 260,
    TerminatorCall = 261,
    TerminatorAssert = 262,
    TerminatorUnwindResume = 263,
    TerminatorYield = 264,
    TerminatorCoroutineDrop = 265,
    TerminatorFalseEdge = 266,
    TerminatorInlineAsm = 267,
    TerminatorTailCall = 268,
    TerminatorUnwindTerminate = 269,
    TerminatorFalseUnwind = 270,
}

impl MirSemanticOperationKind {
    const fn from_raw(value: u16) -> Option<Self> {
        Some(match value {
            1 => Self::StatementAssign,
            2 => Self::StatementFakeRead,
            3 => Self::StatementSetDiscriminant,
            4 => Self::StatementDeinit,
            5 => Self::StatementStorageLive,
            6 => Self::StatementStorageDead,
            7 => Self::StatementRetag,
            8 => Self::StatementPlaceMention,
            9 => Self::StatementAscribeUserType,
            10 => Self::StatementCoverage,
            11 => Self::StatementIntrinsic,
            12 => Self::StatementConstEvalCounter,
            13 => Self::StatementNop,
            14 => Self::StatementBackwardIncompatibleDropHint,
            256 => Self::TerminatorGoto,
            257 => Self::TerminatorSwitchInt,
            258 => Self::TerminatorReturn,
            259 => Self::TerminatorUnreachable,
            260 => Self::TerminatorDrop,
            261 => Self::TerminatorCall,
            262 => Self::TerminatorAssert,
            263 => Self::TerminatorUnwindResume,
            264 => Self::TerminatorYield,
            265 => Self::TerminatorCoroutineDrop,
            266 => Self::TerminatorFalseEdge,
            267 => Self::TerminatorInlineAsm,
            268 => Self::TerminatorTailCall,
            269 => Self::TerminatorUnwindTerminate,
            270 => Self::TerminatorFalseUnwind,
            _ => return None,
        })
    }

    /// Returns whether this classification is a terminator.
    pub const fn is_terminator(self) -> bool {
        self as u16 >= Self::TerminatorGoto as u16
    }
}

/// Exact inert evidence extracted from a typed MIR operation.
///
/// Callers cannot forge semantic evidence because all fields are private:
///
/// ```compile_fail
/// use dialect_mir::pliron::{MirSemanticOperationKind, MirSemanticOperationSnapshot};
///
/// let forged = MirSemanticOperationSnapshot {
///     ordinal: 0,
///     kind: MirSemanticOperationKind::StatementNop,
///     identity: [1, 2, 3, 4],
///     span: panic!(),
///     successors: Vec::new(),
/// };
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSemanticOperationSnapshot {
    ordinal: u32,
    kind: MirSemanticOperationKind,
    identity: [u64; 4],
    provenance: MirSemanticSpanProvenance,
    successors: Vec<u32>,
}

impl MirSemanticOperationSnapshot {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn kind(&self) -> MirSemanticOperationKind {
        self.kind
    }

    pub const fn identity(&self) -> [u64; 4] {
        self.identity
    }

    pub const fn provenance(&self) -> MirSemanticSpanProvenance {
        self.provenance
    }

    pub const fn expansion_span(&self) -> MirSemanticSourceSpan {
        self.provenance.expansion()
    }

    pub const fn call_site_span(&self) -> MirSemanticSourceSpan {
        self.provenance.call_site()
    }

    pub fn successors(&self) -> &[u32] {
        &self.successors
    }
}

/// Pointer-independent operation evidence from one verified MIR CFG block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirSnapshotOperation {
    /// Canonical block marker carrying its stable MIR block identifier.
    BlockMarker(MirBlockId),
    /// One exact optimized-rustc statement observation.
    SemanticStatement(MirSemanticOperationSnapshot),
    /// One exact optimized-rustc terminator observation not yet admitted by
    /// the target-neutral lowering.
    SemanticTerminator(MirSemanticOperationSnapshot),
    /// Place-based MIR return terminator.
    Return,
}

/// Pointer-independent semantic snapshot of one verified MIR CFG block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlockSnapshot {
    block_id: MirBlockId,
    operations: Vec<MirSnapshotOperation>,
}

impl MirBlockSnapshot {
    /// Returns the verified stable MIR block identifier.
    pub const fn block_id(&self) -> MirBlockId {
        self.block_id
    }

    /// Returns operations in exact block order.
    pub fn operations(&self) -> &[MirSnapshotOperation] {
        &self.operations
    }
}

/// Pointer-independent semantic snapshot of one verified MIR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionSnapshot {
    identity: String,
    argument_type_ids: Vec<MirTypeId>,
    blocks: Vec<MirBlockSnapshot>,
}

impl MirFunctionSnapshot {
    /// Returns the exact verified function identity.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns verified MIR type references in argument order.
    pub fn argument_type_ids(&self) -> &[MirTypeId] {
        &self.argument_type_ids
    }

    /// Returns verified blocks in canonical CFG order.
    pub fn blocks(&self) -> &[MirBlockSnapshot] {
        &self.blocks
    }
}

/// Bounded failures while taking a semantic snapshot through an owned body handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirModuleSnapshotError {
    /// The owner-aware module-body capability was rejected.
    Handle(MirBlockHandleError),
    /// A verified module no longer has the required MIR shape.
    MalformedModule,
    /// A function argument is not a MIR type-table reference.
    UnsupportedArgumentType {
        /// Function ordinal in module order.
        function: usize,
        /// Argument ordinal in signature order.
        argument: usize,
    },
}

impl fmt::Display for MirModuleSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handle(error) => write!(formatter, "MIR module body access failed: {error}"),
            Self::MalformedModule => formatter.write_str("verified MIR module shape changed"),
            Self::UnsupportedArgumentType { function, argument } => write!(
                formatter,
                "MIR function {function} argument {argument} is not a MIR type reference"
            ),
        }
    }
}

impl Error for MirModuleSnapshotError {}

impl fmt::Debug for MirModuleBodyHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MirModuleBodyHandle")
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl MirModuleBodyHandle {
    /// Returns the number of live functions after owner and liveness checks.
    pub fn function_count(&self, context: &Context) -> Result<usize, MirBlockHandleError> {
        with_owned_block(
            self.owner,
            self.pointer,
            self.parent,
            MirBlockRole::ModuleBody,
            context,
            |body, context| body.deref(context).iter(context).count(),
        )
    }

    /// Takes a pointer-independent snapshot after authenticating owner and liveness.
    pub fn semantic_functions(
        &self,
        context: &Context,
    ) -> Result<Vec<MirFunctionSnapshot>, MirModuleSnapshotError> {
        with_owned_block(
            self.owner,
            self.pointer,
            self.parent,
            MirBlockRole::ModuleBody,
            context,
            |body, context| {
                let mut functions = Vec::new();
                for (function_index, operation) in body.deref(context).iter(context).enumerate() {
                    let function = Operation::get_op::<MirFunctionOp>(operation, context)
                        .ok_or(MirModuleSnapshotError::MalformedModule)?;
                    let identity = function
                        .get_attr_function_identity(context)
                        .ok_or(MirModuleSnapshotError::MalformedModule)?
                        .as_str()
                        .to_owned();
                    let signature = function
                        .signature(context)
                        .ok_or(MirModuleSnapshotError::MalformedModule)?;
                    let signature_ref = signature.deref(context);
                    let signature = signature_ref
                        .downcast_ref::<FunctionType>()
                        .ok_or(MirModuleSnapshotError::MalformedModule)?;
                    let argument_types = signature.arg_types().to_vec();
                    drop(signature_ref);

                    let mut argument_type_ids = Vec::with_capacity(argument_types.len());
                    for (argument_index, argument_type) in argument_types.into_iter().enumerate() {
                        let argument_type = argument_type.deref(context);
                        let Some(argument_type) = argument_type.downcast_ref::<MirTypeRef>() else {
                            return Err(MirModuleSnapshotError::UnsupportedArgumentType {
                                function: function_index,
                                argument: argument_index,
                            });
                        };
                        argument_type_ids.push(argument_type.value());
                    }

                    let function_ref = operation.deref(context);
                    let region = function_ref.get_region(0);
                    drop(function_ref);
                    let mut blocks = Vec::new();
                    for block in region.deref(context).iter(context) {
                        let mut operations = Vec::new();
                        let mut block_id = None;
                        for block_operation in block.deref(context).iter(context) {
                            if let Some(marker) =
                                Operation::get_op::<MirBlockOp>(block_operation, context)
                            {
                                let id = marker
                                    .block_id(context)
                                    .ok_or(MirModuleSnapshotError::MalformedModule)?;
                                block_id = Some(id);
                                operations.push(MirSnapshotOperation::BlockMarker(id));
                            } else if let Some(statement) = Operation::get_op::<
                                MirSemanticStatementOp,
                            >(
                                block_operation, context
                            ) {
                                operations.push(MirSnapshotOperation::SemanticStatement(
                                    statement
                                        .semantic_snapshot(context)
                                        .ok_or(MirModuleSnapshotError::MalformedModule)?,
                                ));
                            } else if let Some(terminator) = Operation::get_op::<
                                MirSemanticTerminatorOp,
                            >(
                                block_operation, context
                            ) {
                                operations.push(MirSnapshotOperation::SemanticTerminator(
                                    terminator
                                        .semantic_snapshot(context)
                                        .ok_or(MirModuleSnapshotError::MalformedModule)?,
                                ));
                            } else if Operation::is_op::<MirReturnOp>(block_operation, context) {
                                operations.push(MirSnapshotOperation::Return);
                            } else {
                                return Err(MirModuleSnapshotError::MalformedModule);
                            }
                        }
                        blocks.push(MirBlockSnapshot {
                            block_id: block_id.ok_or(MirModuleSnapshotError::MalformedModule)?,
                            operations,
                        });
                    }
                    functions.push(MirFunctionSnapshot {
                        identity,
                        argument_type_ids,
                        blocks,
                    });
                }
                Ok(functions)
            },
        )
        .map_err(MirModuleSnapshotError::Handle)?
    }

    /// Verifies the live module containing this body.
    pub fn verify(&self, context: &Context) -> Result<(), MirBlockHandleError> {
        verify_owned_block_parent(
            self.owner,
            self.pointer,
            self.parent,
            MirBlockRole::ModuleBody,
            context,
        )
    }

    /// Erases this module body after owner, liveness, and role checks.
    pub fn erase(&self, context: &mut Context) -> Result<(), MirBlockHandleError> {
        erase_owned_block(
            self.owner,
            self.pointer,
            self.parent,
            MirBlockRole::ModuleBody,
            context,
        )
    }

    /// This in-memory capability grants no publication or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl MirBlockHandle {
    /// Returns the canonical MIR block identity after owner and liveness checks.
    pub fn block_id(&self, context: &Context) -> Result<MirBlockId, MirBlockHandleError> {
        self.with_block(context, |block, context| {
            block
                .deref(context)
                .get_head()
                .and_then(|marker| Operation::get_op::<MirBlockOp>(marker, context))
                .and_then(|marker| marker.block_id(context))
        })?
        .ok_or(MirBlockHandleError::MalformedBlock)
    }

    /// Inserts one exact statement immediately before this block's terminator.
    pub fn append_semantic_statement(
        &self,
        context: &mut Context,
        ordinal: u32,
        kind: MirSemanticOperationKind,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
    ) -> Result<(), MirDialectBuildError> {
        self.authenticate(context)
            .map_err(|_| MirDialectBuildError::MalformedOperation("invalid block handle"))?;
        catch_unwind(AssertUnwindSafe(|| {
            let tail = self.pointer.deref(context).get_tail().ok_or(
                MirDialectBuildError::MalformedOperation("block terminator is missing"),
            )?;
            if !Operation::is_op::<MirReturnOp>(tail, context)
                && !Operation::is_op::<MirSemanticTerminatorOp>(tail, context)
            {
                return Err(MirDialectBuildError::MalformedOperation(
                    "block tail is not a MIR terminator",
                ));
            }
            let statement =
                MirSemanticStatementOp::try_new(context, ordinal, kind, identity, provenance)?;
            statement.get_operation().insert_before(context, tail);
            Ok(())
        }))
        .unwrap_or(Err(MirDialectBuildError::UpstreamPanicked))
    }

    /// Replaces the synthetic return with one exact optimized-rustc terminator.
    pub fn replace_with_semantic_terminator(
        &self,
        context: &mut Context,
        ordinal: u32,
        kind: MirSemanticOperationKind,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
        successors: &[MirBlockHandle],
    ) -> Result<(), MirDialectBuildError> {
        self.authenticate(context)
            .map_err(|_| MirDialectBuildError::MalformedOperation("invalid block handle"))?;
        catch_unwind(AssertUnwindSafe(|| {
            if successors.len() > MAX_IMPORTED_MIR_SUCCESSORS {
                return Err(MirDialectBuildError::TooManySemanticSuccessors {
                    count: successors.len(),
                    limit: MAX_IMPORTED_MIR_SUCCESSORS,
                });
            }
            let mut target_ids = Vec::with_capacity(successors.len());
            let mut target_blocks = Vec::with_capacity(successors.len());
            for target in successors {
                target
                    .authenticate(context)
                    .map_err(|_| MirDialectBuildError::InvalidSemanticSuccessorTarget)?;
                if target.parent != self.parent
                    || !matches!(
                        target.role,
                        MirBlockRole::FunctionEntry | MirBlockRole::FunctionBlock
                    )
                {
                    return Err(MirDialectBuildError::InvalidSemanticSuccessorTarget);
                }
                if target.pointer.deref(context).get_num_arguments() != 0 {
                    return Err(MirDialectBuildError::SemanticSuccessorArityUnsupported);
                }
                target_ids.push(
                    target
                        .block_id(context)
                        .map_err(|_| MirDialectBuildError::InvalidSemanticSuccessorTarget)?
                        .0,
                );
                target_blocks.push(target.pointer);
            }
            let tail = self.pointer.deref(context).get_tail().ok_or(
                MirDialectBuildError::MalformedOperation("block terminator is missing"),
            )?;
            if !Operation::is_op::<MirReturnOp>(tail, context) {
                return Err(MirDialectBuildError::MalformedOperation(
                    "synthetic MIR return is missing",
                ));
            }
            let terminator = MirSemanticTerminatorOp::try_new(
                context,
                ordinal,
                kind,
                identity,
                provenance,
                &target_ids,
                target_blocks,
            )?;
            terminator.get_operation().insert_before(context, tail);
            Operation::erase(tail, context);
            Ok(())
        }))
        .unwrap_or(Err(MirDialectBuildError::UpstreamPanicked))
    }

    /// Verifies the live parent function containing this block.
    pub fn verify(&self, context: &Context) -> Result<(), MirBlockHandleError> {
        let verified = self.with_block(context, |block, context| {
            block
                .deref(context)
                .get_parent_op(context)
                .is_some_and(|parent| verify_operation(parent, context).is_ok())
        })?;
        if verified {
            Ok(())
        } else {
            Err(MirBlockHandleError::VerificationFailed)
        }
    }

    /// Erases this block after owner and liveness checks.
    pub fn erase(&self, context: &mut Context) -> Result<(), MirBlockHandleError> {
        erase_owned_block(self.owner, self.pointer, self.parent, self.role, context)
    }

    /// This in-memory capability grants no publication or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    fn authenticate(&self, context: &Context) -> Result<(), MirBlockHandleError> {
        authenticate_owned_block(self.owner, self.pointer, self.parent, self.role, context)
    }

    fn with_block<T>(
        &self,
        context: &Context,
        action: impl FnOnce(Ptr<BasicBlock>, &Context) -> T,
    ) -> Result<T, MirBlockHandleError> {
        with_owned_block(
            self.owner,
            self.pointer,
            self.parent,
            self.role,
            context,
            action,
        )
    }
}

fn authenticate_owned_block(
    expected_owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    expected_parent: Ptr<Operation>,
    role: MirBlockRole,
    context: &Context,
) -> Result<(), MirBlockHandleError> {
    let owner = require_context_identity(context).map_err(MirBlockHandleError::ContextIdentity)?;
    if owner != expected_owner {
        return Err(MirBlockHandleError::ForeignContext);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let block = pointer
            .try_deref(context)
            .map_err(|_| MirBlockHandleError::StaleHandle)?;
        let Some(parent_region) = block.get_parent_region() else {
            return Err(MirBlockHandleError::WrongKind);
        };
        drop(block);

        let region = parent_region.deref(context);
        let parent = region.get_parent_op();
        let at_head = region.get_head() == Some(pointer);
        let is_only_block = at_head && region.get_tail() == Some(pointer);
        drop(region);

        let parent_operation = parent.deref(context);
        let owns_region =
            parent_operation.num_regions() == 1 && parent_operation.get_region(0) == parent_region;
        drop(parent_operation);
        let (has_expected_parent_kind, has_expected_position) = match role {
            MirBlockRole::ModuleBody => (
                Operation::is_op::<MirModuleOp>(parent, context),
                is_only_block,
            ),
            MirBlockRole::FunctionEntry => {
                (Operation::is_op::<MirFunctionOp>(parent, context), at_head)
            }
            MirBlockRole::FunctionBlock => {
                (Operation::is_op::<MirFunctionOp>(parent, context), true)
            }
        };
        if !owns_region || !has_expected_parent_kind {
            return Err(MirBlockHandleError::WrongKind);
        }
        if parent != expected_parent {
            return Err(MirBlockHandleError::TransplantedHandle);
        }
        if !has_expected_position {
            return Err(MirBlockHandleError::WrongKind);
        }
        Ok(())
    })) {
        Ok(result) => result,
        Err(_) => Err(MirBlockHandleError::UpstreamPanicked),
    }
}

fn with_owned_block<T>(
    owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    parent: Ptr<Operation>,
    role: MirBlockRole,
    context: &Context,
    action: impl FnOnce(Ptr<BasicBlock>, &Context) -> T,
) -> Result<T, MirBlockHandleError> {
    authenticate_owned_block(owner, pointer, parent, role, context)?;
    catch_unwind(AssertUnwindSafe(|| action(pointer, context)))
        .map_err(|_| MirBlockHandleError::UpstreamPanicked)
}

fn verify_owned_block_parent(
    owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    parent: Ptr<Operation>,
    role: MirBlockRole,
    context: &Context,
) -> Result<(), MirBlockHandleError> {
    let verified = with_owned_block(owner, pointer, parent, role, context, |block, context| {
        block
            .deref(context)
            .get_parent_op(context)
            .is_some_and(|parent| verify_operation(parent, context).is_ok())
    })?;
    if verified {
        Ok(())
    } else {
        Err(MirBlockHandleError::VerificationFailed)
    }
}

fn erase_owned_block(
    owner: ContextIdentity,
    pointer: Ptr<BasicBlock>,
    parent: Ptr<Operation>,
    role: MirBlockRole,
    context: &mut Context,
) -> Result<(), MirBlockHandleError> {
    authenticate_owned_block(owner, pointer, parent, role, context)?;
    catch_unwind(AssertUnwindSafe(|| BasicBlock::erase(pointer, context)))
        .map_err(|_| MirBlockHandleError::UpstreamPanicked)
}

/// Verifier failures specific to the MIR dialect shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirDialectVerifyError {
    MissingAttribute(&'static str),
    InvalidIdentity,
    InvalidLimits,
    InvalidTypeId(u32),
    InvalidBlockId(u32),
    InvalidSemanticIdentity,
    InvalidSemanticKind(u16),
    InvalidSemanticSpan,
    InvalidSemanticSuccessors,
    SemanticSuccessorTargetMissing(u32),
    SemanticSuccessorOrderMismatch,
    SemanticSuccessorArityUnsupported(u32),
    InvalidSemanticOperationOrder,
    SemanticStatementOutsideFunction,
    SemanticTerminatorOutsideFunction,
    SemanticStatementKindMismatch,
    SemanticTerminatorKindMismatch,
    UnexpectedModuleChild,
    DuplicateFunctionIdentity(String),
    FunctionLimitExceeded,
    IdentityLimitExceeded,
    InconsistentLimits,
    FunctionOutsideModule,
    InvalidFunctionType,
    FunctionResultsUnsupported,
    ArgumentLimitExceeded,
    EntryArgumentsMismatch,
    BlockLimitExceeded,
    MissingBlock,
    MissingBlockMarker,
    NonCanonicalBlockId { expected: u32, found: u32 },
    NonEntryBlockArguments,
    BlockMarkerOutsideFunction,
    BlockMarkerNotFirst,
    ReturnOutsideFunction,
    InvalidProductionLocator(&'static str),
}

impl fmt::Display for MirDialectVerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAttribute(name) => write!(formatter, "missing MIR attribute `{name}`"),
            Self::InvalidIdentity => formatter.write_str("invalid MIR identity attribute"),
            Self::InvalidLimits => formatter.write_str("invalid MIR limits attribute"),
            Self::InvalidTypeId(id) => write!(formatter, "invalid MIR type id {id}"),
            Self::InvalidBlockId(id) => write!(formatter, "invalid MIR block id {id}"),
            Self::InvalidSemanticIdentity => formatter.write_str("invalid MIR semantic identity"),
            Self::InvalidSemanticKind(kind) => {
                write!(formatter, "invalid MIR semantic operation kind {kind}")
            }
            Self::InvalidSemanticSpan => formatter.write_str("invalid MIR semantic source span"),
            Self::InvalidSemanticSuccessors => {
                formatter.write_str("invalid MIR semantic successor list")
            }
            Self::SemanticSuccessorTargetMissing(target) => {
                write!(
                    formatter,
                    "MIR semantic successor block {target} does not exist"
                )
            }
            Self::SemanticSuccessorOrderMismatch => formatter.write_str(
                "MIR semantic successor pointers do not match ordered target identities",
            ),
            Self::SemanticSuccessorArityUnsupported(target) => write!(
                formatter,
                "MIR semantic successor block {target} has arguments but the edge has none"
            ),
            Self::InvalidSemanticOperationOrder => {
                formatter.write_str("invalid MIR semantic operation order")
            }
            Self::SemanticStatementOutsideFunction => {
                formatter.write_str("mir.semantic_statement must be nested in mir.func")
            }
            Self::SemanticTerminatorOutsideFunction => {
                formatter.write_str("mir.semantic_terminator must be nested in mir.func")
            }
            Self::SemanticStatementKindMismatch => {
                formatter.write_str("mir.semantic_statement requires a statement kind")
            }
            Self::SemanticTerminatorKindMismatch => {
                formatter.write_str("mir.semantic_terminator requires a terminator kind")
            }
            Self::UnexpectedModuleChild => {
                formatter.write_str("mir.module may contain only mir.func operations")
            }
            Self::DuplicateFunctionIdentity(identity) => {
                write!(formatter, "duplicate MIR function identity `{identity}`")
            }
            Self::FunctionLimitExceeded => {
                formatter.write_str("mir.module exceeds its function limit")
            }
            Self::IdentityLimitExceeded => {
                formatter.write_str("MIR identity exceeds its configured module limit")
            }
            Self::InconsistentLimits => {
                formatter.write_str("mir.func limits differ from its parent mir.module")
            }
            Self::FunctionOutsideModule => {
                formatter.write_str("mir.func must be nested directly in mir.module")
            }
            Self::InvalidFunctionType => {
                formatter.write_str("mir.func requires a builtin function type")
            }
            Self::FunctionResultsUnsupported => formatter
                .write_str("the D1 shell accepts only place-based functions with no SSA results"),
            Self::ArgumentLimitExceeded => {
                formatter.write_str("mir.func exceeds the MIR argument limit")
            }
            Self::EntryArgumentsMismatch => {
                formatter.write_str("mir.func entry arguments do not match its function type")
            }
            Self::BlockLimitExceeded => formatter.write_str("mir.func exceeds its block limit"),
            Self::MissingBlock => formatter.write_str("mir.func requires an entry block"),
            Self::MissingBlockMarker => {
                formatter.write_str("every MIR CFG block must start with mir.block")
            }
            Self::NonCanonicalBlockId { expected, found } => write!(
                formatter,
                "MIR block id {found} is non-canonical; expected {expected}"
            ),
            Self::NonEntryBlockArguments => {
                formatter.write_str("only the MIR entry block may have arguments")
            }
            Self::BlockMarkerOutsideFunction => {
                formatter.write_str("mir.block must be nested in mir.func")
            }
            Self::BlockMarkerNotFirst => {
                formatter.write_str("mir.block must be the first operation in its CFG block")
            }
            Self::ReturnOutsideFunction => {
                formatter.write_str("mir.return must be nested in mir.func")
            }
            Self::InvalidProductionLocator(reason) => {
                write!(formatter, "invalid production locator IR: {reason}")
            }
        }
    }
}

impl Error for MirDialectVerifyError {}

#[pliron_attr(name = "mir.identity", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirIdentityAttr(StringAttr);

impl MirIdentityAttr {
    /// Creates an unverified attribute. Operation verification applies the hard cap.
    pub fn new(value: impl Into<String>) -> Self {
        Self(StringAttr::new(value.into()))
    }

    fn try_new_bounded(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, MirDialectBuildError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MirDialectBuildError::EmptyIdentity);
        }
        if value.len() > max_bytes {
            return Err(MirDialectBuildError::IdentityTooLong {
                bytes: value.len(),
                limit: max_bytes,
            });
        }
        Ok(Self::new(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Verify for MirIdentityAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.as_str().is_empty() || self.as_str().len() > MAX_EXECUTABLE_IDENTITY_BYTES {
            return verify_err_noloc!(MirDialectVerifyError::InvalidIdentity);
        }
        Ok(())
    }
}

#[pliron_attr(
    name = "mir.limits",
    format = "`<` $max_functions `,` $max_blocks_per_function `,` $max_identity_bytes `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirLimitsAttr {
    max_functions: u32,
    max_blocks_per_function: u32,
    max_identity_bytes: u32,
}

impl MirLimitsAttr {
    pub fn new(limits: MirDialectLimits) -> Self {
        Self {
            max_functions: limits.max_functions as u32,
            max_blocks_per_function: limits.max_blocks_per_function as u32,
            max_identity_bytes: limits.max_identity_bytes as u32,
        }
    }

    pub fn limits(&self) -> MirDialectLimits {
        MirDialectLimits {
            max_functions: self.max_functions as usize,
            max_blocks_per_function: self.max_blocks_per_function as usize,
            max_identity_bytes: self.max_identity_bytes as usize,
        }
    }
}

impl Verify for MirLimitsAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        let limits = self.limits();
        if MirDialectLimits::new(
            limits.max_functions,
            limits.max_blocks_per_function,
            limits.max_identity_bytes,
        )
        .is_err()
        {
            return verify_err_noloc!(MirDialectVerifyError::InvalidLimits);
        }
        Ok(())
    }
}

#[pliron_attr(name = "mir.block_id", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirBlockIdAttr(u32);

impl MirBlockIdAttr {
    pub const fn new(id: MirBlockId) -> Self {
        Self(id.0)
    }

    pub const fn value(self) -> MirBlockId {
        MirBlockId(self.0)
    }
}

impl Verify for MirBlockIdAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.0 as usize >= MAX_EXECUTABLE_BLOCKS {
            return verify_err_noloc!(MirDialectVerifyError::InvalidBlockId(self.0));
        }
        Ok(())
    }
}

/// Fixed-width identity of one exact optimized-rustc MIR operation.
#[pliron_attr(
    name = "mir.semantic_identity",
    format = "`<` $word0 `,` $word1 `,` $word2 `,` $word3 `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirSemanticIdentityAttr {
    word0: u64,
    word1: u64,
    word2: u64,
    word3: u64,
}

impl MirSemanticIdentityAttr {
    /// Creates a nonzero semantic identity.
    pub fn new(words: [u64; 4]) -> Result<Self, MirDialectBuildError> {
        if words == [0; 4] {
            return Err(MirDialectBuildError::InvalidSemanticIdentity);
        }
        Ok(Self {
            word0: words[0],
            word1: words[1],
            word2: words[2],
            word3: words[3],
        })
    }

    /// Returns the identity words.
    pub const fn words(&self) -> [u64; 4] {
        [self.word0, self.word1, self.word2, self.word3]
    }
}

impl Verify for MirSemanticIdentityAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.words() == [0; 4] {
            return verify_err_noloc!(MirDialectVerifyError::InvalidSemanticIdentity);
        }
        Ok(())
    }
}

/// Typed rustc MIR statement or terminator classification.
#[pliron_attr(name = "mir.semantic_kind", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirSemanticKindAttr(u16);

impl MirSemanticKindAttr {
    /// Creates a typed classification attribute.
    pub const fn new(kind: MirSemanticOperationKind) -> Self {
        Self(kind as u16)
    }

    /// Returns the validated operation classification.
    pub const fn kind(self) -> Option<MirSemanticOperationKind> {
        MirSemanticOperationKind::from_raw(self.0)
    }
}

impl Verify for MirSemanticKindAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.kind().is_none() {
            return verify_err_noloc!(MirDialectVerifyError::InvalidSemanticKind(self.0));
        }
        Ok(())
    }
}

/// Exact source coordinates attached to one imported rustc MIR operation.
#[pliron_attr(
    name = "mir.semantic_span",
    format = "`<` $file_word0 `,` $file_word1 `,` $file_word2 `,` $file_word3 `,` $start_line `,` $start_column `,` $end_line `,` $end_column `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirSemanticSpanAttr {
    file_word0: u64,
    file_word1: u64,
    file_word2: u64,
    file_word3: u64,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

impl MirSemanticSpanAttr {
    /// Creates a typed span from checked source coordinates.
    pub const fn new(span: MirSemanticSourceSpan) -> Self {
        Self {
            file_word0: span.file_identity[0],
            file_word1: span.file_identity[1],
            file_word2: span.file_identity[2],
            file_word3: span.file_identity[3],
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        }
    }

    /// Returns the checked source coordinates.
    pub fn span(&self) -> Result<MirSemanticSourceSpan, MirDialectBuildError> {
        MirSemanticSourceSpan::new(
            [
                self.file_word0,
                self.file_word1,
                self.file_word2,
                self.file_word3,
            ],
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column,
        )
    }
}

impl Verify for MirSemanticSpanAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.span().is_err() {
            return verify_err_noloc!(MirDialectVerifyError::InvalidSemanticSpan);
        }
        Ok(())
    }
}

/// Exact operation ordinal within its optimized-rustc basic block.
#[pliron_attr(name = "mir.semantic_ordinal", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirSemanticOrdinalAttr(u32);

impl MirSemanticOrdinalAttr {
    /// Creates an ordinal attribute.
    pub const fn new(ordinal: u32) -> Self {
        Self(ordinal)
    }

    /// Returns the operation ordinal.
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl Verify for MirSemanticOrdinalAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        Ok(())
    }
}

/// Ordered CFG targets attached to one imported rustc MIR terminator.
#[pliron_attr(name = "mir.semantic_successors")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirSemanticSuccessorsAttr(Option<Vec<u32>>);

impl MirSemanticSuccessorsAttr {
    /// Creates a bounded, order-preserving target list.
    pub fn new(targets: &[u32]) -> Result<Self, MirDialectBuildError> {
        if targets.len() > MAX_IMPORTED_MIR_SUCCESSORS {
            return Err(MirDialectBuildError::TooManySemanticSuccessors {
                count: targets.len(),
                limit: MAX_IMPORTED_MIR_SUCCESSORS,
            });
        }
        Ok(Self(Some(targets.to_vec())))
    }

    /// Returns typed targets in exact rustc successor order.
    pub fn targets(&self) -> Result<&[u32], MirDialectBuildError> {
        let targets = self
            .0
            .as_deref()
            .ok_or(MirDialectBuildError::MalformedOperation(
                "non-canonical successor list",
            ))?;
        if targets.len() > MAX_IMPORTED_MIR_SUCCESSORS {
            return Err(MirDialectBuildError::TooManySemanticSuccessors {
                count: targets.len(),
                limit: MAX_IMPORTED_MIR_SUCCESSORS,
            });
        }
        Ok(targets)
    }

    fn from_text(text: &str) -> Self {
        Self(parse_semantic_successor_text(text).ok())
    }
}

fn parse_semantic_successor_text(text: &str) -> Result<Vec<u32>, MirDialectBuildError> {
    if text.len() > MAX_IMPORTED_MIR_SUCCESSOR_TEXT_BYTES {
        return Err(MirDialectBuildError::SemanticSuccessorTextTooLong {
            bytes: text.len(),
            limit: MAX_IMPORTED_MIR_SUCCESSOR_TEXT_BYTES,
        });
    }
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let count = text.bytes().try_fold(1_usize, |count, byte| {
        if byte == b',' {
            count.checked_add(1)
        } else {
            Some(count)
        }
    });
    let count = count.ok_or(MirDialectBuildError::TooManySemanticSuccessors {
        count: usize::MAX,
        limit: MAX_IMPORTED_MIR_SUCCESSORS,
    })?;
    if count > MAX_IMPORTED_MIR_SUCCESSORS {
        return Err(MirDialectBuildError::TooManySemanticSuccessors {
            count,
            limit: MAX_IMPORTED_MIR_SUCCESSORS,
        });
    }
    let mut targets = Vec::with_capacity(count);
    for target in text.split(',') {
        if target.is_empty()
            || (target.len() > 1 && target.starts_with('0'))
            || !target.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(MirDialectBuildError::MalformedOperation(
                "non-canonical successor list",
            ));
        }
        targets.push(
            target
                .parse::<u32>()
                .map_err(|_| MirDialectBuildError::MalformedOperation("invalid successor"))?,
        );
    }
    if targets.len() != count {
        return Err(MirDialectBuildError::MalformedOperation(
            "non-canonical successor list",
        ));
    }
    Ok(targets)
}

impl Printable for MirSemanticSuccessorsAttr {
    fn fmt(
        &self,
        _context: &Context,
        _state: &printable::State,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        #[cfg(test)]
        SEMANTIC_SUCCESSOR_TEXT_PRINT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        formatter.write_str("\"")?;
        let Some(targets) = &self.0 else {
            return formatter.write_str("<invalid>\"");
        };
        for (index, target) in targets.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{target}")?;
        }
        formatter.write_str("\"")
    }
}

impl Parsable for MirSemanticSuccessorsAttr {
    type Arg = ();
    type Parsed = Self;

    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        _arg: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        #[cfg(test)]
        SEMANTIC_SUCCESSOR_TEXT_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        StringAttr::parser(())
            .map(|text| Self::from_text(text.as_str()))
            .parse_stream(state_stream)
            .into()
    }
}

#[cfg(test)]
static SEMANTIC_SUCCESSOR_TEXT_PARSE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static SEMANTIC_SUCCESSOR_TEXT_PRINT_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl Verify for MirSemanticSuccessorsAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.targets().is_err() {
            return verify_err_noloc!(MirDialectVerifyError::InvalidSemanticSuccessors);
        }
        Ok(())
    }
}

const fn production_edge_role_code(role: SemanticEdgeRoleV1) -> u8 {
    match role {
        SemanticEdgeRoleV1::Goto => 0,
        SemanticEdgeRoleV1::SwitchValue => 1,
        SemanticEdgeRoleV1::SwitchOtherwise => 2,
        SemanticEdgeRoleV1::CallReturn => 3,
        SemanticEdgeRoleV1::CallUnwind => 4,
        SemanticEdgeRoleV1::TailCallUnwind => 5,
        SemanticEdgeRoleV1::DropReturn => 6,
        SemanticEdgeRoleV1::DropUnwind => 7,
        SemanticEdgeRoleV1::AssertSuccess => 8,
        SemanticEdgeRoleV1::AssertUnwind => 9,
        SemanticEdgeRoleV1::FalseEdgeReal => 10,
        SemanticEdgeRoleV1::FalseEdgeImaginary => 11,
    }
}

const fn production_edge_role_from_code(code: u8) -> Option<SemanticEdgeRoleV1> {
    Some(match code {
        0 => SemanticEdgeRoleV1::Goto,
        1 => SemanticEdgeRoleV1::SwitchValue,
        2 => SemanticEdgeRoleV1::SwitchOtherwise,
        3 => SemanticEdgeRoleV1::CallReturn,
        4 => SemanticEdgeRoleV1::CallUnwind,
        5 => SemanticEdgeRoleV1::TailCallUnwind,
        6 => SemanticEdgeRoleV1::DropReturn,
        7 => SemanticEdgeRoleV1::DropUnwind,
        8 => SemanticEdgeRoleV1::AssertSuccess,
        9 => SemanticEdgeRoleV1::AssertUnwind,
        10 => SemanticEdgeRoleV1::FalseEdgeReal,
        11 => SemanticEdgeRoleV1::FalseEdgeImaginary,
        _ => return None,
    })
}

/// Fixed-width locator for the retained semantic MIR module.
#[pliron_attr(
    name = "mir.production_semantic_sha256_v1",
    format = "`<` $word0 `,` $word1 `,` $word2 `,` $word3 `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct MirProductionSemanticSha256AttrV1 {
    word0: u64,
    word1: u64,
    word2: u64,
    word3: u64,
}

impl MirProductionSemanticSha256AttrV1 {
    fn new(value: MirProductionSemanticSha256V1) -> Self {
        let bytes = value.0;
        Self {
            word0: u64::from_le_bytes(bytes[0..8].try_into().expect("fixed SHA-256 word")),
            word1: u64::from_le_bytes(bytes[8..16].try_into().expect("fixed SHA-256 word")),
            word2: u64::from_le_bytes(bytes[16..24].try_into().expect("fixed SHA-256 word")),
            word3: u64::from_le_bytes(bytes[24..32].try_into().expect("fixed SHA-256 word")),
        }
    }

    fn value(&self) -> MirProductionSemanticSha256V1 {
        let mut bytes = [0_u8; 32];
        bytes[0..8].copy_from_slice(&self.word0.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.word1.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.word2.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.word3.to_le_bytes());
        MirProductionSemanticSha256V1(bytes)
    }
}

impl Verify for MirProductionSemanticSha256AttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        Ok(())
    }
}

#[pliron_attr(name = "mir.production_function_id_v1", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MirProductionFunctionIdAttrV1(u32);

impl MirProductionFunctionIdAttrV1 {
    const fn new(id: SemanticFunctionIdV1) -> Self {
        Self(id.index())
    }

    const fn value(self) -> SemanticFunctionIdV1 {
        SemanticFunctionIdV1::from_index(self.0)
    }
}

impl Verify for MirProductionFunctionIdAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if u64::from(self.0) >= HARD_MAX_FUNCTIONS_V1 {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "function ID is out of range"
            ));
        }
        Ok(())
    }
}

#[pliron_attr(name = "mir.production_block_id_v1", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MirProductionBlockIdAttrV1(u32);

impl MirProductionBlockIdAttrV1 {
    const fn new(id: SemanticBlockIdV1) -> Self {
        Self(id.index())
    }

    const fn value(self) -> SemanticBlockIdV1 {
        SemanticBlockIdV1::from_index(self.0)
    }
}

impl Verify for MirProductionBlockIdAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if u64::from(self.0) >= HARD_MAX_BLOCKS_V1 {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "block ID is out of range"
            ));
        }
        Ok(())
    }
}

#[pliron_attr(name = "mir.production_source_block_ordinal_v1", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MirProductionSourceBlockOrdinalAttrV1(u32);

impl MirProductionSourceBlockOrdinalAttrV1 {
    const fn new(ordinal: u32) -> Self {
        Self(ordinal)
    }

    const fn value(self) -> u32 {
        self.0
    }
}

impl Verify for MirProductionSourceBlockOrdinalAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if u64::from(self.0) >= HARD_MAX_BLOCKS_V1 {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "source block ordinal is out of range"
            ));
        }
        Ok(())
    }
}

#[pliron_attr(name = "mir.production_statement_ordinal_v1", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MirProductionStatementOrdinalAttrV1(u32);

impl MirProductionStatementOrdinalAttrV1 {
    const fn new(ordinal: u32) -> Self {
        Self(ordinal)
    }

    const fn value(self) -> u32 {
        self.0
    }
}

impl Verify for MirProductionStatementOrdinalAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if u64::from(self.0) >= HARD_MAX_STATEMENTS_V1 {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "statement ordinal is out of range"
            ));
        }
        Ok(())
    }
}

/// Ordered semantic roles and block locators for one production terminator.
#[pliron_attr(name = "mir.production_successor_arcs_v1")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct MirProductionSuccessorArcsAttrV1(Option<Vec<MirProductionSuccessorArcV1>>);

impl MirProductionSuccessorArcsAttrV1 {
    fn new(arcs: &[MirProductionSuccessorArcV1]) -> Self {
        Self(Some(arcs.to_vec()))
    }

    fn arcs(&self) -> Option<&[MirProductionSuccessorArcV1]> {
        self.0.as_deref().filter(|arcs| {
            arcs.len() <= MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1
                && arcs
                    .iter()
                    .all(|arc| u64::from(arc.target.index()) < HARD_MAX_BLOCKS_V1)
        })
    }

    fn from_text(text: &str) -> Self {
        Self(parse_production_arc_text_v1(text))
    }
}

fn parse_canonical_decimal_u32(text: &str) -> Option<u32> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}

fn parse_production_arc_text_v1(text: &str) -> Option<Vec<MirProductionSuccessorArcV1>> {
    if text.len() > MAX_PRODUCTION_LOCATOR_ARC_TEXT_BYTES_V1 {
        return None;
    }
    if text.is_empty() {
        return Some(Vec::new());
    }
    let count = text.bytes().filter(|byte| *byte == b',').count() + 1;
    if count > MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1 {
        return None;
    }
    let mut arcs = Vec::with_capacity(count);
    for encoded in text.split(',') {
        let (role, target) = encoded.split_once(':')?;
        if target.contains(':') {
            return None;
        }
        let role = parse_canonical_decimal_u32(role)?;
        let role = u8::try_from(role)
            .ok()
            .and_then(production_edge_role_from_code)?;
        let target = parse_canonical_decimal_u32(target)?;
        arcs.push(MirProductionSuccessorArcV1::new(
            role,
            SemanticBlockIdV1::from_index(target),
        ));
    }
    (arcs.len() == count).then_some(arcs)
}

impl Printable for MirProductionSuccessorArcsAttrV1 {
    fn fmt(
        &self,
        _context: &Context,
        _state: &printable::State,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str("\"")?;
        let Some(arcs) = &self.0 else {
            return formatter.write_str("<invalid>\"");
        };
        for (index, arc) in arcs.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(
                formatter,
                "{}:{}",
                production_edge_role_code(arc.role),
                arc.target.index()
            )?;
        }
        formatter.write_str("\"")
    }
}

impl Parsable for MirProductionSuccessorArcsAttrV1 {
    type Arg = ();
    type Parsed = Self;

    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        _arg: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        StringAttr::parser(())
            .map(|text| Self::from_text(text.as_str()))
            .parse_stream(state_stream)
            .into()
    }
}

impl Verify for MirProductionSuccessorArcsAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.arcs().is_none() {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "successor arcs are unknown, oversized, or non-canonical"
            ));
        }
        Ok(())
    }
}

#[pliron_attr(name = "mir.production_tree_work_limit_v1", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MirProductionTreeWorkLimitAttrV1(u64);

impl MirProductionTreeWorkLimitAttrV1 {
    const fn new(limits: MirProductionPlironLimitsV1) -> Self {
        Self(limits.max_tree_work)
    }

    const fn limits(self) -> MirProductionPlironLimitsV1 {
        MirProductionPlironLimitsV1 {
            max_tree_work: self.0,
        }
    }
}

impl Verify for MirProductionTreeWorkLimitAttrV1 {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if MirProductionPlironLimitsV1::new(self.0).is_err() {
            return verify_err_noloc!(MirDialectVerifyError::InvalidProductionLocator(
                "middle-end tree-work limit is invalid"
            ));
        }
        Ok(())
    }
}

/// A bounded typed index for a future validated [`fe2o3_mir_model`] type table.
///
/// This shell enforces the frozen table-size cap. A later importer remains
/// responsible for proving that the referenced entry exists in its module.
#[pliron_type(name = "mir.type_ref", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirTypeRef(u32);

impl MirTypeRef {
    pub fn get(
        context: &Context,
        id: MirTypeId,
    ) -> Result<TypedHandle<Self>, MirDialectBuildError> {
        if id.0 as usize >= MAX_EXECUTABLE_TYPES {
            return Err(MirDialectBuildError::TypeIdOutOfRange(id));
        }
        Ok(Self::instantiate(Self(id.0), context))
    }

    pub const fn value(&self) -> MirTypeId {
        MirTypeId(self.0)
    }
}

impl Verify for MirTypeRef {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.0 as usize >= MAX_EXECUTABLE_TYPES {
            return verify_err_noloc!(MirDialectVerifyError::InvalidTypeId(self.0));
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.module",
    format,
    interfaces = [
        OneRegionInterface,
        SingleBlockRegionInterface,
        NoTerminatorInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        module_identity: MirIdentityAttr,
        module_limits: MirLimitsAttr
    )
)]
pub struct MirModuleOp;

#[op_interface_impl]
impl RegionKindInterface for MirModuleOp {
    fn get_region_kind(&self, _index: usize) -> RegionKind {
        RegionKind::Graph
    }
}

impl MirModuleOp {
    pub fn try_new(
        context: &mut Context,
        identity: impl Into<String>,
        limits: MirDialectLimits,
    ) -> Result<Self, MirDialectBuildError> {
        let identity = MirIdentityAttr::try_new_bounded(identity, limits.max_identity_bytes)?;
        ensure_mir_context_owner(context)?;
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            1,
        );
        let module = Self { op };
        module.set_attr_module_identity(context, identity);
        module.set_attr_module_limits(context, MirLimitsAttr::new(limits));

        let body = BasicBlock::new(
            context,
            Some("module".try_into().expect("valid label")),
            vec![],
        );
        body.insert_at_front(op.deref(context).get_region(0), context);
        Ok(module)
    }

    /// Returns the owner-bound block containing this module's functions.
    pub fn body(&self, context: &Context) -> Result<MirModuleBodyHandle, MirBlockHandleError> {
        let owner =
            require_context_identity(context).map_err(MirBlockHandleError::ContextIdentity)?;
        let pointer = catch_unwind(AssertUnwindSafe(|| self.body_raw(context)))
            .map_err(|_| MirBlockHandleError::UpstreamPanicked)?;
        let parent = self.get_operation();
        let handle = MirModuleBodyHandle {
            owner,
            pointer,
            parent,
        };
        authenticate_owned_block(owner, pointer, parent, MirBlockRole::ModuleBody, context)?;
        Ok(handle)
    }

    fn body_raw(&self, context: &Context) -> Ptr<BasicBlock> {
        self.get_region(context)
            .deref(context)
            .get_head()
            .expect("verified mir.module has one body block")
    }

    pub fn function_count(&self, context: &Context) -> usize {
        self.body_raw(context).deref(context).iter(context).count()
    }

    pub fn append_function(
        &self,
        context: &mut Context,
        identity: impl Into<String>,
        arguments: &[MirTypeId],
    ) -> Result<MirFunctionOp, MirDialectBuildError> {
        let limits = self
            .get_attr_module_limits(context)
            .map(|attr| attr.limits())
            .ok_or(MirDialectBuildError::MalformedOperation(
                "module limits are missing",
            ))?;
        if self.function_count(context) >= limits.max_functions {
            return Err(MirDialectBuildError::FunctionLimitExceeded {
                limit: limits.max_functions,
            });
        }
        if arguments.len() > MAX_EXECUTABLE_BLOCK_PARAMETERS {
            return Err(MirDialectBuildError::TooManyArguments {
                count: arguments.len(),
                limit: MAX_EXECUTABLE_BLOCK_PARAMETERS,
            });
        }
        let identity = MirIdentityAttr::try_new_bounded(identity, limits.max_identity_bytes)?;
        let argument_types = arguments
            .iter()
            .copied()
            .map(|id| MirTypeRef::get(context, id).map(|ty| ty.to_handle()))
            .collect::<Result<Vec<_>, _>>()?;
        let function = MirFunctionOp::new(context, identity, limits, argument_types);
        function
            .get_operation()
            .insert_at_back(self.body_raw(context), context);
        Ok(function)
    }
}

impl Verify for MirModuleOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let Some(identity) = self.get_attr_module_identity(context) else {
            return verify_err!(
                op.deref(context).loc(),
                MirDialectVerifyError::MissingAttribute("module_identity")
            );
        };
        let Some(limits) = self.get_attr_module_limits(context) else {
            return verify_err!(
                op.deref(context).loc(),
                MirDialectVerifyError::MissingAttribute("module_limits")
            );
        };
        let limits = limits.limits();
        if identity.as_str().len() > limits.max_identity_bytes {
            return verify_err!(
                op.deref(context).loc(),
                MirDialectVerifyError::IdentityLimitExceeded
            );
        }

        let mut identities = BTreeSet::new();
        let functions: Vec<_> = self
            .body_raw(context)
            .deref(context)
            .iter(context)
            .collect();
        if functions.len() > limits.max_functions {
            return verify_err!(
                op.deref(context).loc(),
                MirDialectVerifyError::FunctionLimitExceeded
            );
        }
        for function in functions {
            let Some(function) = Operation::get_op::<MirFunctionOp>(function, context) else {
                return verify_err!(
                    op.deref(context).loc(),
                    MirDialectVerifyError::UnexpectedModuleChild
                );
            };
            let Some(function_identity) = function.get_attr_function_identity(context) else {
                return verify_err!(
                    op.deref(context).loc(),
                    MirDialectVerifyError::MissingAttribute("function_identity")
                );
            };
            if !identities.insert(function_identity.as_str().to_owned()) {
                return verify_err!(
                    op.deref(context).loc(),
                    MirDialectVerifyError::DuplicateFunctionIdentity(
                        function_identity.as_str().to_owned()
                    )
                );
            }
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.func",
    format,
    interfaces = [
        OneRegionInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        function_identity: MirIdentityAttr,
        function_limits: MirLimitsAttr,
        function_signature: TypeAttr
    )
)]
pub struct MirFunctionOp;

#[op_interface_impl]
impl RegionKindInterface for MirFunctionOp {
    fn get_region_kind(&self, _index: usize) -> RegionKind {
        RegionKind::SSACFG
    }
}

impl MirFunctionOp {
    fn new(
        context: &mut Context,
        identity: MirIdentityAttr,
        limits: MirDialectLimits,
        argument_types: Vec<TypeHandle>,
    ) -> Self {
        let signature = FunctionType::get(context, argument_types.clone(), vec![]);
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            1,
        );
        let function = Self { op };
        function.set_attr_function_identity(context, identity);
        function.set_attr_function_limits(context, MirLimitsAttr::new(limits));
        function.set_attr_function_signature(context, TypeAttr::new(signature.into()));

        let entry = BasicBlock::new(
            context,
            Some("bb0".try_into().expect("valid label")),
            argument_types,
        );
        entry.insert_at_front(op.deref(context).get_region(0), context);
        MirBlockOp::new(context, MirBlockId(0))
            .get_operation()
            .insert_at_back(entry, context);
        MirReturnOp::new(context)
            .get_operation()
            .insert_at_back(entry, context);
        function
    }

    /// Returns the owner-bound entry block for this function.
    pub fn entry_block(&self, context: &Context) -> Result<MirBlockHandle, MirBlockHandleError> {
        let owner =
            require_context_identity(context).map_err(MirBlockHandleError::ContextIdentity)?;
        let pointer = catch_unwind(AssertUnwindSafe(|| self.entry_block_raw(context)))
            .map_err(|_| MirBlockHandleError::UpstreamPanicked)?;
        let parent = self.get_operation();
        let handle = MirBlockHandle {
            owner,
            pointer,
            parent,
            role: MirBlockRole::FunctionEntry,
        };
        handle.authenticate(context)?;
        Ok(handle)
    }

    fn entry_block_raw(&self, context: &Context) -> Ptr<BasicBlock> {
        self.get_region(context)
            .deref(context)
            .get_head()
            .expect("verified mir.func has an entry block")
    }

    pub fn block_count(&self, context: &Context) -> usize {
        self.get_region(context)
            .deref(context)
            .iter(context)
            .count()
    }

    pub fn append_block(
        &self,
        context: &mut Context,
    ) -> Result<MirBlockHandle, MirDialectBuildError> {
        let owner = ensure_mir_context_owner(context)?;
        catch_unwind(AssertUnwindSafe(|| {
            let limits = self
                .get_attr_function_limits(context)
                .map(|attr| attr.limits())
                .ok_or(MirDialectBuildError::MalformedOperation(
                    "function limits are missing",
                ))?;
            let block_count = self.block_count(context);
            if block_count >= limits.max_blocks_per_function {
                return Err(MirDialectBuildError::BlockLimitExceeded {
                    limit: limits.max_blocks_per_function,
                });
            }
            let id = u32::try_from(block_count).map_err(|_| {
                MirDialectBuildError::BlockLimitExceeded {
                    limit: limits.max_blocks_per_function,
                }
            })?;
            let block = BasicBlock::new(
                context,
                Some(format!("bb{id}").try_into().expect("valid generated label")),
                vec![],
            );
            block.insert_at_back(self.get_region(context), context);
            MirBlockOp::new(context, MirBlockId(id))
                .get_operation()
                .insert_at_back(block, context);
            MirReturnOp::new(context)
                .get_operation()
                .insert_at_back(block, context);
            Ok(MirBlockHandle {
                owner,
                pointer: block,
                parent: self.get_operation(),
                role: MirBlockRole::FunctionBlock,
            })
        }))
        .unwrap_or(Err(MirDialectBuildError::UpstreamPanicked))
    }

    pub fn signature(&self, context: &Context) -> Option<TypeHandle> {
        self.get_attr_function_signature(context)
            .map(|attribute| attribute.get_type(context))
    }
}

impl Verify for MirFunctionOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let location = op.deref(context).loc();
        let parent = op.deref(context).get_parent_op(context);
        let Some(parent) =
            parent.and_then(|parent| Operation::get_op::<MirModuleOp>(parent, context))
        else {
            return verify_err!(location, MirDialectVerifyError::FunctionOutsideModule);
        };
        let Some(identity) = self.get_attr_function_identity(context) else {
            return verify_err!(
                location,
                MirDialectVerifyError::MissingAttribute("function_identity")
            );
        };
        let Some(limits) = self.get_attr_function_limits(context) else {
            return verify_err!(
                location,
                MirDialectVerifyError::MissingAttribute("function_limits")
            );
        };
        let limits = limits.limits();
        let Some(parent_limits) = parent.get_attr_module_limits(context) else {
            return verify_err!(
                location,
                MirDialectVerifyError::MissingAttribute("module_limits")
            );
        };
        if limits != parent_limits.limits() {
            return verify_err!(location, MirDialectVerifyError::InconsistentLimits);
        }
        if identity.as_str().len() > limits.max_identity_bytes {
            return verify_err!(location, MirDialectVerifyError::IdentityLimitExceeded);
        }
        let Some(signature) = self.signature(context) else {
            return verify_err!(
                location,
                MirDialectVerifyError::MissingAttribute("function_signature")
            );
        };
        let signature_ref = signature.deref(context);
        let Some(signature) = signature_ref.downcast_ref::<FunctionType>() else {
            return verify_err!(location, MirDialectVerifyError::InvalidFunctionType);
        };
        if !signature.res_types().is_empty() {
            return verify_err!(location, MirDialectVerifyError::FunctionResultsUnsupported);
        }
        if signature.arg_types().len() > MAX_EXECUTABLE_BLOCK_PARAMETERS {
            return verify_err!(location, MirDialectVerifyError::ArgumentLimitExceeded);
        }

        let blocks: Vec<_> = self
            .get_region(context)
            .deref(context)
            .iter(context)
            .collect();
        if blocks.is_empty() {
            return verify_err!(location, MirDialectVerifyError::MissingBlock);
        }
        if blocks.len() > limits.max_blocks_per_function {
            return verify_err!(location, MirDialectVerifyError::BlockLimitExceeded);
        }
        let entry_arguments: Vec<_> = blocks[0]
            .deref(context)
            .arguments()
            .map(|argument| argument.get_type(context))
            .collect();
        if entry_arguments != signature.arg_types() {
            return verify_err!(location, MirDialectVerifyError::EntryArgumentsMismatch);
        }

        for (index, block) in blocks.iter().copied().enumerate() {
            if index != 0 && block.deref(context).get_num_arguments() != 0 {
                return verify_err!(location, MirDialectVerifyError::NonEntryBlockArguments);
            }
            let Some(marker) = block.deref(context).get_head() else {
                return verify_err!(location, MirDialectVerifyError::MissingBlockMarker);
            };
            let Some(marker) = Operation::get_op::<MirBlockOp>(marker, context) else {
                return verify_err!(location, MirDialectVerifyError::MissingBlockMarker);
            };
            let Some(block_id) = marker.get_attr_block_id(context) else {
                return verify_err!(
                    location,
                    MirDialectVerifyError::MissingAttribute("block_id")
                );
            };
            let expected = index as u32;
            let found = block_id.value().0;
            if found != expected {
                return verify_err!(
                    location,
                    MirDialectVerifyError::NonCanonicalBlockId { expected, found }
                );
            }
            let operations = block.deref(context).iter(context).collect::<Vec<_>>();
            let mut expected_ordinal = 0_u32;
            for (operation_index, operation) in operations.iter().copied().enumerate().skip(1) {
                if let Some(statement) =
                    Operation::get_op::<MirSemanticStatementOp>(operation, context)
                {
                    let Some(snapshot) = statement.semantic_snapshot(context) else {
                        return verify_err!(
                            location,
                            MirDialectVerifyError::InvalidSemanticOperationOrder
                        );
                    };
                    if snapshot.ordinal() != expected_ordinal
                        || operation_index + 1 == operations.len()
                    {
                        return verify_err!(
                            location,
                            MirDialectVerifyError::InvalidSemanticOperationOrder
                        );
                    }
                    expected_ordinal += 1;
                } else if let Some(terminator) =
                    Operation::get_op::<MirSemanticTerminatorOp>(operation, context)
                {
                    let Some(snapshot) = terminator.semantic_snapshot(context) else {
                        return verify_err!(
                            location,
                            MirDialectVerifyError::InvalidSemanticOperationOrder
                        );
                    };
                    if snapshot.ordinal() != expected_ordinal
                        || operation_index + 1 != operations.len()
                    {
                        return verify_err!(
                            location,
                            MirDialectVerifyError::InvalidSemanticOperationOrder
                        );
                    }
                    let operation_ref = operation.deref(context);
                    if operation_ref.get_num_successors() != snapshot.successors().len() {
                        return verify_err!(
                            location,
                            MirDialectVerifyError::SemanticSuccessorOrderMismatch
                        );
                    }
                    for (actual, expected) in operation_ref
                        .successors()
                        .zip(snapshot.successors().iter().copied())
                    {
                        let Some(expected_block) = usize::try_from(expected)
                            .ok()
                            .and_then(|expected| blocks.get(expected))
                            .copied()
                        else {
                            return verify_err!(
                                location,
                                MirDialectVerifyError::SemanticSuccessorTargetMissing(expected)
                            );
                        };
                        if actual != expected_block {
                            return verify_err!(
                                location,
                                MirDialectVerifyError::SemanticSuccessorOrderMismatch
                            );
                        }
                        if actual.deref(context).get_num_arguments() != 0 {
                            return verify_err!(
                                location,
                                MirDialectVerifyError::SemanticSuccessorArityUnsupported(expected)
                            );
                        }
                    }
                } else if Operation::is_op::<MirReturnOp>(operation, context)
                    && (operation_index + 1 != operations.len() || expected_ordinal != 0)
                {
                    return verify_err!(
                        location,
                        MirDialectVerifyError::InvalidSemanticOperationOrder
                    );
                }
            }
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.block",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, NResultsInterface<0>],
    attributes = (block_id: MirBlockIdAttr)
)]
pub struct MirBlockOp;

impl MirBlockOp {
    pub fn new(context: &mut Context, id: MirBlockId) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let block = Self { op };
        block.set_attr_block_id(context, MirBlockIdAttr::new(id));
        block
    }

    pub fn block_id(&self, context: &Context) -> Option<MirBlockId> {
        self.get_attr_block_id(context)
            .map(|attribute| attribute.value())
    }
}

impl Verify for MirBlockOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let location = op.deref(context).loc();
        if self.get_attr_block_id(context).is_none() {
            return verify_err!(
                location,
                MirDialectVerifyError::MissingAttribute("block_id")
            );
        }
        let Some(parent_block) = op.deref(context).get_parent_block() else {
            return verify_err!(location, MirDialectVerifyError::BlockMarkerOutsideFunction);
        };
        let parent = parent_block.deref(context).get_parent_op(context);
        if !parent.is_some_and(|parent| Operation::is_op::<MirFunctionOp>(parent, context)) {
            return verify_err!(location, MirDialectVerifyError::BlockMarkerOutsideFunction);
        }
        if op.deref(context).get_prev().is_some() {
            return verify_err!(location, MirDialectVerifyError::BlockMarkerNotFirst);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.semantic_statement",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, NResultsInterface<0>],
    attributes = (
        semantic_statement_ordinal: MirSemanticOrdinalAttr,
        semantic_statement_kind: MirSemanticKindAttr,
        semantic_statement_identity: MirSemanticIdentityAttr,
        semantic_statement_expansion_span: MirSemanticSpanAttr,
        semantic_statement_call_site_span: MirSemanticSpanAttr
    )
)]
/// One exact optimized-rustc statement retained as typed inert Pliron IR.
pub struct MirSemanticStatementOp;

impl MirSemanticStatementOp {
    fn try_new(
        context: &mut Context,
        ordinal: u32,
        kind: MirSemanticOperationKind,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
    ) -> Result<Self, MirDialectBuildError> {
        if kind.is_terminator() {
            return Err(MirDialectBuildError::InvalidSemanticKind(kind as u16));
        }
        let identity = MirSemanticIdentityAttr::new(identity)?;
        provenance.validate()?;
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let statement = Self { op };
        statement
            .set_attr_semantic_statement_ordinal(context, MirSemanticOrdinalAttr::new(ordinal));
        statement.set_attr_semantic_statement_kind(context, MirSemanticKindAttr::new(kind));
        statement.set_attr_semantic_statement_identity(context, identity);
        statement.set_attr_semantic_statement_expansion_span(
            context,
            MirSemanticSpanAttr::new(provenance.expansion()),
        );
        statement.set_attr_semantic_statement_call_site_span(
            context,
            MirSemanticSpanAttr::new(provenance.call_site()),
        );
        Ok(statement)
    }

    /// Returns exact pointer-independent semantic evidence when all typed
    /// attributes are present and valid.
    pub fn semantic_snapshot(&self, context: &Context) -> Option<MirSemanticOperationSnapshot> {
        Some(MirSemanticOperationSnapshot {
            ordinal: self.get_attr_semantic_statement_ordinal(context)?.value(),
            kind: self.get_attr_semantic_statement_kind(context)?.kind()?,
            identity: self.get_attr_semantic_statement_identity(context)?.words(),
            provenance: MirSemanticSpanProvenance::new(
                self.get_attr_semantic_statement_expansion_span(context)?
                    .span()
                    .ok()?,
                self.get_attr_semantic_statement_call_site_span(context)?
                    .span()
                    .ok()?,
            )
            .ok()?,
            successors: Vec::new(),
        })
    }
}

impl Verify for MirSemanticStatementOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let location = op.deref(context).loc();
        if !op
            .deref(context)
            .get_parent_op(context)
            .is_some_and(|parent| Operation::is_op::<MirFunctionOp>(parent, context))
        {
            return verify_err!(
                location,
                MirDialectVerifyError::SemanticStatementOutsideFunction
            );
        }
        let Some(snapshot) = self.semantic_snapshot(context) else {
            return verify_err!(location, MirDialectVerifyError::InvalidSemanticIdentity);
        };
        if snapshot.kind().is_terminator() {
            return verify_err!(
                location,
                MirDialectVerifyError::SemanticStatementKindMismatch
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.semantic_terminator",
    format,
    interfaces = [
        IsTerminatorInterface,
        NRegionsInterface<0>,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        semantic_terminator_ordinal: MirSemanticOrdinalAttr,
        semantic_terminator_kind: MirSemanticKindAttr,
        semantic_terminator_identity: MirSemanticIdentityAttr,
        semantic_terminator_expansion_span: MirSemanticSpanAttr,
        semantic_terminator_call_site_span: MirSemanticSpanAttr,
        semantic_terminator_successors: MirSemanticSuccessorsAttr
    )
)]
/// One exact optimized-rustc terminator retained as typed inert Pliron IR.
pub struct MirSemanticTerminatorOp;

impl MirSemanticTerminatorOp {
    fn try_new(
        context: &mut Context,
        ordinal: u32,
        kind: MirSemanticOperationKind,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
        successor_ids: &[u32],
        successors: Vec<Ptr<BasicBlock>>,
    ) -> Result<Self, MirDialectBuildError> {
        if !kind.is_terminator() {
            return Err(MirDialectBuildError::InvalidSemanticKind(kind as u16));
        }
        let identity = MirSemanticIdentityAttr::new(identity)?;
        let successor_ids = MirSemanticSuccessorsAttr::new(successor_ids)?;
        provenance.validate()?;
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            successors,
            0,
        );
        let terminator = Self { op };
        terminator
            .set_attr_semantic_terminator_ordinal(context, MirSemanticOrdinalAttr::new(ordinal));
        terminator.set_attr_semantic_terminator_kind(context, MirSemanticKindAttr::new(kind));
        terminator.set_attr_semantic_terminator_identity(context, identity);
        terminator.set_attr_semantic_terminator_expansion_span(
            context,
            MirSemanticSpanAttr::new(provenance.expansion()),
        );
        terminator.set_attr_semantic_terminator_call_site_span(
            context,
            MirSemanticSpanAttr::new(provenance.call_site()),
        );
        terminator.set_attr_semantic_terminator_successors(context, successor_ids);
        Ok(terminator)
    }

    /// Returns exact pointer-independent semantic evidence when all typed
    /// attributes are present and valid.
    pub fn semantic_snapshot(&self, context: &Context) -> Option<MirSemanticOperationSnapshot> {
        Some(MirSemanticOperationSnapshot {
            ordinal: self.get_attr_semantic_terminator_ordinal(context)?.value(),
            kind: self.get_attr_semantic_terminator_kind(context)?.kind()?,
            identity: self.get_attr_semantic_terminator_identity(context)?.words(),
            provenance: MirSemanticSpanProvenance::new(
                self.get_attr_semantic_terminator_expansion_span(context)?
                    .span()
                    .ok()?,
                self.get_attr_semantic_terminator_call_site_span(context)?
                    .span()
                    .ok()?,
            )
            .ok()?,
            successors: self
                .get_attr_semantic_terminator_successors(context)?
                .targets()
                .ok()?
                .to_vec(),
        })
    }
}

impl Verify for MirSemanticTerminatorOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let location = op.deref(context).loc();
        if !op
            .deref(context)
            .get_parent_op(context)
            .is_some_and(|parent| Operation::is_op::<MirFunctionOp>(parent, context))
        {
            return verify_err!(
                location,
                MirDialectVerifyError::SemanticTerminatorOutsideFunction
            );
        }
        let Some(snapshot) = self.semantic_snapshot(context) else {
            return verify_err!(location, MirDialectVerifyError::InvalidSemanticIdentity);
        };
        if !snapshot.kind().is_terminator() {
            return verify_err!(
                location,
                MirDialectVerifyError::SemanticTerminatorKindMismatch
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.return",
    format,
    interfaces = [
        IsTerminatorInterface,
        NRegionsInterface<0>,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ]
)]
pub struct MirReturnOp;

impl MirReturnOp {
    pub fn new(context: &mut Context) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        Self { op }
    }
}

impl Verify for MirReturnOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let op = self.get_operation();
        let location = op.deref(context).loc();
        let parent = op.deref(context).get_parent_op(context);
        if !parent.is_some_and(|parent| Operation::is_op::<MirFunctionOp>(parent, context)) {
            return verify_err!(location, MirDialectVerifyError::ReturnOutsideFunction);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.production_module_v1",
    format,
    interfaces = [
        OneRegionInterface,
        SingleBlockRegionInterface,
        NoTerminatorInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        production_module_sha256: MirProductionSemanticSha256AttrV1,
        production_module_tree_work_limit: MirProductionTreeWorkLimitAttrV1
    )
)]
struct MirProductionModuleOpV1;

#[op_interface_impl]
impl RegionKindInterface for MirProductionModuleOpV1 {
    fn get_region_kind(&self, _index: usize) -> RegionKind {
        RegionKind::Graph
    }
}

impl MirProductionModuleOpV1 {
    fn new(
        context: &mut Context,
        semantic_sha256: MirProductionSemanticSha256V1,
        limits: MirProductionPlironLimitsV1,
    ) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            1,
        );
        let module = Self { op };
        module.set_attr_production_module_sha256(
            context,
            MirProductionSemanticSha256AttrV1::new(semantic_sha256),
        );
        module.set_attr_production_module_tree_work_limit(
            context,
            MirProductionTreeWorkLimitAttrV1::new(limits),
        );
        let body = BasicBlock::new(
            context,
            Some("production_module_v1".try_into().expect("valid label")),
            vec![],
        );
        body.insert_at_front(op.deref(context).get_region(0), context);
        module
    }

    fn body_raw(&self, context: &Context) -> Option<Ptr<BasicBlock>> {
        let operation = self.get_operation().deref(context);
        if operation.num_regions() != 1 {
            return None;
        }
        let region = operation.get_region(0);
        drop(operation);
        let region = region.deref(context);
        let body = region.get_head()?;
        (region.get_tail() == Some(body)).then_some(body)
    }
}

impl Verify for MirProductionModuleOpV1 {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        if let Err(error) = verify_production_module_v1(self, context) {
            return verify_err!(self.get_operation().deref(context).loc(), error);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.production_function_v1",
    format,
    interfaces = [
        OneRegionInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        production_function_id: MirProductionFunctionIdAttrV1,
        production_entry_block_id: MirProductionBlockIdAttrV1
    )
)]
struct MirProductionFunctionOpV1;

#[op_interface_impl]
impl RegionKindInterface for MirProductionFunctionOpV1 {
    fn get_region_kind(&self, _index: usize) -> RegionKind {
        RegionKind::SSACFG
    }
}

impl MirProductionFunctionOpV1 {
    fn new(
        context: &mut Context,
        function_id: SemanticFunctionIdV1,
        entry_block_id: SemanticBlockIdV1,
    ) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            1,
        );
        let function = Self { op };
        function.set_attr_production_function_id(
            context,
            MirProductionFunctionIdAttrV1::new(function_id),
        );
        function.set_attr_production_entry_block_id(
            context,
            MirProductionBlockIdAttrV1::new(entry_block_id),
        );
        function
    }
}

impl Verify for MirProductionFunctionOpV1 {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let location = operation.deref(context).loc();
        let Some(parent) = operation
            .deref(context)
            .get_parent_op(context)
            .and_then(|parent| Operation::get_op::<MirProductionModuleOpV1>(parent, context))
        else {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "function is outside its production module"
                )
            );
        };
        let Some(limits) = parent.get_attr_production_module_tree_work_limit(context) else {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "module tree-work limit is missing"
                )
            );
        };
        if let Err(error) = verify_production_function_v1(self, limits.limits(), context) {
            return verify_err!(location, error);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.production_block_v1",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, NResultsInterface<0>],
    attributes = (
        production_block_function_id: MirProductionFunctionIdAttrV1,
        production_block_id: MirProductionBlockIdAttrV1,
        production_source_block_ordinal: MirProductionSourceBlockOrdinalAttrV1
    )
)]
struct MirProductionBlockOpV1;

impl MirProductionBlockOpV1 {
    fn new(
        context: &mut Context,
        function_id: SemanticFunctionIdV1,
        block_id: SemanticBlockIdV1,
        source_ordinal: u32,
    ) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let block = Self { op };
        block.set_attr_production_block_function_id(
            context,
            MirProductionFunctionIdAttrV1::new(function_id),
        );
        block.set_attr_production_block_id(context, MirProductionBlockIdAttrV1::new(block_id));
        block.set_attr_production_source_block_ordinal(
            context,
            MirProductionSourceBlockOrdinalAttrV1::new(source_ordinal),
        );
        block
    }
}

impl Verify for MirProductionBlockOpV1 {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let location = operation.deref(context).loc();
        if !production_has_exact_attribute_count(operation, context, 3)
            || self
                .get_attr_production_block_function_id(context)
                .is_none()
            || self.get_attr_production_block_id(context).is_none()
            || self
                .get_attr_production_source_block_ordinal(context)
                .is_none()
        {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "block locator attributes are missing"
                )
            );
        }
        let Some(parent_block) = operation.deref(context).get_parent_block() else {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator("block marker is detached")
            );
        };
        if !parent_block
            .deref(context)
            .get_parent_op(context)
            .is_some_and(|parent| Operation::is_op::<MirProductionFunctionOpV1>(parent, context))
            || operation.deref(context).get_prev().is_some()
        {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "block marker has non-canonical placement"
                )
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.production_statement_v1",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, NResultsInterface<0>],
    attributes = (
        production_statement_function_id: MirProductionFunctionIdAttrV1,
        production_statement_block_id: MirProductionBlockIdAttrV1,
        production_statement_ordinal: MirProductionStatementOrdinalAttrV1
    )
)]
struct MirProductionStatementOpV1;

impl MirProductionStatementOpV1 {
    fn new(
        context: &mut Context,
        function_id: SemanticFunctionIdV1,
        block_id: SemanticBlockIdV1,
        ordinal: u32,
    ) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let statement = Self { op };
        statement.set_attr_production_statement_function_id(
            context,
            MirProductionFunctionIdAttrV1::new(function_id),
        );
        statement.set_attr_production_statement_block_id(
            context,
            MirProductionBlockIdAttrV1::new(block_id),
        );
        statement.set_attr_production_statement_ordinal(
            context,
            MirProductionStatementOrdinalAttrV1::new(ordinal),
        );
        statement
    }
}

impl Verify for MirProductionStatementOpV1 {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let location = operation.deref(context).loc();
        if !production_has_exact_attribute_count(operation, context, 3)
            || self
                .get_attr_production_statement_function_id(context)
                .is_none()
            || self
                .get_attr_production_statement_block_id(context)
                .is_none()
            || self
                .get_attr_production_statement_ordinal(context)
                .is_none()
            || !production_leaf_is_in_function(operation, context)
        {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "statement locator is missing attributes or nesting"
                )
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "mir.production_terminator_v1",
    format,
    interfaces = [
        IsTerminatorInterface,
        NRegionsInterface<0>,
        NOpdsInterface<0>,
        NResultsInterface<0>
    ],
    attributes = (
        production_terminator_function_id: MirProductionFunctionIdAttrV1,
        production_terminator_block_id: MirProductionBlockIdAttrV1,
        production_terminator_successor_arcs: MirProductionSuccessorArcsAttrV1
    )
)]
struct MirProductionTerminatorOpV1;

impl MirProductionTerminatorOpV1 {
    fn new(
        context: &mut Context,
        function_id: SemanticFunctionIdV1,
        block_id: SemanticBlockIdV1,
        arcs: &[MirProductionSuccessorArcV1],
        successors: Vec<Ptr<BasicBlock>>,
    ) -> Self {
        let op = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            successors,
            0,
        );
        let terminator = Self { op };
        terminator.set_attr_production_terminator_function_id(
            context,
            MirProductionFunctionIdAttrV1::new(function_id),
        );
        terminator.set_attr_production_terminator_block_id(
            context,
            MirProductionBlockIdAttrV1::new(block_id),
        );
        terminator.set_attr_production_terminator_successor_arcs(
            context,
            MirProductionSuccessorArcsAttrV1::new(arcs),
        );
        terminator
    }
}

impl Verify for MirProductionTerminatorOpV1 {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let location = operation.deref(context).loc();
        if !production_has_exact_attribute_count(operation, context, 3)
            || self
                .get_attr_production_terminator_function_id(context)
                .is_none()
            || self
                .get_attr_production_terminator_block_id(context)
                .is_none()
            || self
                .get_attr_production_terminator_successor_arcs(context)
                .and_then(|arcs| arcs.arcs().map(|_| ()))
                .is_none()
            || !production_leaf_is_in_function(operation, context)
            || operation.deref(context).get_next().is_some()
        {
            return verify_err!(
                location,
                MirDialectVerifyError::InvalidProductionLocator(
                    "terminator locator is malformed or non-final"
                )
            );
        }
        Ok(())
    }
}

fn production_leaf_is_in_function(operation: Ptr<Operation>, context: &Context) -> bool {
    operation
        .deref(context)
        .get_parent_block()
        .and_then(|block| block.deref(context).get_parent_op(context))
        .is_some_and(|parent| Operation::is_op::<MirProductionFunctionOpV1>(parent, context))
}

fn production_has_exact_attribute_count(
    operation: Ptr<Operation>,
    context: &Context,
    expected: usize,
) -> bool {
    operation.deref(context).attributes.0.len() == expected
}

fn verify_production_module_v1(
    module: &MirProductionModuleOpV1,
    context: &Context,
) -> Result<(), MirDialectVerifyError> {
    if !production_has_exact_attribute_count(module.get_operation(), context, 2) {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "module has unknown or extra attributes",
        ));
    }
    if module
        .get_operation()
        .deref(context)
        .get_parent_block()
        .is_some()
    {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "production module must be a detached root",
        ));
    }
    let limits = module
        .get_attr_production_module_tree_work_limit(context)
        .ok_or(MirDialectVerifyError::InvalidProductionLocator(
            "module tree-work limit is missing",
        ))?
        .limits();
    MirProductionPlironLimitsV1::new(limits.max_tree_work).map_err(|_| {
        MirDialectVerifyError::InvalidProductionLocator("module tree-work limit is invalid")
    })?;
    module.get_attr_production_module_sha256(context).ok_or(
        MirDialectVerifyError::InvalidProductionLocator("module SHA-256 locator is missing"),
    )?;
    let body = module
        .body_raw(context)
        .ok_or(MirDialectVerifyError::InvalidProductionLocator(
            "module must have exactly one body block",
        ))?;
    if body.deref(context).get_num_arguments() != 0 {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "module body has block arguments",
        ));
    }
    let children = body.deref(context).iter(context).collect::<Vec<_>>();
    if children.is_empty() {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "module has no functions",
        ));
    }
    if children.len() as u64 > HARD_MAX_FUNCTIONS_V1 {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "module exceeds the semantic function schema cap",
        ));
    }
    let mut statement_count = 0_usize;
    let mut block_count = 0_usize;
    let mut tree_work = 2_u64;
    for (expected, child) in children.into_iter().enumerate() {
        let function = Operation::get_op::<MirProductionFunctionOpV1>(child, context).ok_or(
            MirDialectVerifyError::InvalidProductionLocator("module has an unknown child"),
        )?;
        let found = function
            .get_attr_production_function_id(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "function ID is missing",
            ))?
            .value()
            .index();
        if found != expected as u32 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "function IDs are duplicate, missing, or non-canonical",
            ));
        }
        let (function_blocks, function_statements, function_work) =
            verify_production_function_v1(&function, limits, context)?;
        block_count = block_count.checked_add(function_blocks).ok_or(
            MirDialectVerifyError::InvalidProductionLocator("module block count overflowed"),
        )?;
        if block_count as u64 > HARD_MAX_BLOCKS_V1 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "module exceeds the semantic block schema cap",
            ));
        }
        statement_count = statement_count.checked_add(function_statements).ok_or(
            MirDialectVerifyError::InvalidProductionLocator("module statement count overflowed"),
        )?;
        if statement_count as u64 > HARD_MAX_STATEMENTS_V1 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "module exceeds the semantic statement schema cap",
            ));
        }
        tree_work = tree_work.checked_add(function_work).ok_or(
            MirDialectVerifyError::InvalidProductionLocator("module tree work overflowed"),
        )?;
        if tree_work > limits.max_tree_work {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "module exceeds the production Pliron tree-work limit",
            ));
        }
    }
    Ok(())
}

fn verify_production_function_v1(
    function: &MirProductionFunctionOpV1,
    module_limits: MirProductionPlironLimitsV1,
    context: &Context,
) -> Result<(usize, usize, u64), MirDialectVerifyError> {
    if !production_has_exact_attribute_count(function.get_operation(), context, 2) {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function has unknown or extra attributes",
        ));
    }
    let function_id = function
        .get_attr_production_function_id(context)
        .ok_or(MirDialectVerifyError::InvalidProductionLocator(
            "function ID is missing",
        ))?
        .value();
    if u64::from(function_id.index()) >= HARD_MAX_FUNCTIONS_V1 {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function ID is out of range",
        ));
    }
    let entry = function
        .get_attr_production_entry_block_id(context)
        .ok_or(MirDialectVerifyError::InvalidProductionLocator(
            "entry block ID is missing",
        ))?
        .value();
    let operation = function.get_operation().deref(context);
    if operation.num_regions() != 1 {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function must have exactly one region",
        ));
    }
    let region = operation.get_region(0);
    drop(operation);
    let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
    if blocks.is_empty() {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function has no blocks",
        ));
    }
    if blocks.len() as u64 > HARD_MAX_BLOCKS_V1 {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function exceeds the semantic block schema cap",
        ));
    }
    if entry.index() as usize >= blocks.len() {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "entry block locator is dangling",
        ));
    }

    let mut by_id = vec![None; blocks.len()];
    let mut statement_count = 0_usize;
    let mut tree_work = 1_u64;
    for (position, block) in blocks.iter().copied().enumerate() {
        if block.deref(context).get_num_arguments() != 0 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "production blocks may not have arguments",
            ));
        }
        let operations = block.deref(context).iter(context).collect::<Vec<_>>();
        if operations.len() < 2 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "block requires a marker and one terminator",
            ));
        }
        let marker = Operation::get_op::<MirProductionBlockOpV1>(operations[0], context).ok_or(
            MirDialectVerifyError::InvalidProductionLocator("block marker is missing or unknown"),
        )?;
        if !production_has_exact_attribute_count(marker.get_operation(), context, 3) {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "block marker has unknown or extra attributes",
            ));
        }
        let marker_function = marker
            .get_attr_production_block_function_id(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "block function locator is missing",
            ))?
            .value();
        let block_id = marker
            .get_attr_production_block_id(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "block ID is missing",
            ))?
            .value();
        let source_ordinal = marker
            .get_attr_production_source_block_ordinal(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "source block ordinal is missing",
            ))?
            .value();
        let expected = canonical_production_block_id(position, entry.index());
        if marker_function != function_id
            || block_id.index() != expected
            || source_ordinal != block_id.index()
        {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "block IDs or source order are duplicate, missing, foreign, or non-canonical",
            ));
        }
        by_id[block_id.index() as usize] = Some(block);

        let statement_operations = &operations[1..operations.len() - 1];
        if statement_operations.len() as u64 > HARD_MAX_STATEMENTS_V1 {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "block exceeds the semantic statement schema cap",
            ));
        }
        for (expected_ordinal, operation) in statement_operations.iter().copied().enumerate() {
            let statement = Operation::get_op::<MirProductionStatementOpV1>(operation, context)
                .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                    "block contains an unknown or misplaced operation",
                ))?;
            if !production_has_exact_attribute_count(statement.get_operation(), context, 3) {
                return Err(MirDialectVerifyError::InvalidProductionLocator(
                    "statement has unknown or extra attributes",
                ));
            }
            let statement_function = statement
                .get_attr_production_statement_function_id(context)
                .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                    "statement function locator is missing",
                ))?
                .value();
            let statement_block = statement
                .get_attr_production_statement_block_id(context)
                .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                    "statement block locator is missing",
                ))?
                .value();
            let ordinal = statement
                .get_attr_production_statement_ordinal(context)
                .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                    "statement ordinal is missing",
                ))?
                .value();
            if statement_function != function_id
                || statement_block != block_id
                || ordinal != expected_ordinal as u32
            {
                return Err(MirDialectVerifyError::InvalidProductionLocator(
                    "statement locator is foreign, duplicate, missing, or non-canonical",
                ));
            }
        }
        statement_count = statement_count
            .checked_add(statement_operations.len())
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "function statement count overflowed",
            ))?;
        tree_work = tree_work
            .checked_add(3)
            .and_then(|work| work.checked_add(statement_operations.len() as u64))
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "function tree work overflowed",
            ))?;

        let terminator = Operation::get_op::<MirProductionTerminatorOpV1>(
            *operations.last().expect("length checked"),
            context,
        )
        .ok_or(MirDialectVerifyError::InvalidProductionLocator(
            "block terminator is missing or unknown",
        ))?;
        if !production_has_exact_attribute_count(terminator.get_operation(), context, 3) {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "terminator has unknown or extra attributes",
            ));
        }
        let terminator_function = terminator
            .get_attr_production_terminator_function_id(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "terminator function locator is missing",
            ))?
            .value();
        let terminator_block = terminator
            .get_attr_production_terminator_block_id(context)
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "terminator block locator is missing",
            ))?
            .value();
        if terminator_function != function_id || terminator_block != block_id {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "terminator locator is foreign or non-canonical",
            ));
        }
    }

    for block in blocks {
        let operations = block.deref(context).iter(context).collect::<Vec<_>>();
        let terminator = Operation::get_op::<MirProductionTerminatorOpV1>(
            *operations.last().expect("structure checked"),
            context,
        )
        .expect("structure checked");
        let arcs = terminator
            .get_attr_production_terminator_successor_arcs(context)
            .and_then(|arcs| arcs.arcs().map(<[_]>::to_vec))
            .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                "terminator arcs are missing, unknown, or oversized",
            ))?;
        let terminator_ref = terminator.get_operation().deref(context);
        if terminator_ref.get_num_successors() != arcs.len() {
            return Err(MirDialectVerifyError::InvalidProductionLocator(
                "terminator arc and successor counts differ",
            ));
        }
        for (actual, arc) in terminator_ref.successors().zip(arcs) {
            let expected = by_id
                .get(arc.target.index() as usize)
                .and_then(|target| *target)
                .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                    "terminator successor locator is dangling",
                ))?;
            if actual != expected {
                return Err(MirDialectVerifyError::InvalidProductionLocator(
                    "terminator successor pointers do not match ordered arcs",
                ));
            }
            tree_work =
                tree_work
                    .checked_add(1)
                    .ok_or(MirDialectVerifyError::InvalidProductionLocator(
                        "function tree work overflowed",
                    ))?;
        }
    }
    if tree_work > module_limits.max_tree_work {
        return Err(MirDialectVerifyError::InvalidProductionLocator(
            "function exceeds the production Pliron tree-work limit",
        ));
    }
    Ok((by_id.len(), statement_count, tree_work))
}

/// An opaque production-locator module capability bound to one Pliron context.
#[derive(Clone, Eq, PartialEq)]
pub struct MirProductionModuleHandleV1 {
    owner: ContextIdentity,
    pointer: Ptr<Operation>,
}

impl fmt::Debug for MirProductionModuleHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MirProductionModuleHandleV1")
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl MirProductionModuleHandleV1 {
    /// Materializes a closed locator graph after complete allocation-free preflight.
    pub fn try_new(
        context: &mut Context,
        locator: MirProductionModuleLocatorV1,
        limits: MirProductionPlironLimitsV1,
    ) -> Result<Self, MirProductionLocatorErrorV1> {
        preflight_production_locator_v1(&locator, limits)?;
        let owner = match catch_unwind(AssertUnwindSafe(|| ensure_context_identity(context))) {
            Ok(Ok(owner)) => owner,
            Ok(Err(error)) => return Err(MirProductionLocatorErrorV1::ContextIdentity(error)),
            Err(_) => return Err(MirProductionLocatorErrorV1::UpstreamPanicked),
        };
        catch_unwind(AssertUnwindSafe(|| {
            materialize_production_locator_v1(context, owner, &locator, limits)
        }))
        .unwrap_or(Err(MirProductionLocatorErrorV1::UpstreamPanicked))
    }

    /// Verifies owner, liveness, closed structure, and exact ordered arcs.
    pub fn verify(&self, context: &Context) -> Result<(), MirProductionLocatorErrorV1> {
        self.authenticate(context)?;
        let verified = catch_unwind(AssertUnwindSafe(|| {
            verify_operation(self.pointer, context).is_ok()
        }))
        .map_err(|_| MirProductionLocatorErrorV1::UpstreamPanicked)?;
        if verified {
            Ok(())
        } else {
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        }
    }

    /// Returns an exact pointer-independent snapshot after full verification.
    pub fn snapshot(
        &self,
        context: &Context,
    ) -> Result<MirProductionModuleLocatorV1, MirProductionLocatorErrorV1> {
        self.verify(context)?;
        catch_unwind(AssertUnwindSafe(|| {
            snapshot_production_locator_v1(self.pointer, context)
        }))
        .unwrap_or(Err(MirProductionLocatorErrorV1::UpstreamPanicked))
    }

    /// This inert locator graph grants no proof, publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    fn authenticate(&self, context: &Context) -> Result<(), MirProductionLocatorErrorV1> {
        let owner = require_context_identity(context)
            .map_err(MirProductionLocatorErrorV1::ContextIdentity)?;
        if owner != self.owner {
            return Err(MirProductionLocatorErrorV1::ForeignContext);
        }
        catch_unwind(AssertUnwindSafe(|| {
            let operation = self
                .pointer
                .try_deref(context)
                .map_err(|_| MirProductionLocatorErrorV1::StaleHandle)?;
            if !Operation::is_op::<MirProductionModuleOpV1>(self.pointer, context)
                || operation.get_parent_block().is_some()
            {
                return Err(MirProductionLocatorErrorV1::WrongKind);
            }
            Ok(())
        }))
        .unwrap_or(Err(MirProductionLocatorErrorV1::UpstreamPanicked))
    }
}

fn preflight_production_locator_v1(
    locator: &MirProductionModuleLocatorV1,
    limits: MirProductionPlironLimitsV1,
) -> Result<(), MirProductionLocatorErrorV1> {
    let mut tree_work = 2_u64;
    for function in &locator.functions {
        tree_work = tree_work.checked_add(1).ok_or(
            MirProductionLocatorErrorV1::MiddleEndResourceLimitExceeded {
                resource: MirProductionPlironResourceV1::TreeWork,
                actual: u64::MAX,
                limit: limits.max_tree_work,
            },
        )?;
        for block in &function.blocks {
            tree_work = tree_work
                .checked_add(3)
                .and_then(|work| work.checked_add(block.statements.len() as u64))
                .and_then(|work| work.checked_add(block.terminator.successors.len() as u64))
                .ok_or(
                    MirProductionLocatorErrorV1::MiddleEndResourceLimitExceeded {
                        resource: MirProductionPlironResourceV1::TreeWork,
                        actual: u64::MAX,
                        limit: limits.max_tree_work,
                    },
                )?;
            if tree_work > limits.max_tree_work {
                return Err(
                    MirProductionLocatorErrorV1::MiddleEndResourceLimitExceeded {
                        resource: MirProductionPlironResourceV1::TreeWork,
                        actual: tree_work,
                        limit: limits.max_tree_work,
                    },
                );
            }
        }
    }
    Ok(())
}

fn materialize_production_locator_v1(
    context: &mut Context,
    owner: ContextIdentity,
    locator: &MirProductionModuleLocatorV1,
    limits: MirProductionPlironLimitsV1,
) -> Result<MirProductionModuleHandleV1, MirProductionLocatorErrorV1> {
    let module = MirProductionModuleOpV1::new(context, locator.semantic_sha256, limits);
    let module_pointer = module.get_operation();
    let body = module
        .body_raw(context)
        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
    for function_locator in &locator.functions {
        let function = MirProductionFunctionOpV1::new(
            context,
            function_locator.function_id,
            function_locator.entry_block_id,
        );
        let function_pointer = function.get_operation();
        function_pointer.insert_at_back(body, context);
        let region = function_pointer.deref(context).get_region(0);
        let mut by_id = vec![None; function_locator.blocks.len()];
        for physical_position in 0..function_locator.blocks.len() {
            let source_ordinal = canonical_production_block_id(
                physical_position,
                function_locator.entry_block_id.index(),
            );
            let block_locator = &function_locator.blocks[source_ordinal as usize];
            let block = BasicBlock::new(
                context,
                Some(
                    format!("production_bb{}", block_locator.block_id.index())
                        .try_into()
                        .expect("valid generated label"),
                ),
                vec![],
            );
            block.insert_at_back(region, context);
            by_id[block_locator.block_id.index() as usize] = Some(block);
            MirProductionBlockOpV1::new(
                context,
                function_locator.function_id,
                block_locator.block_id,
                source_ordinal,
            )
            .get_operation()
            .insert_at_back(block, context);
            for statement in &block_locator.statements {
                MirProductionStatementOpV1::new(
                    context,
                    function_locator.function_id,
                    block_locator.block_id,
                    statement.ordinal,
                )
                .get_operation()
                .insert_at_back(block, context);
            }
        }
        for block_locator in &function_locator.blocks {
            let block = by_id[block_locator.block_id.index() as usize]
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
            let successors = block_locator
                .terminator
                .successors
                .iter()
                .map(|arc| {
                    by_id[arc.target.index() as usize]
                        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)
                })
                .collect::<Result<Vec<_>, _>>()?;
            MirProductionTerminatorOpV1::new(
                context,
                function_locator.function_id,
                block_locator.block_id,
                &block_locator.terminator.successors,
                successors,
            )
            .get_operation()
            .insert_at_back(block, context);
        }
    }
    if verify_operation(module_pointer, context).is_err() {
        Operation::erase(module_pointer, context);
        return Err(MirProductionLocatorErrorV1::VerificationFailed);
    }
    Ok(MirProductionModuleHandleV1 {
        owner,
        pointer: module_pointer,
    })
}

fn snapshot_production_locator_v1(
    pointer: Ptr<Operation>,
    context: &Context,
) -> Result<MirProductionModuleLocatorV1, MirProductionLocatorErrorV1> {
    let module = Operation::get_op::<MirProductionModuleOpV1>(pointer, context)
        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
    let semantic_sha256 = module
        .get_attr_production_module_sha256(context)
        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
        .value();
    let body = module
        .body_raw(context)
        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
    let function_operations = body.deref(context).iter(context).collect::<Vec<_>>();
    let mut functions = Vec::with_capacity(function_operations.len());
    for function_operation in function_operations {
        let function = Operation::get_op::<MirProductionFunctionOpV1>(function_operation, context)
            .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
        let function_id = function
            .get_attr_production_function_id(context)
            .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
            .value();
        let entry = function
            .get_attr_production_entry_block_id(context)
            .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
            .value();
        let region = function_operation.deref(context).get_region(0);
        let block_pointers = region.deref(context).iter(context).collect::<Vec<_>>();
        let mut blocks_by_source = vec![None; block_pointers.len()];
        for block in block_pointers {
            let operations = block.deref(context).iter(context).collect::<Vec<_>>();
            let marker = operations
                .first()
                .and_then(|operation| {
                    Operation::get_op::<MirProductionBlockOpV1>(*operation, context)
                })
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
            let block_id = marker
                .get_attr_production_block_id(context)
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
                .value();
            let source_ordinal = marker
                .get_attr_production_source_block_ordinal(context)
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
                .value();
            if source_ordinal as usize >= blocks_by_source.len()
                || source_ordinal != block_id.index()
                || blocks_by_source[source_ordinal as usize].is_some()
            {
                return Err(MirProductionLocatorErrorV1::MalformedSnapshot);
            }
            let mut statements = Vec::with_capacity(operations.len().saturating_sub(2));
            for operation in &operations[1..operations.len() - 1] {
                let statement =
                    Operation::get_op::<MirProductionStatementOpV1>(*operation, context)
                        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
                statements.push(MirProductionStatementLocatorV1::new(
                    statement
                        .get_attr_production_statement_ordinal(context)
                        .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?
                        .value(),
                ));
            }
            let terminator = operations
                .last()
                .and_then(|operation| {
                    Operation::get_op::<MirProductionTerminatorOpV1>(*operation, context)
                })
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
            let arcs = terminator
                .get_attr_production_terminator_successor_arcs(context)
                .and_then(|arcs| arcs.arcs().map(<[_]>::to_vec))
                .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
            blocks_by_source[source_ordinal as usize] = Some(MirProductionBlockLocatorV1::try_new(
                block_id,
                statements,
                MirProductionTerminatorLocatorV1::try_new(arcs)?,
            )?);
        }
        let blocks = blocks_by_source
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(MirProductionLocatorErrorV1::MalformedSnapshot)?;
        functions.push(MirProductionFunctionLocatorV1::try_new(
            function_id,
            entry,
            blocks,
        )?);
    }
    MirProductionModuleLocatorV1::try_new(semantic_sha256, functions)
}

/// Explicitly registers every D1 MIR entity. Repeated calls are idempotent.
pub fn register_mir_dialect(context: &mut Context) {
    let _ = catch_unwind(AssertUnwindSafe(|| ensure_context_identity(context)));
    MirTypeRef::register(context);
    MirIdentityAttr::register(context);
    MirLimitsAttr::register(context);
    MirBlockIdAttr::register(context);
    MirSemanticIdentityAttr::register(context);
    MirSemanticKindAttr::register(context);
    MirSemanticSpanAttr::register(context);
    MirSemanticOrdinalAttr::register(context);
    MirSemanticSuccessorsAttr::register(context);
    MirProductionSemanticSha256AttrV1::register(context);
    MirProductionFunctionIdAttrV1::register(context);
    MirProductionBlockIdAttrV1::register(context);
    MirProductionSourceBlockOrdinalAttrV1::register(context);
    MirProductionStatementOrdinalAttrV1::register(context);
    MirProductionSuccessorArcsAttrV1::register(context);
    MirProductionTreeWorkLimitAttrV1::register(context);
    MirModuleOp::register(context);
    MirFunctionOp::register(context);
    MirBlockOp::register(context);
    MirSemanticStatementOp::register(context);
    MirSemanticTerminatorOp::register(context);
    MirReturnOp::register(context);
    MirProductionModuleOpV1::register(context);
    MirProductionFunctionOpV1::register(context);
    MirProductionBlockOpV1::register(context);
    MirProductionStatementOpV1::register(context);
    MirProductionTerminatorOpV1::register(context);
}

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT)?;
    service.register_type::<MirTypeRef>()?;
    service.register_attribute::<MirIdentityAttr>()?;
    service.register_attribute::<MirLimitsAttr>()?;
    service.register_attribute::<MirBlockIdAttr>()?;
    service.register_attribute::<MirSemanticIdentityAttr>()?;
    service.register_attribute::<MirSemanticKindAttr>()?;
    service.register_attribute::<MirSemanticSpanAttr>()?;
    service.register_attribute::<MirSemanticOrdinalAttr>()?;
    service.register_attribute::<MirSemanticSuccessorsAttr>()?;
    service.register_attribute::<MirProductionSemanticSha256AttrV1>()?;
    service.register_attribute::<MirProductionFunctionIdAttrV1>()?;
    service.register_attribute::<MirProductionBlockIdAttrV1>()?;
    service.register_attribute::<MirProductionSourceBlockOrdinalAttrV1>()?;
    service.register_attribute::<MirProductionStatementOrdinalAttrV1>()?;
    service.register_attribute::<MirProductionSuccessorArcsAttrV1>()?;
    service.register_attribute::<MirProductionTreeWorkLimitAttrV1>()?;
    service.register_operation::<MirModuleOp>()?;
    service.register_operation::<MirFunctionOp>()?;
    service.register_operation::<MirBlockOp>()?;
    service.register_operation::<MirSemanticStatementOp>()?;
    service.register_operation::<MirSemanticTerminatorOp>()?;
    service.register_operation::<MirReturnOp>()?;
    service.register_operation::<MirProductionModuleOpV1>()?;
    service.register_operation::<MirProductionFunctionOpV1>()?;
    service.register_operation::<MirProductionBlockOpV1>()?;
    service.register_operation::<MirProductionStatementOpV1>()?;
    service.register_operation::<MirProductionTerminatorOpV1>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn mir_dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT, registration_hook)
}

#[cfg(test)]
mod direct_semantic_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn span(seed: u64) -> MirSemanticSourceSpan {
        MirSemanticSourceSpan::new([seed, seed + 1, seed + 2, seed + 3], 1, 1, 1, 2).unwrap()
    }

    fn production_block_for_direct_test(
        id: u32,
        statements: u32,
        arcs: Vec<MirProductionSuccessorArcV1>,
    ) -> MirProductionBlockLocatorV1 {
        MirProductionBlockLocatorV1::try_new(
            SemanticBlockIdV1::from_index(id),
            (0..statements)
                .map(MirProductionStatementLocatorV1::new)
                .collect(),
            MirProductionTerminatorLocatorV1::try_new(arcs).unwrap(),
        )
        .unwrap()
    }

    fn production_handle_for_direct_test(context: &mut Context) -> MirProductionModuleHandleV1 {
        register_mir_dialect(context);
        let function = MirProductionFunctionLocatorV1::try_new(
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(0),
            vec![
                production_block_for_direct_test(
                    0,
                    1,
                    vec![
                        MirProductionSuccessorArcV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        MirProductionSuccessorArcV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(0),
                        ),
                    ],
                ),
                production_block_for_direct_test(1, 0, vec![]),
            ],
        )
        .unwrap();
        let locator = MirProductionModuleLocatorV1::try_new(
            MirProductionSemanticSha256V1::from_sha256([0x5a; 32]),
            vec![function],
        )
        .unwrap();
        MirProductionModuleHandleV1::try_new(
            context,
            locator,
            MirProductionPlironLimitsV1::default(),
        )
        .unwrap()
    }

    fn direct_production_function(
        handle: &MirProductionModuleHandleV1,
        context: &Context,
    ) -> MirProductionFunctionOpV1 {
        let module = Operation::get_op::<MirProductionModuleOpV1>(handle.pointer, context).unwrap();
        let function = module
            .body_raw(context)
            .unwrap()
            .deref(context)
            .get_head()
            .unwrap();
        Operation::get_op::<MirProductionFunctionOpV1>(function, context).unwrap()
    }

    fn direct_production_block(
        function: &MirProductionFunctionOpV1,
        context: &Context,
        physical_position: usize,
    ) -> Ptr<BasicBlock> {
        function
            .get_region(context)
            .deref(context)
            .iter(context)
            .nth(physical_position)
            .unwrap()
    }

    #[test]
    fn direct_snapshot_never_consults_successor_text_parser() {
        SEMANTIC_SUCCESSOR_TEXT_PARSE_COUNT.store(0, Ordering::Relaxed);
        SEMANTIC_SUCCESSOR_TEXT_PRINT_COUNT.store(0, Ordering::Relaxed);

        let mut context = Context::new();
        register_mir_dialect(&mut context);
        let module =
            MirModuleOp::try_new(&mut context, "typed", MirDialectLimits::default()).unwrap();
        let function = module.append_function(&mut context, "kernel", &[]).unwrap();
        let entry = function.entry_block(&context).unwrap();
        let block1 = function.append_block(&mut context).unwrap();
        let block2 = function.append_block(&mut context).unwrap();
        entry
            .replace_with_semantic_terminator(
                &mut context,
                0,
                MirSemanticOperationKind::TerminatorSwitchInt,
                [1, 2, 3, 4],
                MirSemanticSpanProvenance::new(span(1), span(11)).unwrap(),
                &[block2.clone(), block1, block2],
            )
            .unwrap();

        let snapshot = module
            .body(&context)
            .unwrap()
            .semantic_functions(&context)
            .unwrap();
        let MirSnapshotOperation::SemanticTerminator(terminator) =
            &snapshot[0].blocks()[0].operations()[1]
        else {
            panic!("expected semantic terminator");
        };
        assert_eq!(terminator.successors(), &[2, 1, 2]);
        assert_eq!(
            SEMANTIC_SUCCESSOR_TEXT_PARSE_COUNT.load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            SEMANTIC_SUCCESSOR_TEXT_PRINT_COUNT.load(Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn production_arc_parser_rejects_unknown_and_noncanonical_locators() {
        let context = Context::new();
        for text in [
            "12:0", "01:0", "0:01", "0:0:1", "0:", ":0", "0:0,", "0:262144",
        ] {
            let attribute = MirProductionSuccessorArcsAttrV1::from_text(text);
            assert!(attribute.arcs().is_none(), "unexpectedly accepted {text}");
            assert!(attribute.verify(&context).is_err());
        }
        let duplicate = MirProductionSuccessorArcsAttrV1::from_text("1:7,1:7,2:0");
        assert_eq!(
            duplicate.arcs().unwrap(),
            &[
                MirProductionSuccessorArcV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(7),
                ),
                MirProductionSuccessorArcV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(7),
                ),
                MirProductionSuccessorArcV1::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    SemanticBlockIdV1::from_index(0),
                ),
            ]
        );
    }

    #[test]
    fn every_production_op_rejects_an_extra_attribute() {
        for target_kind in 0..5 {
            let mut context = Context::new();
            let handle = production_handle_for_direct_test(&mut context);
            let module =
                Operation::get_op::<MirProductionModuleOpV1>(handle.pointer, &context).unwrap();
            let function = direct_production_function(&handle, &context);
            let block = direct_production_block(&function, &context, 0);
            let operations = block.deref(&context).iter(&context).collect::<Vec<_>>();
            let marker =
                Operation::get_op::<MirProductionBlockOpV1>(operations[0], &context).unwrap();
            let statement =
                Operation::get_op::<MirProductionStatementOpV1>(operations[1], &context).unwrap();
            let terminator = Operation::get_op::<MirProductionTerminatorOpV1>(
                *operations.last().unwrap(),
                &context,
            )
            .unwrap();
            let target = match target_kind {
                0 => module.get_operation(),
                1 => function.get_operation(),
                2 => marker.get_operation(),
                3 => statement.get_operation(),
                4 => terminator.get_operation(),
                _ => unreachable!(),
            };
            target.deref_mut(&context).attributes.set(
                "hostile_extra".try_into().unwrap(),
                StringAttr::new("unknown".to_owned()),
            );
            let rejected = match target_kind {
                0 => module.verify(&context).is_err(),
                1 => function.verify(&context).is_err(),
                2 => marker.verify(&context).is_err(),
                3 => statement.verify(&context).is_err(),
                4 => terminator.verify(&context).is_err(),
                _ => unreachable!(),
            };
            assert!(
                rejected,
                "production op kind {target_kind} accepted an extra attribute"
            );
        }
    }

    #[test]
    fn production_verifier_rejects_mutated_ids_ordinals_and_source_order() {
        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        function.set_attr_production_function_id(
            &context,
            MirProductionFunctionIdAttrV1::new(SemanticFunctionIdV1::from_index(1)),
        );
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );

        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        let block = direct_production_block(&function, &context, 0);
        let statement = block.deref(&context).iter(&context).nth(1).unwrap();
        Operation::get_op::<MirProductionStatementOpV1>(statement, &context)
            .unwrap()
            .set_attr_production_statement_ordinal(
                &context,
                MirProductionStatementOrdinalAttrV1::new(7),
            );
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );

        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        let block = direct_production_block(&function, &context, 0);
        let marker = block.deref(&context).get_head().unwrap();
        Operation::get_op::<MirProductionBlockOpV1>(marker, &context)
            .unwrap()
            .set_attr_production_source_block_ordinal(
                &context,
                MirProductionSourceBlockOrdinalAttrV1::new(1),
            );
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );
    }

    #[test]
    fn production_verifier_rejects_dangling_reordered_and_unknown_structure() {
        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        let block = direct_production_block(&function, &context, 0);
        let terminator = block.deref(&context).get_tail().unwrap();
        Operation::get_op::<MirProductionTerminatorOpV1>(terminator, &context)
            .unwrap()
            .set_attr_production_terminator_successor_arcs(
                &context,
                MirProductionSuccessorArcsAttrV1::new(&[
                    MirProductionSuccessorArcV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(99),
                    ),
                    MirProductionSuccessorArcV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(0),
                    ),
                ]),
            );
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );

        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        let block = direct_production_block(&function, &context, 0);
        let terminator = block.deref(&context).get_tail().unwrap();
        Operation::get_op::<MirProductionTerminatorOpV1>(terminator, &context)
            .unwrap()
            .set_attr_production_terminator_successor_arcs(
                &context,
                MirProductionSuccessorArcsAttrV1::new(&[
                    MirProductionSuccessorArcV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(0),
                    ),
                    MirProductionSuccessorArcV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(1),
                    ),
                ]),
            );
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );

        let mut context = Context::new();
        let handle = production_handle_for_direct_test(&mut context);
        let function = direct_production_function(&handle, &context);
        let block = direct_production_block(&function, &context, 0);
        let terminator = block.deref(&context).get_tail().unwrap();
        MirReturnOp::new(&mut context)
            .get_operation()
            .insert_before(&context, terminator);
        assert_eq!(
            handle.verify(&context),
            Err(MirProductionLocatorErrorV1::VerificationFailed)
        );
    }
}
