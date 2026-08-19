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
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{op_interface_impl, pliron_attr, pliron_op, pliron_type},
    linked_list::{ContainsLinkedList, LinkedList},
    location::Located,
    op::Op,
    operation::{Operation, verify_operation},
    result::Result as PlironResult,
    r#type::{Type, TypeHandle, Typed, TypedHandle},
    verify_err, verify_err_noloc,
};
use fe2o3_mir_model::{
    MAX_EXECUTABLE_BLOCK_PARAMETERS, MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_FUNCTIONS,
    MAX_EXECUTABLE_IDENTITY_BYTES, MAX_EXECUTABLE_TYPES, MirBlockId, MirTypeId,
};
use fe2o3_pliron::{
    ContextIdentity, ContextIdentityError, DialectRegistration, DialectRegistrationService,
    NameError, RegistrationHookError, ensure_context_identity, require_context_identity,
};

use crate::DIALECT;

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

/// Pointer-independent operation evidence from one verified MIR CFG block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSnapshotOperation {
    /// Canonical block marker carrying its stable MIR block identifier.
    BlockMarker(MirBlockId),
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
}

impl fmt::Display for MirDialectVerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAttribute(name) => write!(formatter, "missing MIR attribute `{name}`"),
            Self::InvalidIdentity => formatter.write_str("invalid MIR identity attribute"),
            Self::InvalidLimits => formatter.write_str("invalid MIR limits attribute"),
            Self::InvalidTypeId(id) => write!(formatter, "invalid MIR type id {id}"),
            Self::InvalidBlockId(id) => write!(formatter, "invalid MIR block id {id}"),
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

        for (index, block) in blocks.into_iter().enumerate() {
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

/// Explicitly registers every D1 MIR entity. Repeated calls are idempotent.
pub fn register_mir_dialect(context: &mut Context) {
    let _ = catch_unwind(AssertUnwindSafe(|| ensure_context_identity(context)));
    MirTypeRef::register(context);
    MirIdentityAttr::register(context);
    MirLimitsAttr::register(context);
    MirBlockIdAttr::register(context);
    MirModuleOp::register(context);
    MirFunctionOp::register(context);
    MirBlockOp::register(context);
    MirReturnOp::register(context);
}

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT)?;
    service.register_type::<MirTypeRef>()?;
    service.register_attribute::<MirIdentityAttr>()?;
    service.register_attribute::<MirLimitsAttr>()?;
    service.register_attribute::<MirBlockIdAttr>()?;
    service.register_operation::<MirModuleOp>()?;
    service.register_operation::<MirFunctionOp>()?;
    service.register_operation::<MirBlockOp>()?;
    service.register_operation::<MirReturnOp>()?;
    Ok(())
}

/// Returns the explicit MIR registration consumed by [`fe2o3_pliron`].
pub fn mir_dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT, registration_hook)
}
