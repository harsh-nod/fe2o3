#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU64, Ordering},
};

use pliron::{
    attribute::Attribute,
    context::Context,
    dialect::{Dialect, DialectName},
    identifier::Identifier,
    location::Location,
    op::{Op, OpBox},
    parsable::Parsable,
    r#type::{Type, TypedHandle},
    uniqued_any::{self, UniquedKey},
};

/// The only accepted Pliron workspace revision for Wave 0.
pub const PLIRON_REVISION: &str = "2610651306ea3ba670f68d5d8b1e1159bcd521ed";

/// Hard byte cap for dialect names admitted by registration adapters.
pub const HARD_MAX_NAME_BYTES: usize = 96;

/// Hard cap on typed entity registrations performed by one dialect hook.
pub const HARD_MAX_DIALECT_REGISTRATION_ACTIONS: usize = 128;

/// Auxiliary-data key for the fe2o3 context-identity locator.
pub const CONTEXT_IDENTITY_MARKER_KEY: &str = "fe2o3_pliron_context_identity_v1";

static NEXT_CONTEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Opaque process-local identity for one Pliron context.
///
/// The value is descriptive provenance for in-memory handles. It is not a
/// durable compiler, artifact, proof, publication, or runtime identity. Its
/// representation cannot be constructed or recovered by callers:
///
/// ```compile_fail
/// use std::num::NonZeroU64;
/// use fe2o3_pliron_owner_core::ContextIdentity;
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

/// Why a bounded dialect name was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameError {
    /// The name was empty.
    Empty,
    /// The name exceeded [`HARD_MAX_NAME_BYTES`].
    TooLong,
    /// The first byte was not a lowercase ASCII letter.
    InvalidFirstByte,
    /// A later byte was not lowercase ASCII, a digit, or an underscore.
    InvalidByte,
}

/// Validates a bounded dialect namespace or identifier name.
pub fn validate_dialect_name(value: &str) -> Result<(), NameError> {
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
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_') {
            return Err(NameError::InvalidByte);
        }
    }
    Ok(())
}

/// An error returned by a dialect's explicit registration hook.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationHookError {
    detail: String,
}

impl RegistrationHookError {
    /// Creates a registration rejection with noncanonical diagnostic detail.
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

impl Error for RegistrationHookError {}

/// A bounded registration capability borrowed from one context construction.
///
/// The service cannot be constructed or disassembled outside this crate. It
/// exposes no context, dialect object, arena pointer, generic callback, or
/// caller-provided state. Its borrow cannot outlive the registration hook:
///
/// ```compile_fail
/// use fe2o3_pliron_owner_core::DialectRegistrationService;
/// use pliron::{context::Context, dialect::DialectName};
///
/// let mut context = Context::new();
/// let dialect_name = DialectName::try_new("gpu").unwrap();
/// let forged = DialectRegistrationService {
///     context: &mut context,
///     dialect_name: &dialect_name,
///     actions: 0,
/// };
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron_owner_core::DialectRegistrationService;
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
///
/// Its hook and owned name cannot be replaced by callers after validation:
///
/// ```compile_fail
/// use fe2o3_pliron_owner_core::{
///     DialectRegistration, DialectRegistrationService, RegistrationHookError,
/// };
///
/// fn hook(
///     _service: &mut DialectRegistrationService<'_>,
/// ) -> Result<(), RegistrationHookError> {
///     Ok(())
/// }
///
/// let forged = DialectRegistration { name: "gpu".into(), hook };
/// ```
#[derive(Clone)]
pub struct DialectRegistration {
    name: String,
    hook: DialectRegistrationHook,
}

impl DialectRegistration {
    /// Validates and creates one named registration adapter.
    pub fn new(name: &str, hook: DialectRegistrationHook) -> Result<Self, NameError> {
        validate_dialect_name(name)?;
        Ok(Self {
            name: name.to_owned(),
            hook,
        })
    }

    /// Returns the validated dialect namespace.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Registers the dialect and invokes its hook through a bounded service.
    ///
    /// This is the owner-shell integration point. It does not expose the
    /// context to the hook or return any raw Pliron handle.
    #[doc(hidden)]
    pub fn register_into(
        &self,
        context: &mut Context,
        dialect_name: &DialectName,
    ) -> Result<(), RegistrationHookError> {
        if dialect_name.as_ref() != self.name {
            return Err(RegistrationHookError::new(
                "dialect registration name mismatch",
            ));
        }
        Dialect::register(context, dialect_name);
        let mut service = DialectRegistrationService::new(context, dialect_name);
        (self.hook)(&mut service)
    }
}
