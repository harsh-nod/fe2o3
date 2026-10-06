//! Source-owned V18 optimizer and original-MIR refinement statements.
//!
//! The modeled boundary is N -> O, not Rust/MIR -> N. Unchanged operations use
//! one shared interpretation; calls, allocation and device semantics are not
//! proved by replacing them with that interpretation. A generated statement is
//! not an executed theorem, a signed receipt, or Worker authority.
//! The separate V30 original-MIR child independently models the original source
//! and canonical endpoint; optimizer equations do not establish that boundary.

use std::{fmt, mem::size_of};

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV18 as Inventory,
    CanonicalKirTransitionErrorV1, check_canonical_kir_transition_v18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrIdentityV18,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedOutputHandoffV26 as Handoff,
    ProductionSourceOwnedViewErrorV18 as SourceError, ProductionSourceOwnedViewV18 as Source,
};
use sha2::{Digest as _, Sha256};

use crate::{
    CanonicalGeneratedVerusProofInputV3, GeneratedVerusProofInputErrorV3,
    MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3,
};

#[path = "mixed_optimizer_aggregate_refinement_v30.rs"]
mod aggregate_v30;
#[path = "original_semantic_mir_refinement_v30.rs"]
mod original_mir_v30;
pub use original_mir_v30::{
    OriginalSemanticMirRefinementSubjectV30, OriginalSemanticMirRefinementSubjectV31,
    OriginalSemanticMirRefinementSubjectV36, PreparedOriginalSemanticMirRefinementV30,
    PreparedOriginalSemanticMirRefinementV31, PreparedOriginalSemanticMirRefinementV36,
    prepare_original_semantic_mir_refinement_v30, prepare_original_semantic_mir_refinement_v31,
    prepare_original_semantic_mir_refinement_v36,
};
#[path = "mixed_optimizer_cfg_refinement_v27.rs"]
mod cfg_v27;
pub use aggregate_v30::{
    AggregateMemoryCfgObligationV30, AggregateMemoryCfgSubjectV30,
    prepare_aggregate_memory_cfg_obligation_v30,
};
#[path = "mixed_optimizer_receipt_v26.rs"]
mod receipt;
#[path = "mixed_optimizer_relocation_plan_v28.rs"]
mod relocation_plan_v28;
pub use relocation_plan_v28::{
    ExecutedMixedComposedRefinementV29, ExecutedPredicatedTypedSourceTailV90,
    ExecutedTypedSourceTailV50, InertPredicatedTypedSourceReceiptV90, InertTypedSourceReceiptV53,
    MixedOptimizerRelocationCfgSubjectV28, MixedOptimizerRelocationErrorV28,
    MixedOptimizerRelocationSubjectV28, PredicatedTypedSourceTailSubjectV90,
    PreparedMixedComposedExecutionV29, PreparedMixedComposedRelocationCfgRefinementV28,
    PreparedMixedFixedpointComposedRelocationCfgRefinementV29,
    PreparedMixedFixedpointRelocationCfgRefinementV29,
    PreparedMixedFixedpointRelocationExpressionsV29, PreparedMixedRelocationCfgRefinementV28,
    PreparedMixedRelocationExpressionsV28, PreparedPredicatedTypedSourceTailExecutionV90,
    PreparedPredicatedTypedSourceTailV90, PreparedTypedSourceTailExecutionV50,
    PreparedTypedSourceTailV50, SourceScalarReferenceInputV69, TypedSourceTailSubjectV50,
    check_inert_predicated_typed_source_receipt_v90, check_inert_typed_source_receipt_v53,
    prepare_mixed_fixedpoint_relocation_expressions_v29, prepare_mixed_relocation_expressions_v28,
    prepare_predicated_typed_source_tail_v90,
    prepare_predicated_typed_source_tail_with_references_v90, prepare_typed_source_tail_v50,
    prepare_typed_source_tail_with_references_v69,
};
#[cfg(test)]
#[path = "mixed_optimizer_memory_error_v52_tests.rs"]
mod memory_error_v52_tests;
#[path = "mixed_optimizer_semantics_v26.rs"]
mod semantics;
pub use cfg_v27::{
    ExecutedMixedPureCseCfgRefinementV27, ExecutedMixedWorklistCfgRefinementV27,
    MixedOptimizerCfgSubjectV27, PreparedMixedPureCseCfgRefinementV27,
    PreparedMixedWorklistCfgRefinementV27, execute_mixed_pure_cse_cfg_refinement_v27,
    execute_mixed_worklist_cfg_refinement_v27, prepare_mixed_pure_cse_cfg_refinement_v27,
    prepare_mixed_worklist_cfg_refinement_v27,
};
pub use receipt::{
    ExecutedMixedOptimizerBlockSimulationV26, execute_mixed_optimizer_block_simulation_v26,
};
pub use semantics::original_scalar_v30::{
    ExpandedSupportCallKindV280, ExpandedSupportCallV280, ExpandedSupportCensusV280,
    ExpandedSupportCursorV280, ExpandedSupportCutV280, ExpandedSupportInstanceV280,
    ExpandedSupportModelV280, ExpandedSupportRootV280, ExpandedSupportRuntimeV280,
    with_expanded_support_model_v280,
};

