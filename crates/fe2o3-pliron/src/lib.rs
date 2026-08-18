#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU64, Ordering},
};

use pliron::{
    context::Context,
    dialect::{Dialect, DialectName},
    identifier::Identifier,
    pass::Pass,
    uniqued_any::{self, UniquedKey},
};

/// The only accepted Pliron workspace revision for Wave 0.
pub const PLIRON_REVISION: &str = "2610651306ea3ba670f68d5d8b1e1159bcd521ed";

/// Hard implementation caps that configuration cannot exceed.
pub const HARD_MAX_DIALECTS: usize = 64;
pub const HARD_MAX_PASSES: usize = 256;
pub const HARD_MAX_NAME_BYTES: usize = 96;
pub const HARD_MAX_DIAGNOSTIC_BYTES: usize = 4_096;

/// Auxiliary-data key for the fe2o3 context-identity locator.
pub const CONTEXT_IDENTITY_MARKER_KEY: &str = "fe2o3_pliron_context_identity_v1";

static NEXT_CONTEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Opaque process-local identity for one Pliron context.
///
/// The value is descriptive provenance for in-memory handles. It is not a
/// durable compiler, artifact, proof, publication, or runtime identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContextIdentity(NonZeroU64);

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
    let anchor = uniqued_any::save(context, ContextIdentityAnchor(proposed_identity));
    let identity = uniqued_any::get(context, anchor).0;
    let marker = context
        .aux_data
        .insert(Box::new(ContextIdentityMarker { anchor }));
    context
        .aux_data_map
        .insert(context_identity_marker_key(), marker);
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
    let Some(index) = context
        .aux_data_map
        .get(&context_identity_marker_key())
        .copied()
    else {
        return Ok(None);
    };
    let Some(marker) = context.aux_data.get(index) else {
        return Err(ContextIdentityError::CorruptMarker);
    };
    let marker = marker
        .downcast_ref::<ContextIdentityMarker>()
        .ok_or(ContextIdentityError::MarkerCollision)?;
    catch_unwind(AssertUnwindSafe(|| {
        uniqued_any::get(context, marker.anchor).0
    }))
    .map(Some)
    .map_err(|_| ContextIdentityError::CorruptMarker)
}

fn next_context_identity() -> Result<NonZeroU64, ContextIdentityError> {
    let value = NEXT_CONTEXT_IDENTITY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ContextIdentityError::IdentitySpaceExhausted)?;
    NonZeroU64::new(value).ok_or(ContextIdentityError::IdentitySpaceExhausted)
}

fn context_identity_marker_key() -> Identifier {
    CONTEXT_IDENTITY_MARKER_KEY
        .try_into()
        .expect("static context identity key is valid")
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
    let first = bytes.next().expect("non-empty name");
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

/// Registers types, attributes, or operations after the shell creates a dialect.
pub type DialectRegistrationHook =
    fn(&mut Context, &DialectName) -> Result<(), RegistrationHookError>;

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
    DuplicateDialect(String),
    UpstreamRejectedDialect(String),
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
pub struct PlironSession {
    context: Context,
    manifest: ContextManifest,
    poisoned: bool,
}

impl PlironSession {
    /// Builds a fresh context after preflighting every registration.
    pub fn new(
        limits: ShellLimits,
        registrations: impl IntoIterator<Item = DialectRegistration>,
    ) -> Result<Self, ContextBuildError> {
        let registrations: Vec<_> = registrations.into_iter().collect();
        if registrations.len() > limits.max_dialects {
            return Err(ContextBuildError::TooManyDialects);
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
        for registration in &registrations {
            let dialect_name = DialectName::try_new(&registration.name).map_err(|_| {
                ContextBuildError::UpstreamRejectedDialect(registration.name.clone())
            })?;
            Dialect::register(&mut context, &dialect_name);
            if (registration.hook)(&mut context, &dialect_name).is_err() {
                return Err(ContextBuildError::RegistrationFailed(Diagnostic::new(
                    DiagnosticCode::DialectHookFailed,
                    Some(&registration.name),
                    "the explicit dialect registration hook failed",
                    limits.max_diagnostic_bytes,
                )));
            }
        }

        Ok(Self {
            context,
            manifest: ContextManifest {
                pliron_revision: PLIRON_REVISION,
                registration_order: registrations
                    .into_iter()
                    .map(|registration| registration.name)
                    .collect(),
            },
            poisoned: false,
        })
    }

    pub const fn manifest(&self) -> &ContextManifest {
        &self.manifest
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Grants scoped construction access without exposing context data as identity.
    pub fn with_context_mut<T>(
        &mut self,
        action: impl FnOnce(&mut Context) -> T,
    ) -> Result<T, SessionPoisoned> {
        if self.poisoned {
            return Err(SessionPoisoned);
        }
        Ok(action(&mut self.context))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPoisoned;

/// Why adding a pass to a deterministic plan failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassPlanError {
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
/// Pliron operation pointers do not carry context provenance, so a safe API
/// cannot distinguish a same-slot foreign root from a root owned by the
/// session. Issue #140 tracks the owner-aware handle required before execution
/// can be restored.
pub struct PassPlan {
    limits: ShellLimits,
    passes: Vec<PlannedPass>,
    names: BTreeSet<String>,
}

impl PassPlan {
    pub fn new(limits: ShellLimits) -> Self {
        Self {
            limits,
            passes: Vec::new(),
            names: BTreeSet::new(),
        }
    }

    pub fn add_pass(&mut self, mut pass: impl Pass + 'static) -> Result<(), PassPlanError> {
        if self.passes.len() == self.limits.max_passes {
            return Err(PassPlanError::TooManyPasses);
        }
        let name = pass.name().to_owned();
        validate_name(&name, NameKind::Pass).map_err(PassPlanError::InvalidName)?;
        if pass.as_pass_manager().is_some() {
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

    pub fn pass_order(&self) -> impl ExactSizeIterator<Item = &str> {
        self.passes.iter().map(|pass| pass.name.as_str())
    }
}
