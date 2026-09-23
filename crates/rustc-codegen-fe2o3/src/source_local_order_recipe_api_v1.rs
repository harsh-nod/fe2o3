//! Linux bounded in-process author entry. All requests/results are inert.
//! Recipe bytes are an owned snapshot; their file custody belongs to the caller.
//! Source-file custody remains live and compiler-associated through the callback.
//! No callback, owner, executable, pass selector, publication or launch authority
//! is part of this public interface. Canonical order is not machine scheduling.
use crate::source_local_order_recipe_v1 as codec;
use fe2o3_source_isa_observation::source_edit_v1::validate_source_edit_path_v1;
use std::mem::size_of;

pub(crate) const LLVM_BYTE_CAP: usize = 48 * 1024;
const DIAGNOSTIC_BYTE_CAP: usize = 4096;
pub(crate) const DESCRIPTOR_PRODUCER: &str = "source-local-order-policy6-v1/gfx942";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderOrderV1 {
    SourceOrder,
    ReverseReady,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderRelationV1 {
    XorBeforeOr,
    OrBeforeXor,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderStrengthV1 {
    Exact,
    Advisory,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderSourceBindingModeV1 {
    ExactRevision,
    RebindCurrent,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderConstraintOutcomeV1 {
    Honored {
        relation: SourceLocalOrderRelationV1,
    },
    NotHonored {
        requested: SourceLocalOrderRelationV1,
        actual: SourceLocalOrderRelationV1,
    },
}
impl From<SourceLocalOrderOrderV1> for codec::Order {
    fn from(value: SourceLocalOrderOrderV1) -> Self {
        match value {
            SourceLocalOrderOrderV1::SourceOrder => Self::SourceOrder,
            SourceLocalOrderOrderV1::ReverseReady => Self::ReverseReady,
        }
    }
}
impl From<codec::Order> for SourceLocalOrderOrderV1 {
    fn from(value: codec::Order) -> Self {
        match value {
            codec::Order::SourceOrder => Self::SourceOrder,
            codec::Order::ReverseReady => Self::ReverseReady,
        }
    }
}
impl From<SourceLocalOrderRelationV1> for codec::Relation {
    fn from(value: SourceLocalOrderRelationV1) -> Self {
        match value {
            SourceLocalOrderRelationV1::XorBeforeOr => Self::XorBeforeOr,
            SourceLocalOrderRelationV1::OrBeforeXor => Self::OrBeforeXor,
        }
    }
}
impl From<codec::Relation> for SourceLocalOrderRelationV1 {
    fn from(value: codec::Relation) -> Self {
        match value {
            codec::Relation::XorBeforeOr => Self::XorBeforeOr,
            codec::Relation::OrBeforeXor => Self::OrBeforeXor,
        }
    }
}
impl From<SourceLocalOrderStrengthV1> for codec::Strength {
    fn from(value: SourceLocalOrderStrengthV1) -> Self {
        match value {
            SourceLocalOrderStrengthV1::Exact => Self::Exact,
            SourceLocalOrderStrengthV1::Advisory => Self::Advisory,
        }
    }
}
impl From<codec::Strength> for SourceLocalOrderStrengthV1 {
    fn from(value: codec::Strength) -> Self {
        match value {
            codec::Strength::Exact => Self::Exact,
            codec::Strength::Advisory => Self::Advisory,
        }
    }
}
impl From<codec::ConstraintOutcome> for SourceLocalOrderConstraintOutcomeV1 {
    fn from(value: codec::ConstraintOutcome) -> Self {
        match value {
            codec::ConstraintOutcome::Honored { relation } => Self::Honored {
                relation: relation.into(),
            },
            codec::ConstraintOutcome::NotHonored { requested, actual } => Self::NotHonored {
                requested: requested.into(),
                actual: actual.into(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderRecipeFailurePhaseV1 {
    Request,
    Frontend,
    Eligibility,
    RecipeBinding,
    Continuation,
    Constraint,
    Observation,
    SourceCurrentness,
}
#[derive(Debug)]
pub struct SourceLocalOrderRecipeFailureV1 {
    phase: SourceLocalOrderRecipeFailurePhaseV1,
    diagnostic: String,
    compiler_fatal: bool,
}
impl std::fmt::Display for SourceLocalOrderRecipeFailureV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.diagnostic)
    }
}
impl std::error::Error for SourceLocalOrderRecipeFailureV1 {}
impl SourceLocalOrderRecipeFailureV1 {
    pub const fn phase(&self) -> SourceLocalOrderRecipeFailurePhaseV1 {
        self.phase
    }
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
    pub const fn compiler_fatal(&self) -> bool {
        self.compiler_fatal
    }
    pub(crate) fn new(phase: SourceLocalOrderRecipeFailurePhaseV1, mut diagnostic: String) -> Self {
        if diagnostic.len() > DIAGNOSTIC_BYTE_CAP {
            let mut end = DIAGNOSTIC_BYTE_CAP;
            while !diagnostic.is_char_boundary(end) {
                end -= 1;
            }
            diagnostic.truncate(end);
        }
        Self {
            phase,
            diagnostic,
            compiler_fatal: false,
        }
    }
}

// Keep the bounded decoded recipe inline: boxing would introduce an unmetered
// allocation solely to shrink the small Create variant.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Intent {
    Create {
        preference: codec::Order,
        constraint: codec::Constraint,
        source_binding: SourceLocalOrderSourceBindingModeV1,
    },
    Replay {
        bytes: Vec<u8>,
        recipe: codec::Recipe,
    },
}

/// One inert request. Source paths use the existing bounded relative-path
/// profile. The expected current file SHA is always a conflict predicate only.
/// Replay accepts a byte snapshot, not a recipe path or file-currentness claim.
pub struct SourceLocalOrderRecipeRequestV1 {
    source: String,
    expected_current_source_sha256: [u8; 32],
    intent: Intent,
    retained_storage: usize,
}
impl SourceLocalOrderRecipeRequestV1 {
    pub fn create(
        source: &str,
        expected_current_source_sha256: [u8; 32],
        preference: SourceLocalOrderOrderV1,
        relation: SourceLocalOrderRelationV1,
        strength: SourceLocalOrderStrengthV1,
        source_binding: SourceLocalOrderSourceBindingModeV1,
    ) -> Result<Self, SourceLocalOrderRecipeFailureV1> {
        Self::new(
            source,
            expected_current_source_sha256,
            Intent::Create {
                preference: preference.into(),
                constraint: codec::Constraint {
                    relation: relation.into(),
                    strength: strength.into(),
                },
                source_binding,
            },
        )
    }

    pub fn replay(
        source: &str,
        expected_current_source_sha256: [u8; 32],
        recipe_bytes: &[u8],
    ) -> Result<Self, SourceLocalOrderRecipeFailureV1> {
        let recipe = codec::Recipe::decode(recipe_bytes)
            .map_err(|error| failure(SourceLocalOrderRecipeFailurePhaseV1::Request, error))?;
        let bytes = copy_bytes(recipe_bytes, codec::BYTE_CAP).map_err(|message| {
            SourceLocalOrderRecipeFailureV1::new(
                SourceLocalOrderRecipeFailurePhaseV1::Request,
                message,
            )
        })?;
        Self::new(
            source,
            expected_current_source_sha256,
            Intent::Replay { bytes, recipe },
        )
    }

    fn new(
        source: &str,
        expected_current_source_sha256: [u8; 32],
        intent: Intent,
    ) -> Result<Self, SourceLocalOrderRecipeFailureV1> {
        let phase = SourceLocalOrderRecipeFailurePhaseV1::Request;
        validate_source_edit_path_v1(source).map_err(|error| failure(phase, error))?;
        let bytes = copy_bytes(source.as_bytes(), 1024)
            .map_err(|message| SourceLocalOrderRecipeFailureV1::new(phase, message))?;
        let source = String::from_utf8(bytes).map_err(|error| failure(phase, error))?;
        let recipe_capacity = match &intent {
            Intent::Create { .. } => 0,
            Intent::Replay { bytes, .. } => bytes.capacity(),
        };
        let retained_storage = size_of::<Self>()
            .checked_add(source.capacity())
            .and_then(|size| size.checked_add(recipe_capacity))
            .ok_or_else(|| {
                SourceLocalOrderRecipeFailureV1::new(
                    phase,
                    "local-order request accounting overflow".into(),
                )
            })?;
        Ok(Self {
            source,
            expected_current_source_sha256,
            intent,
            retained_storage,
        })
    }

    pub fn source_path(&self) -> &str {
        &self.source
    }
    pub const fn expected_current_source_sha256(&self) -> &[u8; 32] {
        &self.expected_current_source_sha256
    }
    pub const fn is_create(&self) -> bool {
        matches!(self.intent, Intent::Create { .. })
    }
    /// Exact logical owned header plus actual String/Vec capacities, not RSS.
    /// The request is moved into/out of the attempt, never deep-cloned.
    pub const fn retained_input_storage(&self) -> usize {
        self.retained_storage
    }
    pub fn replay_recipe_bytes(&self) -> Option<&[u8]> {
        match &self.intent {
            Intent::Create { .. } => None,
            Intent::Replay { bytes, .. } => Some(bytes),
        }
    }
    pub(crate) const fn intent(&self) -> &Intent {
        &self.intent
    }
}

pub(crate) fn copy_bytes(bytes: &[u8], cap: usize) -> Result<Vec<u8>, String> {
    if bytes.is_empty() || bytes.len() > cap {
        return Err("local-order bounded byte copy".into());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes.len())
        .map_err(|_| "local-order bounded allocation failed")?;
    output.extend_from_slice(bytes);
    Ok(output)
}
pub(crate) fn require_current_source_revision(
    expected: &[u8; 32],
    actual: &[u8; 32],
) -> Result<(), SourceLocalOrderRecipeFailureV1> {
    if expected != actual {
        return Err(SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::SourceCurrentness,
            "local-order recipe expected current source revision differs".into(),
        ));
    }
    Ok(())
}
pub(crate) fn failure(
    phase: SourceLocalOrderRecipeFailurePhaseV1,
    error: impl std::fmt::Display,
) -> SourceLocalOrderRecipeFailureV1 {
    SourceLocalOrderRecipeFailureV1::new(phase, error.to_string())
}

/// Inert digest/length observation; cannot reconstruct a verified identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocalOrderIdentityObservationV1 {
    digest: [u8; 32],
    canonical_length: u64,
}
impl SourceLocalOrderIdentityObservationV1 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
    pub(crate) const fn from_current(
        value: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12,
    ) -> Self {
        Self {
            digest: *value.digest(),
            canonical_length: value.canonical_length(),
        }
    }
}

/// Fixed diagnostic observations made from the same checked current N/I/L.
/// Prefix bytes and tail digest/counts are historical observations, not reusable
/// receipts. Formal counts describe compiler-derived obligations, NOT runtime
/// launch geometry, alias authentication, physical resources or proven memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocalOrderRecipeEvidenceV1 {
    pub(crate) source_sha256: [u8; 32],
    pub(crate) source_initializer: [u32; 4],
    pub(crate) semantic_sha256: [u8; 32],
    pub(crate) instance_axes: [[u8; 32]; 5],
    pub(crate) original: SourceLocalOrderIdentityObservationV1,
    pub(crate) input: SourceLocalOrderIdentityObservationV1,
    pub(crate) output: SourceLocalOrderIdentityObservationV1,
    pub(crate) requested_order: SourceLocalOrderOrderV1,
    pub(crate) requested_relation: SourceLocalOrderRelationV1,
    pub(crate) strength: SourceLocalOrderStrengthV1,
    pub(crate) source_binding_mode: SourceLocalOrderSourceBindingModeV1,
    pub(crate) actual_relation: SourceLocalOrderRelationV1,
    pub(crate) constraint_outcome: SourceLocalOrderConstraintOutcomeV1,
    pub(crate) region: [u32; 4],
    pub(crate) output_result_order: [u32; 3],
    pub(crate) prefix_execution_bytes: [u8; 256],
    pub(crate) transition_sha256: [u8; 32],
    pub(crate) transition_bytes: usize,
    pub(crate) transition_rows: [usize; 9],
    pub(crate) fresh_formal_counts: [usize; 5],
    pub(crate) llvm_sha256: [u8; 32],
    pub(crate) descriptor_sha256: [u8; 32],
    pub(crate) recipe_sha256: [u8; 32],
    pub(crate) canonical_work: usize,
    pub(crate) canonical_peak_storage: usize,
    pub(crate) created: bool,
}
impl SourceLocalOrderRecipeEvidenceV1 {
    pub const fn source_sha256(&self) -> &[u8; 32] {
        &self.source_sha256
    }
    pub const fn source_initializer(&self) -> &[u32; 4] {
        &self.source_initializer
    }
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    pub const fn instance_axes(&self) -> &[[u8; 32]; 5] {
        &self.instance_axes
    }
    pub const fn original(&self) -> &SourceLocalOrderIdentityObservationV1 {
        &self.original
    }
    pub const fn input(&self) -> &SourceLocalOrderIdentityObservationV1 {
        &self.input
    }
    pub const fn output(&self) -> &SourceLocalOrderIdentityObservationV1 {
        &self.output
    }
    pub const fn requested_order(&self) -> SourceLocalOrderOrderV1 {
        self.requested_order
    }
    pub const fn requested_relation(&self) -> SourceLocalOrderRelationV1 {
        self.requested_relation
    }
    pub const fn strength(&self) -> SourceLocalOrderStrengthV1 {
        self.strength
    }
    pub const fn source_binding_mode(&self) -> SourceLocalOrderSourceBindingModeV1 {
        self.source_binding_mode
    }
    pub const fn actual_relation(&self) -> SourceLocalOrderRelationV1 {
        self.actual_relation
    }
    pub const fn constraint_outcome(&self) -> SourceLocalOrderConstraintOutcomeV1 {
        self.constraint_outcome
    }
    pub const fn region(&self) -> &[u32; 4] {
        &self.region
    }
    pub const fn output_result_order(&self) -> &[u32; 3] {
        &self.output_result_order
    }
    pub const fn prefix_execution_bytes(&self) -> &[u8; 256] {
        &self.prefix_execution_bytes
    }
    pub const fn transition_sha256(&self) -> &[u8; 32] {
        &self.transition_sha256
    }
    pub const fn transition_bytes(&self) -> usize {
        self.transition_bytes
    }
    pub const fn transition_rows(&self) -> &[usize; 9] {
        &self.transition_rows
    }
    pub const fn fresh_formal_counts(&self) -> &[usize; 5] {
        &self.fresh_formal_counts
    }
    pub const fn llvm_sha256(&self) -> &[u8; 32] {
        &self.llvm_sha256
    }
    pub const fn descriptor_sha256(&self) -> &[u8; 32] {
        &self.descriptor_sha256
    }
    pub const fn recipe_sha256(&self) -> &[u8; 32] {
        &self.recipe_sha256
    }
    pub const fn canonical_work(&self) -> usize {
        self.canonical_work
    }
    pub const fn canonical_peak_storage(&self) -> usize {
        self.canonical_peak_storage
    }
    pub const fn created(&self) -> bool {
        self.created
    }
    pub const fn descriptor_producer(&self) -> &'static str {
        DESCRIPTOR_PRODUCER
    }
    pub const fn composition(&self) -> &'static str {
        codec::COMPOSITION
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Actual L LLVM bytes and inert evidence; no compiler/source/graph owner escapes.
/// Created recipe bytes exist only for explicit Create. Replay never rewrites its
/// original recipe or silently regenerates a rejected binding.
#[derive(Debug)]
pub struct SourceLocalOrderRecipeOutputV1 {
    pub(crate) llvm: String,
    pub(crate) created_recipe: Option<Vec<u8>>,
    pub(crate) evidence: SourceLocalOrderRecipeEvidenceV1,
}
impl SourceLocalOrderRecipeOutputV1 {
    pub fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub fn created_recipe_bytes(&self) -> Option<&[u8]> {
        self.created_recipe.as_deref()
    }
    pub const fn evidence(&self) -> &SourceLocalOrderRecipeEvidenceV1 {
        &self.evidence
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
pub struct SourceLocalOrderRecipeAttemptV1 {
    request: SourceLocalOrderRecipeRequestV1,
    result: Result<SourceLocalOrderRecipeOutputV1, SourceLocalOrderRecipeFailureV1>,
    callback_count: usize,
    compiler_callback_count: usize,
}
impl SourceLocalOrderRecipeAttemptV1 {
    pub const fn request(&self) -> &SourceLocalOrderRecipeRequestV1 {
        &self.request
    }
    pub fn result(
        &self,
    ) -> Result<&SourceLocalOrderRecipeOutputV1, &SourceLocalOrderRecipeFailureV1> {
        self.result.as_ref()
    }
    /// Method invocations; a private test probe can deliberately reenter it.
    pub const fn callback_count(&self) -> usize {
        self.callback_count
    }
    /// Entries issued by rustc, excluding deliberate test-only method reentry.
    pub const fn compiler_callback_count(&self) -> usize {
        self.compiler_callback_count
    }
    pub fn into_parts(
        self,
    ) -> (
        SourceLocalOrderRecipeRequestV1,
        Result<SourceLocalOrderRecipeOutputV1, SourceLocalOrderRecipeFailureV1>,
    ) {
        (self.request, self.result)
    }
    pub(crate) fn new(
        request: SourceLocalOrderRecipeRequestV1,
        result: Result<SourceLocalOrderRecipeOutputV1, SourceLocalOrderRecipeFailureV1>,
        callback_count: usize,
        compiler_callback_count: usize,
    ) -> Self {
        Self {
            request,
            result,
            callback_count,
            compiler_callback_count,
        }
    }
}

pub(crate) fn validate_arguments(args: &[String]) -> Result<(), SourceLocalOrderRecipeFailureV1> {
    let phase = SourceLocalOrderRecipeFailurePhaseV1::Request;
    let refusal = || {
        SourceLocalOrderRecipeFailureV1::new(
            phase,
            "local-order recipe requires bounded complete rustc arguments".into(),
        )
    };
    if args.is_empty() || args.len() > 4096 {
        return Err(refusal());
    }
    let mut bytes = 0usize;
    for arg in args {
        bytes = bytes.checked_add(arg.len()).ok_or_else(refusal)?;
        if bytes > 1024 * 1024 {
            return Err(refusal());
        }
    }
    Ok(())
}
pub(crate) fn finish_callback(
    result: Option<Result<SourceLocalOrderRecipeOutputV1, SourceLocalOrderRecipeFailureV1>>,
    calls: usize,
    fatal: bool,
) -> Result<SourceLocalOrderRecipeOutputV1, SourceLocalOrderRecipeFailureV1> {
    let result = match result {
        Some(Err(mut error)) => {
            error.compiler_fatal |= fatal;
            return Err(error);
        }
        other => other,
    };
    if calls != 1 || fatal {
        let mut error = SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::Frontend,
            if fatal {
                "rustc reported a fatal error during the recipe attempt"
            } else {
                "local-order recipe did not finish exactly one live callback"
            }
            .into(),
        );
        error.compiler_fatal = fatal;
        return Err(error);
    }
    match result {
        Some(Ok(output)) => Ok(output),
        _ => Err(SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::Frontend,
            "local-order recipe callback returned no result".into(),
        )),
    }
}

#[cfg(test)]
#[path = "source_local_order_recipe_test_support_v1.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "source_local_order_recipe_api_v1_tests.rs"]
mod tests;