const DOMAIN: &[u8] = b"FE2O3/V18/POLICY9/SHARED-OPERATOR-BLOCK-SIMULATION/V26\0";
const SOURCE_LIMIT: usize = MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3;

/// These subjects come only from the retained source and consuming Policy9
/// output. Copying them does not import a proof or authenticate compiler origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedOptimizerRefinementSubjectV26 {
    input: VerifiedCanonicalKernelIrIdentityV18,
    output: VerifiedCanonicalKernelIrIdentityV18,
    statement: [u8; 32],
    blocks: usize,
}
impl MixedOptimizerRefinementSubjectV26 {
    pub const fn input(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.input
    }
    pub const fn output(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.output
    }
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    pub const fn modeled_blocks(self) -> usize {
        self.blocks
    }
}

#[derive(Debug)]
pub enum MixedOptimizerRefinementErrorV26 {
    Source(SourceError),
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    /// Exact-owner MemorySSA analysis or query failed, retaining its cause.
    MemorySsa(fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1),
    /// Exact-owner physical byte geometry or initialization analysis failed.
    PrivateMemory(fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1),
    /// Bounded control-flow analysis failed while generating CFG obligations.
    Flow(fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1),
    Transition(CanonicalKirTransitionErrorV1),
    Generated(GeneratedVerusProofInputErrorV3),
    Runtime(crate::FunctionalRefinementRuntimeErrorV1),
    Execution(crate::FunctionalRefinementVerusExecutionErrorV2),
    Receipt(&'static str),
    Statement(&'static str),
    /// The unchanged bounded source writer refused an emission section.
    GeneratedSourceLimit {
        /// Innermost named emission section that reached the limit.
        section: &'static str,
        /// Bytes already accepted by the bounded writer.
        emitted_bytes: usize,
        /// Unchanged global generated proof source limit.
        limit_bytes: usize,
    },
    /// A refused original MIR statement, with exact static source coordinates.
    SourceStatement {
        /// Source root ordinal.
        root: usize,
        /// Original static call-instance ordinal within the root.
        instance: usize,
        /// Original semantic function ordinal.
        function: usize,
        /// Original block ordinal within that declaration.
        block: usize,
        /// Original statement ordinal within that block.
        statement: usize,
        /// Exhaustively classified original statement or rvalue kind.
        kind: &'static str,
        /// Assignment result TypeId and its original nominal shape, when present.
        result_type: Option<(u32, &'static str)>,
        /// Unchanged diagnostic from the refusing source interpreter.
        reason: &'static str,
    },
}
type Error = MixedOptimizerRefinementErrorV26;
type Result<T> = std::result::Result<T, Error>;
impl From<SourceError> for Error {
    fn from(value: SourceError) -> Self {
        Self::Source(value)
    }
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalKirInventoryErrorV1> for Error {
    fn from(value: CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(value)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1> for Error {
    fn from(value: fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1) -> Self {
        Self::MemorySsa(value)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1> for Error {
    fn from(value: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1) -> Self {
        Self::PrivateMemory(value)
    }
}
impl From<fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1> for Error {
    fn from(value: fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1) -> Self {
        Self::Flow(value)
    }
}
impl From<CanonicalKirTransitionErrorV1> for Error {
    fn from(value: CanonicalKirTransitionErrorV1) -> Self {
        Self::Transition(value)
    }
}
impl From<GeneratedVerusProofInputErrorV3> for Error {
    fn from(value: GeneratedVerusProofInputErrorV3) -> Self {
        Self::Generated(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "mixed optimizer refinement: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::MemorySsa(error) => Some(error),
            Self::PrivateMemory(error) => Some(error),
            Self::Flow(error) => Some(error),
            Self::Transition(error) => Some(error),
            Self::Generated(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::Execution(error) => Some(error),
            Self::Statement(_)
            | Self::Receipt(_)
            | Self::SourceStatement { .. }
            | Self::GeneratedSourceLimit { .. } => None,
        }
    }
}

/// Move-only compiler-generated request borrowing both exact subjects. There
/// is intentionally no constructor accepting text, identities or row reports.
#[must_use = "retain or explicitly settle the source-bound generated request"]
pub struct PreparedMixedOptimizerRefinementV26<'handoff, 'view, 'source> {
    source: &'handoff Source<'source>,
    handoff: &'handoff Handoff<'view, 'source>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: MixedOptimizerRefinementSubjectV26,
    retained: usize,
    required: usize,
}
impl PreparedMixedOptimizerRefinementV26<'_, '_, '_> {
    pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerRefinementSubjectV26> {
        self.check(budget)?;
        Ok(self.subject)
    }
    pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(self.generated.source())
    }
    pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.source.check_query_v18(budget)?;
        self.handoff
            .observe_retained_storage_v18(self.required, budget)?;
        Ok(())
    }
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self
            .handoff
            .observe_retained_storage_v18(self.required, budget);
        let Self {
            generated,
            retained,
            source,
            ..
        } = self;
        drop(generated);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| source.retain_query_resource_error_v18(error))
        });
        checked?;
        settled?;
        Ok(())
    }
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Derive a bounded, exact-graph statement after replaying the independently
/// checked occurrence relation. All temporary graph indexes die before their
/// scratch credit is settled. Accepted work and first denials are never reset.
pub fn prepare_mixed_optimizer_refinement_v26<'handoff, 'view, 'source>(
    source: &'handoff Source<'source>,
    handoff: &'handoff Handoff<'view, 'source>,
    budget: &mut Budget<'_>,
) -> Result<PreparedMixedOptimizerRefinementV26<'handoff, 'view, 'source>> {
    source.check_query_v18(budget)?;
    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let input_owner = source.canonical(budget)?;
        let optimized = handoff.output(budget)?;
        let bytes = input_owner.canonical_bytes();
        budget.charge_work(
            bytes
                .len()
                .checked_add(optimized.input_audit_bytes().len())
                .ok_or(Resource::Accounting)?,
        )?;
        if bytes != optimized.input_audit_bytes() {
            return Err(Error::Statement("exact original optimizer audit bytes"));
        }
        // Source String and boxed canonical input can coexist during conversion.
        budget.reserve_storage(
            2 * SOURCE_LIMIT + size_of::<PreparedMixedOptimizerRefinementV26<'_, '_, '_>>(),
        )?;
        let (input, input_storage) = Inventory::derive_v18(input_owner, budget)?;
        budget.reserve_storage(input_storage.retained_storage())?;
        let (output, output_storage) = Inventory::derive_v18(optimized.owner(), budget)?;
        budget.reserve_storage(output_storage.retained_storage())?;
        let (transition, checked_storage) = check_canonical_kir_transition_v18(
            &input,
            &output,
            optimized.occurrences().candidate(),
            budget,
        )?;
        budget.reserve_storage(checked_storage.retained_storage())?;
        let mut writer = Writer::new(budget)?;
        let blocks = semantics::generate(&input, &output, transition.rows(), &mut writer)?;
        let text = writer.finish()?;
        budget.charge_work(text.len().checked_mul(3).ok_or(Resource::Accounting)?)?;
        let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())?;
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        for owner in [input_owner, optimized.owner()] {
            digest.update(owner.identity().canonical_length().to_le_bytes());
            digest.update(owner.identity().digest());
        }
        digest.update(generated.identity().as_bytes());
        let subject = MixedOptimizerRefinementSubjectV26 {
            input: *input_owner.identity(),
            output: *optimized.owner().identity(),
            statement: digest.finalize().into(),
            blocks,
        };
        drop(transition);
        drop(output);
        drop(input);
        budget.release_storage(
            input_storage
                .retained_storage()
                .checked_add(output_storage.retained_storage())
                .and_then(|n| n.checked_add(checked_storage.retained_storage()))
                .ok_or(Resource::Accounting)?,
        )?;
        source.check_query_v18(budget)?;
        handoff.output(budget)?;
        Ok((generated, subject))
    }));
    match result {
        Ok(Ok((generated, subject))) => {
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok(PreparedMixedOptimizerRefinementV26 {
                source,
                handoff,
                generated,
                subject,
                retained,
                required: budget.storage(),
            })
        }
        other => {
            // The closure has dropped every producer-owned temporary already.
            if handoff.observe_retained_storage_v18(floor, budget).is_ok() {
                let release = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?;
                let _ = budget
                    .release_storage(release)
                    .map_err(|error| source.retain_query_resource_error_v18(error));
            }
            match other {
                Ok(Err(error)) => {
                    let resource = match &error {
                        Error::Resource(error)
                        | Error::Inventory(CanonicalKirInventoryErrorV1::Resource(error))
                        | Error::PrivateMemory(
                            fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Resource(
                                error,
                            )
                            | fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Inventory(
                                CanonicalKirInventoryErrorV1::Resource(error),
                            ),
                        )
                        | Error::Transition(CanonicalKirTransitionErrorV1::Resource(error)) => {
                            Some(*error)
                        }
                        _ => None,
                    };
                    if let Some(error) = resource {
                        Err(source.retain_query_resource_error_v18(error).into())
                    } else {
                        Err(error)
                    }
                }
                Err(payload) => std::panic::resume_unwind(payload),
                _ => unreachable!(),
            }
        }
    }
}

