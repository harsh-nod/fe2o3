#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{collections::BTreeSet, fmt};

use pliron::{
    context::{Context, Ptr},
    dialect::{Dialect, DialectName},
    irbuild::IRStatus,
    operation::{Operation, verify_operation},
    pass::{AnalysisManager, Pass, PassManager, PassResult},
};

/// The only accepted Pliron workspace revision for Wave 0.
pub const PLIRON_REVISION: &str = "2610651306ea3ba670f68d5d8b1e1159bcd521ed";

/// Hard implementation caps that configuration cannot exceed.
pub const HARD_MAX_DIALECTS: usize = 64;
pub const HARD_MAX_PASSES: usize = 256;
pub const HARD_MAX_NAME_BYTES: usize = 96;
pub const HARD_MAX_DIAGNOSTIC_BYTES: usize = 4_096;

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
    PassIdentityChanged,
    VerifyBeforeFailed,
    PassFailed,
    VerifyAfterFailed,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DialectHookFailed => "FE2O3-PLIRON-DIALECT-HOOK-FAILED",
            Self::PassIdentityChanged => "FE2O3-PLIRON-PASS-IDENTITY-CHANGED",
            Self::VerifyBeforeFailed => "FE2O3-PLIRON-VERIFY-BEFORE-FAILED",
            Self::PassFailed => "FE2O3-PLIRON-PASS-FAILED",
            Self::VerifyAfterFailed => "FE2O3-PLIRON-VERIFY-AFTER-FAILED",
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
    limits: ShellLimits,
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
            limits,
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
    pass: Box<dyn Pass>,
}

/// A bounded pass sequence. Insertion order is execution order.
pub struct PassPipeline {
    limits: ShellLimits,
    passes: Vec<PlannedPass>,
    names: BTreeSet<String>,
}

impl PassPipeline {
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
            pass: Box::new(pass),
        });
        Ok(())
    }

    pub fn pass_order(&self) -> impl ExactSizeIterator<Item = &str> {
        self.passes.iter().map(|pass| pass.name.as_str())
    }

    /// Runs each pass through Pliron after explicit pre-verification and before
    /// explicit post-verification. The session is poisoned on any failure.
    pub fn run(
        mut self,
        session: &mut PlironSession,
        root: Ptr<Operation>,
    ) -> Result<PipelineReport, PipelineFailure> {
        if session.poisoned {
            return Err(PipelineFailure::session_poisoned());
        }

        let mut completed = Vec::with_capacity(self.passes.len());
        let mut analyses = AnalysisManager::default();
        for (ordinal, planned) in self.passes.iter_mut().enumerate() {
            if planned.pass.name() != planned.name || planned.pass.as_pass_manager().is_some() {
                let receipt = StageReceipt::failed(
                    ordinal,
                    &planned.name,
                    StageStatus::NotRun,
                    StageStatus::NotRun,
                    StageStatus::NotRun,
                );
                return Err(fail_pipeline(
                    session,
                    completed,
                    receipt,
                    DiagnosticCode::PassIdentityChanged,
                    "the pass identity or leaf shape changed after plan construction",
                ));
            }

            if verify_operation(root, &session.context).is_err() {
                let receipt = StageReceipt::failed(
                    ordinal,
                    &planned.name,
                    StageStatus::Failed,
                    StageStatus::NotRun,
                    StageStatus::NotRun,
                );
                return Err(fail_pipeline(
                    session,
                    completed,
                    receipt,
                    DiagnosticCode::VerifyBeforeFailed,
                    "Pliron verification failed before the pass",
                ));
            }

            let pass_result = match ShellPassManager::run_pass(
                &mut *planned.pass,
                root,
                &mut session.context,
                &mut analyses,
            ) {
                Ok(result) => result,
                Err(_) => {
                    let receipt = StageReceipt::failed(
                        ordinal,
                        &planned.name,
                        StageStatus::Passed,
                        StageStatus::Failed,
                        StageStatus::NotRun,
                    );
                    return Err(fail_pipeline(
                        session,
                        completed,
                        receipt,
                        DiagnosticCode::PassFailed,
                        "the Pliron pass returned an error",
                    ));
                }
            };
            analyses.retain_preserved(&pass_result);

            if verify_operation(root, &session.context).is_err() {
                let receipt = StageReceipt::failed_with_effect(
                    ordinal,
                    &planned.name,
                    StageStatus::Passed,
                    StageStatus::Passed,
                    StageStatus::Failed,
                    pass_result.ir_changed,
                );
                return Err(fail_pipeline(
                    session,
                    completed,
                    receipt,
                    DiagnosticCode::VerifyAfterFailed,
                    "Pliron verification failed after the pass",
                ));
            }

            completed.push(StageReceipt::passed(ordinal, &planned.name, pass_result));
        }
        Ok(PipelineReport {
            receipts: completed,
        })
    }
}

