#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU64, Ordering},
};

use pliron::{
    attribute::Attribute,
    builtin::ops::ModuleOp,
    context::Context,
    context::Ptr,
    dialect::{Dialect, DialectName},
    identifier::Identifier,
    location::Location,
    op::{Op, OpBox},
    operation::Operation,
    parsable::Parsable,
    pass::Pass,
    r#type::{Type, TypedHandle},
    uniqued_any::{self, UniquedKey},
};

/// The only accepted Pliron workspace revision for Wave 0.
pub const PLIRON_REVISION: &str = "2610651306ea3ba670f68d5d8b1e1159bcd521ed";

/// Hard implementation caps that configuration cannot exceed.
pub const HARD_MAX_DIALECTS: usize = 64;
pub const HARD_MAX_PASSES: usize = 256;
pub const HARD_MAX_NAME_BYTES: usize = 96;
pub const HARD_MAX_DIAGNOSTIC_BYTES: usize = 4_096;
pub const HARD_MAX_DIALECT_REGISTRATION_ACTIONS: usize = 64;

/// Auxiliary-data key for the fe2o3 context-identity locator.
pub const CONTEXT_IDENTITY_MARKER_KEY: &str = "fe2o3_pliron_context_identity_v1";

static NEXT_CONTEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Opaque process-local identity for one Pliron context.
///
/// The value is descriptive provenance for in-memory handles. It is not a
/// durable compiler, artifact, proof, publication, or runtime identity.
/// Its representation cannot be constructed or recovered by callers:
///
/// ```compile_fail
/// use std::num::NonZeroU64;
/// use fe2o3_pliron::ContextIdentity;
///
/// let forged = ContextIdentity(NonZeroU64::new(1).unwrap());
/// ```
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ContextIdentity(NonZeroU64);

impl fmt::Debug for ContextIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ContextIdentity(<process-local>)")
    }
}

#[derive(Clone, Copy, Debug)]
struct ContextIdentityAnchor(ContextIdentity);

impl PartialEq for ContextIdentityAnchor {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for ContextIdentityAnchor {}

impl Hash for ContextIdentityAnchor {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

#[derive(Debug)]
struct ContextIdentityMarker {
    anchor: UniquedKey<ContextIdentityAnchor>,
    identity: ContextIdentity,
}

/// Failure to create or validate a context-bound identity anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextIdentityError {
    /// Another typed value claimed the public locator key.
    MarkerCollision,
    /// The locator is missing its private, context-owned anchor.
    CorruptMarker,
    /// The process exhausted the context-identity counter.
    IdentitySpaceExhausted,
}

impl fmt::Display for ContextIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerCollision => {
                formatter.write_str("Pliron context identity marker collision")
            }
            Self::CorruptMarker => formatter.write_str("Pliron context identity marker is corrupt"),
            Self::IdentitySpaceExhausted => {
                formatter.write_str("Pliron context identity space is exhausted")
            }
        }
    }
}

impl Error for ContextIdentityError {}

/// Returns this context's identity, creating its private anchor when absent.
///
/// The authoritative anchor is stored in Pliron's private uniqued store. The
/// public auxiliary-data marker is only a locator, so moving that marker to a
/// different context does not transfer the identity.
pub fn ensure_context_identity(
    context: &mut Context,
) -> Result<ContextIdentity, ContextIdentityError> {
    if let Some(identity) = context_identity_state(context)? {
        return Ok(identity);
    }

    let proposed_identity = ContextIdentity(next_context_identity()?);
    let (anchor, identity) = catch_unwind(AssertUnwindSafe(|| {
        let anchor = uniqued_any::save(context, ContextIdentityAnchor(proposed_identity));
        let identity = uniqued_any::get(context, anchor).0;
        (anchor, identity)
    }))
    .map_err(|_| ContextIdentityError::CorruptMarker)?;
    let marker = context
        .aux_data
        .insert(Box::new(ContextIdentityMarker { anchor, identity }));
    context
        .aux_data_map
        .insert(context_identity_marker_key()?, marker);
    Ok(identity)
}

/// Returns a previously created context identity without creating one.
pub fn require_context_identity(
    context: &Context,
) -> Result<ContextIdentity, ContextIdentityError> {
    context_identity_state(context)?.ok_or(ContextIdentityError::CorruptMarker)
}

