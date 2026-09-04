#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod kir_bridge_v1;
mod optimization_v1;
mod production;

pub use kir_bridge_v1::*;
pub use optimization_v1::*;

pub use production::{
    ConstructedGraphStageV1, ConstructionRegisteredStageV1, HARD_MAX_PRODUCTION_CONSTRUCTIONS,
    HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, InertProductionMiddleEndEvidenceV4,
    InertProductionMiddleEndEvidenceV5, KernelChecksVerifiedGraphStageV1,
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4, MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
    MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4, MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2, MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
    PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2, PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5, PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5, PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5, PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2,
    ProductionCheckedNonCanonicalLoopProofImportV1, ProductionCollectiveSemanticContractV1,
    ProductionCollectiveSemanticKindV1, ProductionConstructionV1,
    ProductionCooperativeTensorBindingV1, ProductionEffectRefinementContractV2,
    ProductionFunctionalRefinementAdmissionErrorV2, ProductionGpuWriteSiteV2,
    ProductionIeeeExceptionalValuePolicyV2, ProductionIeeeRoundingModeV2,
    ProductionMiddleEndAssuranceV4, ProductionMiddleEndAssuranceV5,
    ProductionMiddleEndCoverageSummaryV5, ProductionMiddleEndEvidenceCodecErrorV4,
    ProductionMiddleEndEvidenceCodecErrorV5, ProductionMiddleEndEvidenceIdentityV4,
    ProductionMiddleEndEvidenceIdentityV5, ProductionMiddleEndEvidencePassV4,
    ProductionMiddleEndEvidencePassV5, ProductionMiddleEndEvidenceV5,
    ProductionMiddleEndPassSuccessV4, ProductionMiddleEndPassSuccessV5,
    ProductionMiddleEndSemanticSummaryV5, ProductionMiddleEndTypedSemanticReconciliationV5,
    ProductionMirPlironReconciliationErrorV1, ProductionMirPlironSemanticContractDerivationErrorV1,
    ProductionMirPlironSemanticContractErrorV1, ProductionMirPlironSemanticContractReportV1,
    ProductionNonCanonicalLoopClaimsV1, ProductionNonCanonicalLoopProofErrorV1,
    ProductionNonCanonicalLoopProofRequestV1, ProductionNonCanonicalLoopProofRequirementV1,
    ProductionNumericalContractV2, ProductionNumericalRefinementContractV2,
    ProductionOverflowContractV2, ProductionParallelReferenceContractBuilderV1,
    ProductionParallelReferenceContractErrorV1, ProductionParallelReferenceContractReportV1,
    ProductionPlironSessionV1, ProductionPolicyCheckedRefinementStagingV2, ProductionRankedBlockV1,
    ProductionRankedCompileErrorV1, ProductionRankedCompileErrorV2, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReconciledMirPlironKernelV1, ProductionReconciledMirPlironSemanticContractV1,
    ProductionReferenceOutputSiteV2, ProductionReferenceProofV2, ProductionRootHandleV1,
    ProductionSemanticBinaryOpV2, ProductionSemanticCastV2, ProductionSemanticComparisonV2,
    ProductionSemanticExpressionErrorV2, ProductionSemanticExpressionStatsV2,
    ProductionSemanticExpressionV2, ProductionSemanticLoadV2, ProductionSemanticMirErrorV1,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticScalarTypeV2,
    ProductionSemanticUnaryOpV2, ProductionSessionErrorV1, ProductionSessionLimitErrorV1,
    ProductionSessionLimitsV1, ProductionStageHandleV1, ProductionStagedArithmeticCoverageV2,
    ProductionTensorInstructionSiteV1, ProductionTensorRefinementContractV1,
    ProductionTensorResultComponentV1, ProductionTotalOutputStagingErrorV2,
    ProductionTotalOutputStagingReportV2, ProductionTypedSemanticCommitmentReconciliationV2,
    ProductionTypedSemanticObligationSummaryV2, compile_ranked_kernel_for_gfx942_lowering_v1,
    compile_ranked_kernel_for_lowering_v1, derive_and_reconcile_mir_pliron_semantic_contract_v1,
    derive_and_require_parallel_reference_contract_v1, derive_noncanonical_loop_proof_request_v1,
    derive_noncanonical_loop_proof_requirement_v1, normalized_effect_refinement_hash_for_kernel_v2,
    normalized_functional_refinement_formula_hash_for_kernel_v2,
    normalized_numerical_refinement_hash_for_kernel_v2,
    normalized_tensor_refinement_hash_for_kernel_v1, production_dynamic_loop_bound_identity_v1,
    production_effect_contract_identity_v1, production_loop_transition_identity_v1,
    production_loop_variant_identity_v1, production_ranked_value_identity_v1,
    reconcile_ranked_kernel_with_safe_reference_mir_v1, require_mir_pliron_semantic_contract_v1,
    require_parallel_reference_contract_v1, require_total_output_staging_v2,
    typed_semantic_commitment_reconciliation_v2, typed_semantic_obligation_summary_v2,
};