struct ShellPassManager;
impl PassManager for ShellPassManager {}

/// A stage status with no implied proof, publication, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageStatus {
    NotRun,
    Passed,
    Failed,
}

/// The only authority effect a D0 stage-attempt receipt can represent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityEffect {
    None,
}

/// A deterministic observation of one pass attempt.
///
/// This is not a canonical identity, proof receipt, publication receipt, or
/// launch capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageReceipt {
    ordinal: usize,
    pass_name: String,
    verify_before: StageStatus,
    pass: StageStatus,
    verify_after: StageStatus,
    ir_changed: Option<bool>,
    authority_effect: AuthorityEffect,
}

impl StageReceipt {
    fn passed(ordinal: usize, pass_name: &str, result: PassResult) -> Self {
        Self {
            ordinal,
            pass_name: pass_name.to_owned(),
            verify_before: StageStatus::Passed,
            pass: StageStatus::Passed,
            verify_after: StageStatus::Passed,
            ir_changed: Some(result.ir_changed == IRStatus::Changed),
            authority_effect: AuthorityEffect::None,
        }
    }

    fn failed(
        ordinal: usize,
        pass_name: &str,
        verify_before: StageStatus,
        pass: StageStatus,
        verify_after: StageStatus,
    ) -> Self {
        Self::failed_with_effect(
            ordinal,
            pass_name,
            verify_before,
            pass,
            verify_after,
            IRStatus::Unchanged,
        )
        .without_effect_observation()
    }

    fn failed_with_effect(
        ordinal: usize,
        pass_name: &str,
        verify_before: StageStatus,
        pass: StageStatus,
        verify_after: StageStatus,
        effect: IRStatus,
    ) -> Self {
        Self {
            ordinal,
            pass_name: pass_name.to_owned(),
            verify_before,
            pass,
            verify_after,
            ir_changed: Some(effect == IRStatus::Changed),
            authority_effect: AuthorityEffect::None,
        }
    }

    fn without_effect_observation(mut self) -> Self {
        self.ir_changed = None;
        self
    }

    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn pass_name(&self) -> &str {
        &self.pass_name
    }

    pub const fn verify_before(&self) -> StageStatus {
        self.verify_before
    }

    pub const fn pass_status(&self) -> StageStatus {
        self.pass
    }

    pub const fn verify_after(&self) -> StageStatus {
        self.verify_after
    }

    pub const fn ir_changed(&self) -> Option<bool> {
        self.ir_changed
    }

    pub const fn authority_effect(&self) -> AuthorityEffect {
        self.authority_effect
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineReport {
    receipts: Vec<StageReceipt>,
}

impl PipelineReport {
    pub fn receipts(&self) -> &[StageReceipt] {
        &self.receipts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineFailure {
    completed: Vec<StageReceipt>,
    failed: Option<StageReceipt>,
    diagnostic: Option<Diagnostic>,
}

impl PipelineFailure {
    fn session_poisoned() -> Self {
        Self {
            completed: Vec::new(),
            failed: None,
            diagnostic: None,
        }
    }

    pub fn completed(&self) -> &[StageReceipt] {
        &self.completed
    }

    pub const fn failed(&self) -> Option<&StageReceipt> {
        self.failed.as_ref()
    }

    pub const fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }
}

fn fail_pipeline(
    session: &mut PlironSession,
    completed: Vec<StageReceipt>,
    failed: StageReceipt,
    code: DiagnosticCode,
    message: &str,
) -> PipelineFailure {
    session.poisoned = true;
    let diagnostic = Diagnostic::new(
        code,
        Some(failed.pass_name()),
        message,
        session.limits.max_diagnostic_bytes,
    );
    PipelineFailure {
        completed,
        failed: Some(failed),
        diagnostic: Some(diagnostic),
    }
}