fn context_identity_state(
    context: &Context,
) -> Result<Option<ContextIdentity>, ContextIdentityError> {
    let marker_key = context_identity_marker_key()?;
    let Some(index) = context.aux_data_map.get(&marker_key).copied() else {
        return Ok(None);
    };
    let Some(marker) = context.aux_data.get(index) else {
        return Err(ContextIdentityError::CorruptMarker);
    };
    let marker = marker
        .downcast_ref::<ContextIdentityMarker>()
        .ok_or(ContextIdentityError::MarkerCollision)?;
    let identity = catch_unwind(AssertUnwindSafe(|| {
        uniqued_any::get(context, marker.anchor).0
    }))
    .map_err(|_| ContextIdentityError::CorruptMarker)?;
    if identity != marker.identity {
        return Err(ContextIdentityError::CorruptMarker);
    }
    Ok(Some(identity))
}

fn next_context_identity() -> Result<NonZeroU64, ContextIdentityError> {
    let value = NEXT_CONTEXT_IDENTITY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ContextIdentityError::IdentitySpaceExhausted)?;
    NonZeroU64::new(value).ok_or(ContextIdentityError::IdentitySpaceExhausted)
}

fn context_identity_marker_key() -> Result<Identifier, ContextIdentityError> {
    CONTEXT_IDENTITY_MARKER_KEY
        .try_into()
        .map_err(|_| ContextIdentityError::CorruptMarker)
}

/// Resource limits for one context and pass plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellLimits {
    max_dialects: usize,
    max_passes: usize,
    max_diagnostic_bytes: usize,
}

impl ShellLimits {
    /// Creates non-zero limits bounded by the implementation hard caps.
    pub fn new(
        max_dialects: usize,
        max_passes: usize,
        max_diagnostic_bytes: usize,
    ) -> Result<Self, LimitError> {
        validate_limit(max_dialects, HARD_MAX_DIALECTS, LimitKind::Dialects)?;
        validate_limit(max_passes, HARD_MAX_PASSES, LimitKind::Passes)?;
        validate_limit(
            max_diagnostic_bytes,
            HARD_MAX_DIAGNOSTIC_BYTES,
            LimitKind::DiagnosticBytes,
        )?;
        Ok(Self {
            max_dialects,
            max_passes,
            max_diagnostic_bytes,
        })
    }

    pub const fn max_dialects(self) -> usize {
        self.max_dialects
    }

    pub const fn max_passes(self) -> usize {
        self.max_passes
    }

    pub const fn max_diagnostic_bytes(self) -> usize {
        self.max_diagnostic_bytes
    }
}

impl Default for ShellLimits {
    fn default() -> Self {
        Self {
            max_dialects: 32,
            max_passes: 64,
            max_diagnostic_bytes: 512,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitKind {
    Dialects,
    Passes,
    DiagnosticBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitError {
    Zero(LimitKind),
    AboveHardCap(LimitKind),
}

fn validate_limit(value: usize, hard_cap: usize, kind: LimitKind) -> Result<(), LimitError> {
    if value == 0 {
        return Err(LimitError::Zero(kind));
    }
    if value > hard_cap {
        return Err(LimitError::AboveHardCap(kind));
    }
    Ok(())
}

/// Stable diagnostic categories owned by fe2o3.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticCode {
    DialectHookFailed,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DialectHookFailed => "FE2O3-PLIRON-DIALECT-HOOK-FAILED",
        }
    }
}

/// A stable, byte-bounded diagnostic. It contains no Pliron presentation text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    stage: Option<String>,
    message: String,
    truncated: bool,
}