#[cfg(feature = "internal-proof-staging")]
pub use production::{
    ProductionRefinementStagingPolicyV2,
    compile_ranked_kernel_with_policy_checked_refinement_staging_v2,
    import_noncanonical_loop_proof_v1,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub use fe2o3_pliron_owner_core::{
    CONTEXT_IDENTITY_MARKER_KEY, ContextIdentity, ContextIdentityError, DialectRegistration,
    DialectRegistrationHook, DialectRegistrationService, HARD_MAX_DIALECT_REGISTRATION_ACTIONS,
    HARD_MAX_NAME_BYTES, NameError, PLIRON_REVISION, RegistrationHookError,
    ensure_context_identity, require_context_identity, validate_dialect_name,
};
use pliron::{
    builtin::op_interfaces::SymbolOpInterface,
    builtin::ops::ModuleOp,
    combine::{Parser, eof},
    context::Context,
    context::Ptr,
    dialect::DialectName,
    identifier::Identifier,
    irfmt::parsers::spaced,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
    pass::Pass,
};

/// Hard implementation caps that configuration cannot exceed.
pub const HARD_MAX_DIALECTS: usize = 64;
pub const HARD_MAX_PASSES: usize = 256;
pub const HARD_MAX_DIAGNOSTIC_BYTES: usize = 4_096;
pub const HARD_MAX_OPERATION_HANDLES: usize = 4_096;
pub const HARD_MAX_OPERATION_REGIONS: usize = 64;
pub const HARD_MAX_OPERATION_BLOCKS: usize = 4_096;
pub const HARD_MAX_OPERATION_CHILDREN: usize = 4_096;
pub const HARD_MAX_OPERATION_IMPORT_BYTES: usize = 1_048_576;
pub const HARD_MAX_OPERATION_IMPORT_NESTING: usize = 256;
pub const HARD_MAX_OPERATION_TREE_ITEMS: usize = 16_384;
pub const HARD_MAX_SESSION_OPERATION_IMPORT_BYTES: usize = 1_048_576;
pub const HARD_MAX_SESSION_OPERATION_TREE_ITEMS: usize = 65_536;

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

fn validate_name(value: &str, kind: NameKind) -> Result<(), NameError> {
    if kind == NameKind::Dialect {
        return validate_dialect_name(value);
    }
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
        if !common && !matches!(byte, b'.' | b'-') {
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
    operation_roots: BTreeMap<OperationHandleIdentity, OperationHandleIdentity>,
    owned_tree_work: BTreeMap<OperationHandleIdentity, usize>,
    operation_import_bytes: usize,
    operation_tree_work: usize,
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

/// A bounded, pointer-free description of one authenticated operation.
///
/// Counts describe the operation and its immediate regions, blocks, and child
/// operations. They do not recursively traverse child operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationShapeV1 {
    operand_count: usize,
    result_count: usize,
    region_count: usize,
    block_count: usize,
    child_operation_count: usize,
}

impl OperationShapeV1 {
    pub const fn operand_count(self) -> usize {
        self.operand_count
    }

    pub const fn result_count(self) -> usize {
        self.result_count
    }

    pub const fn region_count(self) -> usize {
        self.region_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn child_operation_count(self) -> usize {
        self.child_operation_count
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
    OperationGraphOwnershipMismatch,
    HandleSpaceExhausted,
    TooManyOperationHandles,
    TooManyOperationRegions,
    TooManyOperationBlocks,
    TooManyOperationChildren,
    EmptyOperationImport,
    OperationImportTooLarge,
    OperationImportNestingTooDeep,
    SessionOperationImportLimitExceeded,
    OperationImportRejected,
    OperationVerificationRejected,
    ConstructionRecipeMismatch,
    OperationTreeLimitExceeded,
    SessionOperationTreeLimitExceeded,
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
            Self::OperationGraphOwnershipMismatch => {
                formatter.write_str("operation handle has inconsistent root ownership")
            }
            Self::HandleSpaceExhausted => {
                formatter.write_str("operation handle identity space is exhausted")
            }
            Self::TooManyOperationHandles => {
                formatter.write_str("operation handle count exceeds the hard limit")
            }
            Self::TooManyOperationRegions => {
                formatter.write_str("operation region count exceeds the hard limit")
            }
            Self::TooManyOperationBlocks => {
                formatter.write_str("operation block count exceeds the hard limit")
            }
            Self::TooManyOperationChildren => {
                formatter.write_str("child operation count exceeds the hard limit")
            }
            Self::EmptyOperationImport => formatter.write_str("operation import is empty"),
            Self::OperationImportTooLarge => {
                formatter.write_str("operation import exceeds the hard byte limit")
            }
            Self::OperationImportNestingTooDeep => {
                formatter.write_str("operation import exceeds the hard nesting limit")
            }
            Self::SessionOperationImportLimitExceeded => {
                formatter.write_str("session operation imports exceed the hard byte limit")
            }
            Self::OperationImportRejected => formatter.write_str("operation import was rejected"),
            Self::OperationVerificationRejected => {
                formatter.write_str("operation failed recursive verification")
            }
            Self::ConstructionRecipeMismatch => {
                formatter.write_str("constructed operation does not match its production recipe")
            }
            Self::OperationTreeLimitExceeded => {
                formatter.write_str("operation tree exceeds the hard work limit")
            }
            Self::SessionOperationTreeLimitExceeded => {
                formatter.write_str("session operation trees exceed the hard work limit")
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
            if !seen.insert(registration.name().to_owned()) {
                return Err(ContextBuildError::DuplicateDialect(
                    registration.name().to_owned(),
                ));
            }
        }

        let mut context = Context::new();
        let identity = catch_unwind(AssertUnwindSafe(|| ensure_context_identity(&mut context)))
            .map_err(|_| ContextBuildError::ContextIdentity(ContextIdentityError::CorruptMarker))?
            .map_err(ContextBuildError::ContextIdentity)?;
        for registration in &registrations {
            let dialect_name = DialectName::try_new(registration.name()).map_err(|_| {
                ContextBuildError::UpstreamRejectedDialect(registration.name().to_owned())
            })?;
            let hook_result = catch_unwind(AssertUnwindSafe(|| {
                registration.register_into(&mut context, &dialect_name)
            }));
            if !matches!(hook_result, Ok(Ok(()))) {
                return Err(ContextBuildError::RegistrationFailed(Diagnostic::new(
                    DiagnosticCode::DialectHookFailed,
                    Some(registration.name()),
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
                    .map(|registration| registration.name().to_owned())
                    .collect(),
            },
            operations: BTreeMap::new(),
            operation_roots: BTreeMap::new(),
            owned_tree_work: BTreeMap::new(),
            operation_import_bytes: 0,
            operation_tree_work: 0,
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
        let session_work = self
            .operation_tree_work
            .checked_add(3)
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
            .ok_or(OperationHandleError::SessionOperationTreeLimitExceeded)?;
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
        self.operation_roots.insert(identity, identity);
        self.owned_tree_work.insert(identity, 3);
        self.operation_tree_work = session_work;
        Ok(OperationHandle {
            owner: self.identity,
            identity,
        })
    }

    /// Reaccounts and recursively verifies a root after a closed internal builder
    /// appends typed children. This is crate-private so production callers cannot
    /// mutate a registered graph or inject a construction callback.
    pub(crate) fn finish_internal_root_construction(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<(), OperationHandleError> {
        self.validate_identity()?;
        if handle.owner != self.identity {
            return Err(OperationHandleError::ForeignSession);
        }
        let pointer = self
            .operations
            .get(&handle.identity)
            .copied()
            .ok_or(OperationHandleError::StaleHandle)?;
        let old_work = self
            .owned_tree_work
            .get(&handle.identity)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let tree_work = match catch_unwind(AssertUnwindSafe(|| {
            inspect_operation_tree(pointer, &mut self.context)
        })) {
            Ok(Ok(tree_work)) => tree_work,
            Ok(Err(error)) => {
                self.poisoned = true;
                return Err(error);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        };
        let session_work = self
            .operation_tree_work
            .checked_sub(old_work)
            .and_then(|work| work.checked_add(tree_work))
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS);
        let Some(session_work) = session_work else {
            self.poisoned = true;
            return Err(OperationHandleError::SessionOperationTreeLimitExceeded);
        };
        match catch_unwind(AssertUnwindSafe(|| {
            verify_operation(pointer, &self.context)
        })) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                self.poisoned = true;
                return Err(OperationHandleError::OperationVerificationRejected);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        }
        self.owned_tree_work.insert(handle.identity, tree_work);
        self.operation_tree_work = session_work;
        Ok(())
    }

    /// Checks exact closed-builder tree work before any upstream allocation.
    pub(crate) fn require_internal_tree_capacity(
        &self,
        tree_work: usize,
    ) -> Result<(), OperationHandleError> {
        self.operation_tree_work
            .checked_add(tree_work)
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
            .map(|_| ())
            .ok_or(OperationHandleError::SessionOperationTreeLimitExceeded)
    }

    /// Imports one byte- and tree-guarded textual Pliron root into this owner session.
    ///
    /// Text is a noncanonical construction bridge only. It must never be used
    /// as an artifact, proof, cache, publication, or runtime identity. Parsing
    /// requires exact end-of-input and recursive verification; after parsing
    /// starts, any rejection poisons the session because upstream allocation is
    /// not transactional. A printer/text round trip cannot grant a supported
    /// production compiler capability; typed owner-held construction must
    /// replace this bridge first.
    ///
    /// Registered Pliron parser implementations are trusted code at this
    /// transitional boundary. The preflight limits only the parser input and
    /// the delimiter syntax understood by the pinned, audited parser set; the
    /// postflight limits the graph returned by a parser. Neither limit meters
    /// CPU time, temporary allocations, interning, comments, literals, or
    /// private syntax inside an arbitrary `Parsable` implementation. A caller
    /// that links or registers another parser must audit or contain that
    /// implementation independently.
    pub fn import_operation_text_v1(
        &mut self,
        text: &str,
    ) -> Result<OperationHandle, OperationHandleError> {
        if text.is_empty() {
            return Err(OperationHandleError::EmptyOperationImport);
        }
        if text.len() > HARD_MAX_OPERATION_IMPORT_BYTES {
            return Err(OperationHandleError::OperationImportTooLarge);
        }
        preflight_operation_import_nesting(text)?;
        self.validate_identity()?;
        let session_import_bytes = self
            .operation_import_bytes
            .checked_add(text.len())
            .filter(|bytes| *bytes <= HARD_MAX_SESSION_OPERATION_IMPORT_BYTES)
            .ok_or(OperationHandleError::SessionOperationImportLimitExceeded)?;
        if self.operation_tree_work >= HARD_MAX_SESSION_OPERATION_TREE_ITEMS {
            return Err(OperationHandleError::SessionOperationTreeLimitExceeded);
        }
        let identity = self.allocate_operation_handle()?;
        let pointer = match catch_unwind(AssertUnwindSafe(|| {
            parse_from_str(
                spaced(Operation::top_level_parser()).skip(eof()),
                &mut self.context,
                text,
            )
        })) {
            Ok(Ok(pointer)) => pointer,
            Ok(Err(_)) => {
                self.poisoned = true;
                return Err(OperationHandleError::OperationImportRejected);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        };
        let tree_work = match catch_unwind(AssertUnwindSafe(|| {
            inspect_operation_tree(pointer, &mut self.context)
        })) {
            Ok(Ok(tree_work)) => tree_work,
            Ok(Err(error)) => {
                self.poisoned = true;
                return Err(error);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        };
        let session_work = self
            .operation_tree_work
            .checked_add(tree_work)
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS);
        let Some(session_work) = session_work else {
            self.poisoned = true;
            return Err(OperationHandleError::SessionOperationTreeLimitExceeded);
        };
        match catch_unwind(AssertUnwindSafe(|| {
            verify_operation(pointer, &self.context)
        })) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                self.poisoned = true;
                return Err(OperationHandleError::OperationVerificationRejected);
            }
            Err(_) => {
                self.poisoned = true;
                return Err(OperationHandleError::UpstreamPanicked);
            }
        }

        self.operations.insert(identity, pointer);
        self.operation_roots.insert(identity, identity);
        self.owned_tree_work.insert(identity, tree_work);
        self.operation_import_bytes = session_import_bytes;
        self.operation_tree_work = session_work;
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

    /// Returns a bounded, pointer-free description of an authenticated operation.
    pub fn operation_shape(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<OperationShapeV1, OperationHandleError> {
        self.with_operation(handle, inspect_operation)
            .and_then(|inspection| inspection.map(|(shape, _)| shape))
    }

    /// Tests an authenticated operation's concrete Pliron type without exposing it.
    pub fn operation_is<O: Op>(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<bool, OperationHandleError> {
        self.with_operation(handle, |pointer, context| {
            Operation::is_op::<O>(pointer, context)
        })
    }

    /// Returns stable owner-aware handles for immediate child operations.
    ///
    /// Children are ordered by region, block, and operation order. Traversal and
    /// handle allocation are preflighted, so a limit failure changes no registry
    /// state.
    pub fn operation_children(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<Vec<OperationHandle>, OperationHandleError> {
        let (_, children) = self
            .with_operation(handle, inspect_operation)
            .and_then(|inspection| inspection)?;
        let root = self
            .operation_roots
            .get(&handle.identity)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        self.register_operation_pointers(&children, root)
    }

    /// Erases an authenticated operation, invalidating all clones of its handle.
    pub fn erase_operation(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<(), OperationHandleError> {
        let (subtree_work, subtree) = self
            .with_operation(handle, inspect_operation_tree_details)
            .and_then(|inspection| inspection)?;
        let root = self
            .operation_roots
            .get(&handle.identity)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let removed = self
            .operations
            .iter()
            .filter_map(|(identity, pointer)| subtree.contains(pointer).then_some(*identity))
            .collect::<Vec<_>>();
        if removed.is_empty()
            || removed
                .iter()
                .any(|identity| self.operation_roots.get(identity).copied() != Some(root))
        {
            return Err(OperationHandleError::OperationGraphOwnershipMismatch);
        }
        let charged_root_work = self
            .owned_tree_work
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let refunded_work = if handle.identity == root {
            if charged_root_work != subtree_work {
                return Err(OperationHandleError::OperationGraphOwnershipMismatch);
            }
            charged_root_work
        } else {
            subtree_work
                .checked_add(1)
                .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?
        };
        let remaining_root_work = charged_root_work
            .checked_sub(refunded_work)
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let remaining_session_work = self
            .operation_tree_work
            .checked_sub(refunded_work)
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;

        self.with_operation(handle, Operation::erase)?;
        for identity in removed {
            self.operations.remove(&identity);
            self.operation_roots.remove(&identity);
        }
        if handle.identity == root {
            self.owned_tree_work.remove(&root);
        } else if let Some(root_work) = self.owned_tree_work.get_mut(&root) {
            *root_work = remaining_root_work;
        } else {
            self.poisoned = true;
            return Err(OperationHandleError::OperationGraphOwnershipMismatch);
        }
        self.operation_tree_work = remaining_session_work;
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
        if self.operations.len() >= HARD_MAX_OPERATION_HANDLES {
            return Err(OperationHandleError::TooManyOperationHandles);
        }
        let identity = self
            .next_operation_handle
            .take()
            .map(OperationHandleIdentity)
            .ok_or(OperationHandleError::HandleSpaceExhausted)?;
        self.next_operation_handle = identity.0.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(identity)
    }

    fn register_operation_pointers(
        &mut self,
        pointers: &[Ptr<Operation>],
        root: OperationHandleIdentity,
    ) -> Result<Vec<OperationHandle>, OperationHandleError> {
        let mut unseen = Vec::new();
        for pointer in pointers {
            let registered = self
                .operations
                .iter()
                .find_map(|(identity, registered)| (registered == pointer).then_some(*identity));
            if let Some(identity) = registered
                && self.operation_roots.get(&identity) != Some(&root)
            {
                return Err(OperationHandleError::OperationGraphOwnershipMismatch);
            }
            if registered.is_none() && !unseen.contains(pointer) {
                unseen.push(*pointer);
            }
        }

        let final_count = self
            .operations
            .len()
            .checked_add(unseen.len())
            .ok_or(OperationHandleError::TooManyOperationHandles)?;
        if final_count > HARD_MAX_OPERATION_HANDLES {
            return Err(OperationHandleError::TooManyOperationHandles);
        }
        if let Some(required_offset) = unseen.len().checked_sub(1) {
            let start = self
                .next_operation_handle
                .ok_or(OperationHandleError::HandleSpaceExhausted)?;
            let required_offset = u64::try_from(required_offset)
                .map_err(|_| OperationHandleError::HandleSpaceExhausted)?;
            start
                .get()
                .checked_add(required_offset)
                .ok_or(OperationHandleError::HandleSpaceExhausted)?;
        }

        for pointer in unseen {
            let identity = self.allocate_operation_handle()?;
            self.operations.insert(identity, pointer);
            self.operation_roots.insert(identity, root);
        }

        pointers
            .iter()
            .map(|pointer| {
                let identity = self
                    .operations
                    .iter()
                    .find_map(|(identity, registered)| (registered == pointer).then_some(*identity))
                    .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
                if self.operation_roots.get(&identity) != Some(&root) {
                    return Err(OperationHandleError::OperationGraphOwnershipMismatch);
                }
                Ok(OperationHandle {
                    owner: self.identity,
                    identity,
                })
            })
            .collect()
    }

    fn validate_production_module(
        &mut self,
        handle: &OperationHandle,
        expected_name: &str,
    ) -> Result<OperationShapeV1, OperationHandleError> {
        let validation = self
            .with_operation(handle, |pointer, context| {
                let tree_work = inspect_operation_tree(pointer, context)?;
                verify_operation(pointer, context)
                    .map_err(|_| OperationHandleError::OperationVerificationRejected)?;
                let module = Operation::get_op::<ModuleOp>(pointer, context)
                    .ok_or(OperationHandleError::ConstructionRecipeMismatch)?;
                let shape = inspect_operation(pointer, context)?.0;
                if tree_work != 3
                    || module.get_symbol_name(context).as_ref() != expected_name
                    || shape
                        != (OperationShapeV1 {
                            operand_count: 0,
                            result_count: 0,
                            region_count: 1,
                            block_count: 1,
                            child_operation_count: 0,
                        })
                {
                    return Err(OperationHandleError::ConstructionRecipeMismatch);
                }
                Ok(shape)
            })
            .and_then(|validation| validation);
        if validation.is_err() {
            self.poisoned = true;
        }
        validation
    }
}

fn preflight_operation_import_nesting(text: &str) -> Result<(), OperationHandleError> {
    let mut delimiters = [0_u8; HARD_MAX_OPERATION_IMPORT_NESTING];
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut previous = None;

    for byte in text.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
                previous = Some(byte);
            }
            continue;
        }

        match byte {
            b'"' => {
                quoted = true;
                previous = Some(byte);
            }
            b'(' | b'[' | b'{' | b'<' => {
                if depth == HARD_MAX_OPERATION_IMPORT_NESTING {
                    return Err(OperationHandleError::OperationImportNestingTooDeep);
                }
                delimiters[depth] = byte;
                depth += 1;
                previous = Some(byte);
            }
            b')' | b']' | b'}' | b'>' => {
                let arrow = byte == b'>' && previous == Some(b'-');
                let expected = match byte {
                    b')' => b'(',
                    b']' => b'[',
                    b'}' => b'{',
                    b'>' => b'<',
                    _ => unreachable!(),
                };
                if !arrow && depth != 0 && delimiters[depth - 1] == expected {
                    depth -= 1;
                }
                previous = Some(byte);
            }
            _ => previous = Some(byte),
        }
    }

    Ok(())
}

fn inspect_operation(
    pointer: Ptr<Operation>,
    context: &mut Context,
) -> Result<(OperationShapeV1, Vec<Ptr<Operation>>), OperationHandleError> {
    let operation = pointer.deref(context);
    let region_count = operation.num_regions();
    if region_count > HARD_MAX_OPERATION_REGIONS {
        return Err(OperationHandleError::TooManyOperationRegions);
    }

    let mut block_count = 0_usize;
    let mut children = Vec::new();
    for region in operation.regions() {
        for block in region.deref(context).iter(context) {
            block_count = block_count
                .checked_add(1)
                .ok_or(OperationHandleError::TooManyOperationBlocks)?;
            if block_count > HARD_MAX_OPERATION_BLOCKS {
                return Err(OperationHandleError::TooManyOperationBlocks);
            }
            for child in block.deref(context).iter(context) {
                if children.len() == HARD_MAX_OPERATION_CHILDREN {
                    return Err(OperationHandleError::TooManyOperationChildren);
                }
                children.push(child);
            }
        }
    }

    Ok((
        OperationShapeV1 {
            operand_count: operation.get_num_operands(),
            result_count: operation.get_num_results(),
            region_count,
            block_count,
            child_operation_count: children.len(),
        },
        children,
    ))
}

fn inspect_operation_tree(
    pointer: Ptr<Operation>,
    context: &mut Context,
) -> Result<usize, OperationHandleError> {
    inspect_operation_tree_details(pointer, context).map(|(work, _)| work)
}

fn inspect_operation_tree_details(
    pointer: Ptr<Operation>,
    context: &mut Context,
) -> Result<(usize, Vec<Ptr<Operation>>), OperationHandleError> {
    let mut pending = vec![pointer];
    let mut operations = Vec::new();
    let mut work = 0_usize;
    while let Some(operation) = pending.pop() {
        let (shape, children) = inspect_operation(operation, context)?;
        let local_work = 1_usize
            .checked_add(shape.region_count)
            .and_then(|value| value.checked_add(shape.block_count))
            .and_then(|value| value.checked_add(children.len()))
            .ok_or(OperationHandleError::OperationTreeLimitExceeded)?;
        work = work
            .checked_add(local_work)
            .filter(|work| *work <= HARD_MAX_OPERATION_TREE_ITEMS)
            .ok_or(OperationHandleError::OperationTreeLimitExceeded)?;
        operations.push(operation);
        pending.extend(children.into_iter().rev());
    }
    Ok((work, operations))
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
    use pliron::builtin::op_interfaces::SingleBlockRegionInterface;

    fn session() -> PlironSession {
        PlironSession::new(ShellLimits::default(), []).expect("fresh session")
    }

    fn context_identity_marker_key() -> Identifier {
        CONTEXT_IDENTITY_MARKER_KEY
            .try_into()
            .expect("fixed marker key")
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
        let key = context_identity_marker_key();
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
        let key = context_identity_marker_key();
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

    #[test]
    fn typed_graph_inspection_returns_stable_owner_handles() {
        let mut session = session();
        let root = session.create_module("root").expect("root module");
        let child_pointer = session
            .with_operation(&root, |pointer, context| {
                let root = ModuleOp::from_operation(pointer);
                let child = ModuleOp::new(context, "child".try_into().expect("valid name"));
                root.append_operation(context, child.get_operation(), 0);
                child.get_operation()
            })
            .expect("append child");

        assert_eq!(
            session.operation_shape(&root),
            Ok(OperationShapeV1 {
                operand_count: 0,
                result_count: 0,
                region_count: 1,
                block_count: 1,
                child_operation_count: 1,
            })
        );
        assert_eq!(session.operation_is::<ModuleOp>(&root), Ok(true));

        let first = session.operation_children(&root).expect("first traversal");
        let second = session.operation_children(&root).expect("second traversal");
        assert_eq!(first.len(), 1);
        assert!(first[0].identity == second[0].identity);
        assert_eq!(session.operations[&first[0].identity], child_pointer);
        assert_eq!(session.operation_is::<ModuleOp>(&first[0]), Ok(true));
    }

    #[test]
    fn child_handle_allocation_is_preflighted() {
        let mut session = session();
        let root = session.create_module("root").expect("root module");
        session
            .with_operation(&root, |pointer, context| {
                let root = ModuleOp::from_operation(pointer);
                let child = ModuleOp::new(context, "child".try_into().expect("valid name"));
                root.append_operation(context, child.get_operation(), 0);
            })
            .expect("append child");
        session.next_operation_handle = None;

        assert!(matches!(
            session.operation_children(&root),
            Err(OperationHandleError::HandleSpaceExhausted)
        ));
        assert_eq!(session.operations.len(), 1);
        assert!(!session.is_poisoned());
        assert_eq!(session.operation_result_count(&root), Ok(0));
    }

    #[test]
    fn textual_import_returns_only_a_verified_owner_handle() {
        let mut session = session();
        let root = session
            .import_operation_text_v1(
                "builtin.module @imported { ^entry(): builtin.module @child { ^entry(): } }",
            )
            .expect("verified module import");

        assert_eq!(session.operation_is::<ModuleOp>(&root), Ok(true));
        assert_eq!(
            session.operation_shape(&root),
            Ok(OperationShapeV1 {
                operand_count: 0,
                result_count: 0,
                region_count: 1,
                block_count: 1,
                child_operation_count: 1,
            })
        );
        let children = session.operation_children(&root).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(session.operation_is::<ModuleOp>(&children[0]), Ok(true));
        assert_eq!(session.operation_tree_work, 7);

        session.erase_operation(&root).expect("erase imported root");
        assert_eq!(session.operation_tree_work, 0);
        assert_eq!(
            session.operation_shape(&root),
            Err(OperationHandleError::StaleHandle)
        );
        assert_eq!(
            session.operation_shape(&children[0]),
            Err(OperationHandleError::StaleHandle)
        );
    }

    #[test]
    fn descendant_erasure_refunds_subtree_and_parent_edge_work() {
        let mut session = session();
        let root = session
            .import_operation_text_v1(
                "builtin.module @imported { ^entry(): builtin.module @child { ^entry(): } }",
            )
            .expect("verified module import");
        let child = session.operation_children(&root).unwrap().remove(0);
        assert_eq!(session.operation_tree_work, 7);

        session
            .erase_operation(&child)
            .expect("erase child subtree");
        assert_eq!(session.operation_tree_work, 3);
        assert_eq!(session.owned_tree_work.get(&root.identity), Some(&3));
        assert!(session.operation_children(&root).unwrap().is_empty());
        assert_eq!(
            session.operation_shape(&child),
            Err(OperationHandleError::StaleHandle)
        );

        session
            .erase_operation(&root)
            .expect("erase remaining root");
        assert_eq!(session.operation_tree_work, 0);
        assert!(session.owned_tree_work.is_empty());
        assert!(session.operation_roots.is_empty());
    }

    #[test]
    fn textual_import_rejects_trailing_input_and_poisons_partial_context() {
        let mut session = session();

        assert!(matches!(
            session.import_operation_text_v1("builtin.module @imported { ^entry(): } trailing"),
            Err(OperationHandleError::OperationImportRejected)
        ));
        assert!(session.is_poisoned());
        assert!(matches!(
            session.create_module("later"),
            Err(OperationHandleError::SessionPoisoned)
        ));
    }

    #[test]
    fn textual_import_verification_failure_poisons_partial_context() {
        let mut session = session();

        assert!(matches!(
            session.import_operation_text_v1("builtin.module @invalid {}"),
            Err(OperationHandleError::OperationVerificationRejected)
        ));
        assert!(session.is_poisoned());
        assert!(session.operations.is_empty());
    }

    #[test]
    fn textual_import_size_preflight_does_not_mutate_or_poison() {
        let mut session = session();
        let next = session.next_operation_handle;

        assert!(matches!(
            session.import_operation_text_v1(""),
            Err(OperationHandleError::EmptyOperationImport)
        ));
        let oversized = "x".repeat(HARD_MAX_OPERATION_IMPORT_BYTES + 1);
        assert!(matches!(
            session.import_operation_text_v1(&oversized),
            Err(OperationHandleError::OperationImportTooLarge)
        ));
        assert_eq!(session.next_operation_handle, next);
        assert!(session.operations.is_empty());
        assert!(!session.is_poisoned());
    }

    #[test]
    fn textual_import_session_bytes_are_monotonic_and_preflighted() {
        let text = "builtin.module @imported { ^entry(): }";
        let mut session = session();
        let root = session
            .import_operation_text_v1(text)
            .expect("bounded import");
        assert_eq!(session.operation_import_bytes, text.len());

        session.erase_operation(&root).expect("erase imported root");
        assert_eq!(session.operation_import_bytes, text.len());
        session.operation_import_bytes = HARD_MAX_SESSION_OPERATION_IMPORT_BYTES;
        let next = session.next_operation_handle;

        assert!(matches!(
            session.import_operation_text_v1(text),
            Err(OperationHandleError::SessionOperationImportLimitExceeded)
        ));
        assert_eq!(session.next_operation_handle, next);
        assert!(session.operations.is_empty());
        assert!(!session.is_poisoned());
    }

    #[test]
    fn textual_import_nesting_preflight_does_not_allocate_or_poison() {
        let mut session = session();
        let next = session.next_operation_handle;
        let nested = "{".repeat(HARD_MAX_OPERATION_IMPORT_NESTING + 1);

        assert!(matches!(
            session.import_operation_text_v1(&nested),
            Err(OperationHandleError::OperationImportNestingTooDeep)
        ));
        assert_eq!(session.next_operation_handle, next);
        assert!(session.operations.is_empty());
        assert!(!session.is_poisoned());
    }

    #[test]
    fn nesting_preflight_ignores_escaped_quoted_delimiters() {
        let quoted = format!(
            "\"\\\"{}\"",
            "{".repeat(HARD_MAX_OPERATION_IMPORT_NESTING + 1)
        );
        assert_eq!(preflight_operation_import_nesting(&quoted), Ok(()));
    }

    #[test]
    fn nesting_preflight_does_not_treat_function_arrows_as_closers() {
        let nested = "{builtin.function <() -> ()>".repeat(HARD_MAX_OPERATION_IMPORT_NESTING + 1);
        assert_eq!(
            preflight_operation_import_nesting(&nested),
            Err(OperationHandleError::OperationImportNestingTooDeep)
        );

        let mut session = session();
        let imported = session.import_operation_text_v1(
            "builtin.module @root { ^entry(): builtin.func @f: builtin.function <() -> ()> {} }",
        );
        assert!(
            imported.is_ok(),
            "valid function import failed: {imported:?}"
        );
    }

    #[test]
    fn operation_session_budget_rejects_before_allocation() {
        let mut session = session();
        session.operation_tree_work = HARD_MAX_SESSION_OPERATION_TREE_ITEMS;
        let next = session.next_operation_handle;

        assert!(matches!(
            session.create_module("module"),
            Err(OperationHandleError::SessionOperationTreeLimitExceeded)
        ));
        assert!(matches!(
            session.import_operation_text_v1("builtin.module @imported { ^entry(): }"),
            Err(OperationHandleError::SessionOperationTreeLimitExceeded)
        ));
        assert_eq!(session.next_operation_handle, next);
        assert!(session.operations.is_empty());
        assert!(!session.is_poisoned());
    }

    #[test]
    fn operation_tree_budget_rejects_flat_amplification() {
        let mut text = String::from("builtin.module @root { ^entry(): ");
        for index in 0..(HARD_MAX_OPERATION_CHILDREN / 2) {
            use fmt::Write as _;
            if index != 0 {
                text.push_str("; ");
            }
            write!(
                text,
                "builtin.module @m{index} {{ ^entry(): builtin.module @l{index} {{ ^entry(): }} }}"
            )
            .unwrap();
        }
        text.push('}');
        assert!(text.len() <= HARD_MAX_OPERATION_IMPORT_BYTES);

        let mut session = session();
        let result = session.import_operation_text_v1(&text);
        assert!(
            matches!(
                result,
                Err(OperationHandleError::OperationTreeLimitExceeded)
            ),
            "unexpected import result: {result:?}"
        );
        assert!(session.is_poisoned());
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