struct Writer<'a, 'work> {
    text: String,
    budget: &'a mut Budget<'work>,
    failure: Option<Resource>,
}
impl<'a, 'work> Writer<'a, 'work> {
    fn new(budget: &'a mut Budget<'work>) -> Result<Self> {
        let mut text = String::new();
        text.try_reserve_exact(SOURCE_LIMIT)
            .map_err(|_| Resource::Allocation)?;
        if text.capacity() > SOURCE_LIMIT {
            budget.reserve_storage(text.capacity() - SOURCE_LIMIT)?;
        }
        Ok(Self {
            text,
            budget,
            failure: None,
        })
    }
    fn error(&mut self) -> Error {
        self.failure
            .take()
            .map(Error::Resource)
            .unwrap_or(Error::Statement("generated source limit"))
    }
    fn source_section_error(&self, error: Error, section: &'static str) -> Error {
        match error {
            Error::Statement("generated source limit") => Error::GeneratedSourceLimit {
                section,
                emitted_bytes: self.text.len(),
                limit_bytes: SOURCE_LIMIT,
            },
            error => error,
        }
    }
    fn finish(self) -> Result<String> {
        match self.failure {
            Some(error) => Err(error.into()),
            None => Ok(self.text),
        }
    }
}
impl fmt::Write for Writer<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.failure.is_some() {
            return Err(fmt::Error);
        }
        if let Err(error) = self.budget.charge_work(text.len()) {
            self.failure = Some(error);
            return Err(fmt::Error);
        }
        if text.len() > SOURCE_LIMIT - self.text.len() {
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