impl Diagnostic {
    fn new(
        code: DiagnosticCode,
        stage: Option<&str>,
        message: &str,
        max_message_bytes: usize,
    ) -> Self {
        let (message, truncated) = truncate_utf8(message, max_message_bytes);
        Self {
            code,
            stage: stage.map(str::to_owned),
            message,
            truncated,
        }
    }

    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    pub fn stage(&self) -> Option<&str> {
        self.stage.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn was_truncated(&self) -> bool {
        self.truncated
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_owned(), false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

/// Why a bounded dialect or pass name was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameError {
    Empty,
    TooLong,
    InvalidFirstByte,
    InvalidByte,
}

fn validate_name(value: &str, kind: NameKind) -> Result<(), NameError> {
    if value.is_empty() {
        return Err(NameError::Empty);
    }
    if value.len() > HARD_MAX_NAME_BYTES {
        return Err(NameError::TooLong);
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(NameError::Empty);
    };
    if !first.is_ascii_lowercase() {
        return Err(NameError::InvalidFirstByte);
    }
    for byte in bytes {
        let common = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_';
        let pass_only = kind == NameKind::Pass && matches!(byte, b'.' | b'-');
        if !common && !pass_only {
            return Err(NameError::InvalidByte);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum NameKind {
    Dialect,
    Pass,
}

/// An error returned by a dialect's explicit registration hook.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationHookError {
    detail: String,
}

impl RegistrationHookError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl fmt::Display for RegistrationHookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for RegistrationHookError {}

/// A bounded registration capability borrowed from one context construction.
///
/// The service cannot be constructed or disassembled outside this crate. It
/// exposes no context, dialect object, arena pointer, generic callback, or
/// caller-provided state. Its borrow cannot outlive the registration hook:
///
/// ```compile_fail
/// use fe2o3_pliron::DialectRegistrationService;
/// use pliron::context::Context;
///
/// fn context(service: &mut DialectRegistrationService<'_>) -> &mut Context {
///     service.context
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron::DialectRegistrationService;
///
/// fn retain(
///     service: &mut DialectRegistrationService<'_>,
/// ) -> &'static mut DialectRegistrationService<'static> {
///     service
/// }
/// ```
pub struct DialectRegistrationService<'context> {
    context: &'context mut Context,
    dialect_name: &'context DialectName,
    actions: usize,
}

impl<'context> DialectRegistrationService<'context> {
    fn new(context: &'context mut Context, dialect_name: &'context DialectName) -> Self {
        Self {
            context,
            dialect_name,
            actions: 0,
        }
    }

    /// Rejects a hook that was attached to a different dialect name.
    pub fn require_dialect(&self, expected: &str) -> Result<(), RegistrationHookError> {
        if self.dialect_name.as_ref() == expected {
            Ok(())
        } else {
            Err(RegistrationHookError::new(
                "dialect registration hook name mismatch",
            ))
        }
    }

    /// Registers one type owned by this dialect.
    pub fn register_type<T>(&mut self) -> Result<(), RegistrationHookError>
    where
        T: Type + Parsable<Arg = (), Parsed = TypedHandle<T>>,
    {
        self.claim_action(&T::get_type_id_static().dialect)?;
        T::register(self.context);
        Ok(())
    }

    /// Registers one attribute owned by this dialect.
    pub fn register_attribute<A>(&mut self) -> Result<(), RegistrationHookError>
    where
        A: Attribute + Parsable<Arg = (), Parsed = A>,
    {
        self.claim_action(&A::get_attr_id_static().dialect)?;
        <A as Attribute>::register::<A>(self.context);
        Ok(())
    }

    /// Registers one operation owned by this dialect.
    pub fn register_operation<O>(&mut self) -> Result<(), RegistrationHookError>
    where
        O: Op + Parsable<Arg = Vec<(Identifier, Location)>, Parsed = OpBox>,
    {
        self.claim_action(&O::get_opid_static().dialect)?;
        O::register(self.context);
        Ok(())
    }

    fn claim_action(&mut self, entity_dialect: &DialectName) -> Result<(), RegistrationHookError> {
        if entity_dialect != self.dialect_name {
            return Err(RegistrationHookError::new(
                "registered entity belongs to a different dialect",
            ));
        }
        if self.actions == HARD_MAX_DIALECT_REGISTRATION_ACTIONS {
            return Err(RegistrationHookError::new(
                "dialect registration action limit exceeded",
            ));
        }
        self.actions += 1;
        Ok(())
    }
}

/// Registers typed entities through a context-custody service.
pub type DialectRegistrationHook = for<'context> fn(
    &mut DialectRegistrationService<'context>,
) -> Result<(), RegistrationHookError>;

/// One explicitly named dialect registration.
#[derive(Clone)]
pub struct DialectRegistration {
    name: String,
    hook: DialectRegistrationHook,
}

impl DialectRegistration {
    pub fn new(name: &str, hook: DialectRegistrationHook) -> Result<Self, NameError> {
        validate_name(name, NameKind::Dialect)?;
        Ok(Self {
            name: name.to_owned(),
            hook,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextBuildError {
    TooManyDialects,
    RegistrationInputPanicked,
    DuplicateDialect(String),
    UpstreamRejectedDialect(String),
    ContextIdentity(ContextIdentityError),
    RegistrationFailed(Diagnostic),
}

/// Deterministic fe2o3 metadata about a fresh context.
///
/// This is descriptive metadata, not an artifact or cache identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextManifest {
    pliron_revision: &'static str,
    registration_order: Vec<String>,
}

impl ContextManifest {
    pub const fn pliron_revision(&self) -> &'static str {
        self.pliron_revision
    }

    pub fn registration_order(&self) -> &[String] {
        &self.registration_order
    }
}

/// A real Pliron context behind a fail-closed fe2o3 session boundary.
///
/// The owning context is unavailable through the production API:
///
/// ```compile_fail
/// use fe2o3_pliron::PlironSession;
/// use pliron::context::Context;
///
/// fn context(session: &mut PlironSession) -> &mut Context {
///     &mut session.context
/// }
/// ```
pub struct PlironSession {
    context: Context,
    identity: ContextIdentity,
    manifest: ContextManifest,
    operations: BTreeMap<OperationHandleIdentity, Ptr<Operation>>,
    next_operation_handle: Option<NonZeroU64>,
    poisoned: bool,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct OperationHandleIdentity(NonZeroU64);

/// An opaque operation capability owned by one [`PlironSession`].
///
/// The upstream pointer and its owner identity are intentionally private. A
/// handle can only be dereferenced by session methods that authenticate both.
/// The handle stores no pointer at all, so callers cannot extract one:
///
/// ```compile_fail
/// use fe2o3_pliron::OperationHandle;
/// use pliron::{context::Ptr, operation::Operation};
///
/// fn pointer(handle: &OperationHandle) -> Ptr<Operation> {
///     handle.pointer
/// }
/// ```
#[derive(Clone)]
pub struct OperationHandle {
    owner: ContextIdentity,
    identity: OperationHandleIdentity,
}

impl fmt::Debug for OperationHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationHandle")
            .finish_non_exhaustive()
    }
}

/// Bounded failures from an owner-aware operation handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationHandleError {
    SessionPoisoned,
    InvalidName(NameError),
    ContextIdentity(ContextIdentityError),
    ForeignSession,
    StaleHandle,
    HandleSpaceExhausted,
    UpstreamPanicked,
}

impl fmt::Display for OperationHandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionPoisoned => formatter.write_str("Pliron session is poisoned"),
            Self::InvalidName(_) => formatter.write_str("invalid operation name"),
            Self::ContextIdentity(_) => {
                formatter.write_str("Pliron context identity validation failed")
            }
            Self::ForeignSession => {
                formatter.write_str("operation handle belongs to another session")
            }
            Self::StaleHandle => formatter.write_str("operation handle is stale"),
            Self::HandleSpaceExhausted => {
                formatter.write_str("operation handle identity space is exhausted")
            }
            Self::UpstreamPanicked => formatter.write_str("Pliron operation access panicked"),
        }
    }
}

impl Error for OperationHandleError {}

impl PlironSession {
    /// Builds a fresh context after preflighting every registration.
    pub fn new(
        limits: ShellLimits,
        registrations: impl IntoIterator<Item = DialectRegistration>,
    ) -> Result<Self, ContextBuildError> {
        let mut registration_iter = catch_unwind(AssertUnwindSafe(|| registrations.into_iter()))
            .map_err(|_| ContextBuildError::RegistrationInputPanicked)?;
        let mut registrations = Vec::with_capacity(limits.max_dialects);
        loop {
            let registration = catch_unwind(AssertUnwindSafe(|| registration_iter.next()))
                .map_err(|_| ContextBuildError::RegistrationInputPanicked)?;
            let Some(registration) = registration else {
                break;
            };
            if registrations.len() == limits.max_dialects {
                return Err(ContextBuildError::TooManyDialects);
            }
            registrations.push(registration);
        }

        let mut seen = BTreeSet::new();
        for registration in &registrations {
            if !seen.insert(registration.name.clone()) {
                return Err(ContextBuildError::DuplicateDialect(
                    registration.name.clone(),
                ));
            }
        }

        let mut context = Context::new();
        let identity = catch_unwind(AssertUnwindSafe(|| ensure_context_identity(&mut context)))
            .map_err(|_| ContextBuildError::ContextIdentity(ContextIdentityError::CorruptMarker))?
            .map_err(ContextBuildError::ContextIdentity)?;
        for registration in &registrations {
            let dialect_name = DialectName::try_new(&registration.name).map_err(|_| {
                ContextBuildError::UpstreamRejectedDialect(registration.name.clone())
            })?;
            let hook_result = catch_unwind(AssertUnwindSafe(|| {
                Dialect::register(&mut context, &dialect_name);
                let mut service = DialectRegistrationService::new(&mut context, &dialect_name);
                (registration.hook)(&mut service)
            }));
            if !matches!(hook_result, Ok(Ok(()))) {
                return Err(ContextBuildError::RegistrationFailed(Diagnostic::new(
                    DiagnosticCode::DialectHookFailed,
                    Some(&registration.name),
                    "the explicit dialect registration hook failed",
                    limits.max_diagnostic_bytes,
                )));
            }
        }
        if require_context_identity(&context) != Ok(identity) {
            return Err(ContextBuildError::ContextIdentity(
                ContextIdentityError::CorruptMarker,
            ));
        }

        Ok(Self {
            context,
            identity,
            manifest: ContextManifest {
                pliron_revision: PLIRON_REVISION,
                registration_order: registrations
                    .into_iter()
                    .map(|registration| registration.name)
                    .collect(),
            },
            operations: BTreeMap::new(),
            next_operation_handle: NonZeroU64::new(1),
            poisoned: false,
        })
    }

    pub const fn manifest(&self) -> &ContextManifest {
        &self.manifest
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Grants raw context access only to compiler-internal conformance tests.
    ///
    /// Production crates must use owner-aware handles. This test seam remains
    /// feature-gated until all dialect builders operate through session-owned
    /// services and is never enabled by the default feature set.
    #[cfg(feature = "internal-test-context-access")]
    pub fn with_context_mut<T>(
        &mut self,
        action: impl FnOnce(&mut Context) -> T,
    ) -> Result<T, SessionPoisoned> {
        if self.poisoned {
            return Err(SessionPoisoned);
        }
        match catch_unwind(AssertUnwindSafe(|| action(&mut self.context))) {
            Ok(result) => Ok(result),
            Err(_) => {
                self.poisoned = true;
                Err(SessionPoisoned)
            }
        }
    }

    /// Creates an empty builtin module and returns only its owner-aware handle.
    pub fn create_module(&mut self, name: &str) -> Result<OperationHandle, OperationHandleError> {
        validate_name(name, NameKind::Dialect).map_err(OperationHandleError::InvalidName)?;
        self.validate_identity()?;
        let identity = self.allocate_operation_handle()?;
        let name = Identifier::try_from(name)
            .map_err(|_| OperationHandleError::InvalidName(NameError::InvalidByte))?;
        let pointer = match catch_unwind(AssertUnwindSafe(|| {
            ModuleOp::new(&mut self.context, name).get_operation()
        })) {
            Ok(pointer) => pointer,
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        };
        self.operations.insert(identity, pointer);
        Ok(OperationHandle {
            owner: self.identity,
            identity,
        })
    }

    /// Returns the operation's result count after authenticating its owner.
    pub fn operation_result_count(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<usize, OperationHandleError> {
        self.with_operation(handle, |pointer, context| {
            pointer.deref(context).get_num_results()
        })
    }

    /// Erases an authenticated operation, invalidating all clones of its handle.
    pub fn erase_operation(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<(), OperationHandleError> {
        self.with_operation(handle, Operation::erase)?;
        self.operations.remove(&handle.identity);
        Ok(())
    }

    fn validate_identity(&self) -> Result<(), OperationHandleError> {
        if self.poisoned {
            return Err(OperationHandleError::SessionPoisoned);
        }
        let current = require_context_identity(&self.context)
            .map_err(OperationHandleError::ContextIdentity)?;
        if current != self.identity {
            return Err(OperationHandleError::ContextIdentity(
                ContextIdentityError::CorruptMarker,
            ));
        }
        Ok(())
    }

    fn with_operation<T>(
        &mut self,
        handle: &OperationHandle,
        action: impl FnOnce(Ptr<Operation>, &mut Context) -> T,
    ) -> Result<T, OperationHandleError> {
        self.validate_identity()?;
        if handle.owner != self.identity {
            return Err(OperationHandleError::ForeignSession);
        }
        let Some(pointer) = self.operations.get(&handle.identity).copied() else {
            return Err(OperationHandleError::StaleHandle);
        };
        match catch_unwind(AssertUnwindSafe(|| {
            pointer.try_deref(&self.context).map(drop)
        })) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                self.operations.remove(&handle.identity);
                return Err(OperationHandleError::StaleHandle);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        }
        match catch_unwind(AssertUnwindSafe(|| action(pointer, &mut self.context))) {
            Ok(result) => Ok(result),
            Err(_) => {
                self.poisoned = true;
                Err(OperationHandleError::UpstreamPanicked)
            }
        }
    }

    fn allocate_operation_handle(
        &mut self,
    ) -> Result<OperationHandleIdentity, OperationHandleError> {
        let identity = self
            .next_operation_handle
            .take()
            .map(OperationHandleIdentity)
            .ok_or(OperationHandleError::HandleSpaceExhausted)?;
        self.next_operation_handle = identity.0.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(identity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPoisoned;

/// Why adding a pass to a deterministic plan failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassPlanError {
    PlanPoisoned,
    PassInspectionPanicked,
    InvalidName(NameError),
    DuplicatePass(String),
    NestedPassManagerUnsupported(String),
    TooManyPasses,
}

struct PlannedPass {
    name: String,
    _pass: Box<dyn Pass>,
}

/// A bounded pass plan. Insertion order is preserved as plan metadata.
///
/// This boundary intentionally exposes no generic execution method. Upstream
/// Pliron operation pointers do not carry context provenance. Session-owned
/// roots now do, but executing an arbitrary upstream [`Pass`] would hand that
/// raw pointer and the owning context to caller-supplied code again. Execution
/// therefore remains absent until compiler passes migrate to a sealed
/// owner-aware service:
///
/// ```compile_fail
/// use fe2o3_pliron::{OperationHandle, PassPlan, PlironSession};
///
/// fn execute(plan: &mut PassPlan, session: &mut PlironSession, root: &OperationHandle) {
///     plan.run(session, root);
/// }
/// ```
pub struct PassPlan {
    limits: ShellLimits,
    passes: Vec<PlannedPass>,
    names: BTreeSet<String>,
    poisoned: bool,
}

impl PassPlan {
    pub fn new(limits: ShellLimits) -> Self {
        Self {
            limits,
            passes: Vec::new(),
            names: BTreeSet::new(),
            poisoned: false,
        }
    }

    pub fn add_pass(&mut self, mut pass: impl Pass + 'static) -> Result<(), PassPlanError> {
        if self.poisoned {
            return Err(PassPlanError::PlanPoisoned);
        }
        if self.passes.len() >= self.limits.max_passes {
            return Err(PassPlanError::TooManyPasses);
        }
        let inspection = catch_unwind(AssertUnwindSafe(|| {
            let name = pass.name().to_owned();
            let nested = pass.as_pass_manager().is_some();
            (name, nested)
        }));
        let (name, nested) = match inspection {
            Ok(inspection) => inspection,
            Err(_) => {
                self.poisoned = true;
                return Err(PassPlanError::PassInspectionPanicked);
            }
        };
        validate_name(&name, NameKind::Pass).map_err(PassPlanError::InvalidName)?;
        if nested {
            return Err(PassPlanError::NestedPassManagerUnsupported(name));
        }
        if !self.names.insert(name.clone()) {
            return Err(PassPlanError::DuplicatePass(name));
        }
        self.passes.push(PlannedPass {
            name,
            _pass: Box::new(pass),
        });
        Ok(())
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn pass_order(&self) -> impl ExactSizeIterator<Item = &str> {
        self.passes.iter().map(|pass| pass.name.as_str())
    }
}

#[cfg(test)]
mod owner_handle_tests {
    use super::*;

    fn session() -> PlironSession {
        PlironSession::new(ShellLimits::default(), []).expect("fresh session")
    }

    #[test]
    fn equal_upstream_slots_do_not_transfer_handle_ownership() {
        let mut owner = session();
        let mut foreign = session();
        let owner_handle = owner.create_module("owner").expect("owner module");
        let foreign_handle = foreign.create_module("foreign").expect("foreign module");

        assert_eq!(
            owner.operations[&owner_handle.identity],
            foreign.operations[&foreign_handle.identity]
        );
        assert_eq!(
            foreign.operation_result_count(&owner_handle),
            Err(OperationHandleError::ForeignSession)
        );
    }

    #[test]
    fn transplanted_marker_is_rejected_before_pointer_access() {
        let mut owner = session();
        let mut foreign = session();
        let foreign_handle = foreign.create_module("foreign").expect("foreign module");
        let key = context_identity_marker_key().expect("fixed marker key");
        let owner_index = owner
            .context
            .aux_data_map
            .remove(&key)
            .expect("owner marker index");
        let owner_marker = owner
            .context
            .aux_data
            .remove(owner_index)
            .expect("owner marker");
        let foreign_index = foreign
            .context
            .aux_data_map
            .remove(&key)
            .expect("foreign marker index");
        foreign.context.aux_data.remove(foreign_index);
        let transplanted = foreign.context.aux_data.insert(owner_marker);
        foreign.context.aux_data_map.insert(key, transplanted);

        assert_eq!(
            foreign.operation_result_count(&foreign_handle),
            Err(OperationHandleError::ContextIdentity(
                ContextIdentityError::CorruptMarker
            ))
        );
    }

    #[test]
    fn missing_and_colliding_markers_are_rejected_before_pointer_access() {
        let mut missing = session();
        let missing_handle = missing.create_module("owner").expect("owner module");
        let key = context_identity_marker_key().expect("fixed marker key");
        missing.context.aux_data_map.remove(&key);
        assert_eq!(
            missing.operation_result_count(&missing_handle),
            Err(OperationHandleError::ContextIdentity(
                ContextIdentityError::CorruptMarker
            ))
        );

        let mut collision = session();
        let collision_handle = collision.create_module("owner").expect("owner module");
        let marker = collision
            .context
            .aux_data_map
            .remove(&key)
            .expect("marker index");
        collision.context.aux_data.remove(marker);
        let foreign_type = collision.context.aux_data.insert(Box::new(9_u32));
        collision.context.aux_data_map.insert(key, foreign_type);
        assert_eq!(
            collision.operation_result_count(&collision_handle),
            Err(OperationHandleError::ContextIdentity(
                ContextIdentityError::MarkerCollision
            ))
        );
    }

    #[test]
    fn operation_panics_are_contained_and_poison_the_session() {
        let mut session = session();
        let handle = session.create_module("owner").expect("owner module");

        let result = session.with_operation(&handle, |_, _| panic!("hostile operation"));
        assert_eq!(result, Err(OperationHandleError::UpstreamPanicked));
        assert!(session.is_poisoned());
        assert_eq!(
            session.operation_result_count(&handle),
            Err(OperationHandleError::SessionPoisoned)
        );
    }

    #[test]
    fn erased_handle_registry_entries_are_not_revived() {
        let mut session = session();
        let erased = session.create_module("first").expect("first module");
        let erased_pointer = session.operations[&erased.identity];
        let clone = erased.clone();

        session
            .erase_operation(&erased)
            .expect("erase first module");
        assert!(!session.operations.contains_key(&erased.identity));

        let replacement = session.create_module("second").expect("second module");
        assert!(erased.identity != replacement.identity);
        assert_ne!(erased_pointer, session.operations[&replacement.identity]);
        assert_eq!(
            session.operation_result_count(&clone),
            Err(OperationHandleError::StaleHandle)
        );
    }

    #[test]
    fn exhausted_handle_identity_fails_before_allocating_an_operation() {
        let mut session = session();
        session.next_operation_handle = None;

        assert!(matches!(
            session.create_module("owner"),
            Err(OperationHandleError::HandleSpaceExhausted)
        ));
        assert!(session.operations.is_empty());
    }

    #[cfg(feature = "internal-test-context-access")]
    #[test]
    fn test_context_action_panics_are_contained_and_poison_the_session() {
        let mut session = session();
        let result = session.with_context_mut(|_| panic!("hostile test action"));

        assert_eq!(result, Err(SessionPoisoned));
        assert!(session.is_poisoned());
        assert!(matches!(
            session.create_module("owner"),
            Err(OperationHandleError::SessionPoisoned)
        ));
    }
}
