use super::*;
use crate::{
    ValidatedCompilerTargetLineageV1, ValidatedConditionalCompilerProofInputsV1,
    check_conditional_fill_program_v1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, check_gfx942_fill_analysis_v1,
};

/// Owns the original compiler and analyzer executions plus their conditional refinement.
///
/// No checked view is stored: each view borrows these owners only while checking or
/// proving. This result can outlive the runtime lease, but it cannot authenticate
/// compiler origin, establish native dispatch premises, or authorize a launch.
///
/// ```
/// use fe2o3_verifier::OwnedConditionalFillRefinementExecutionV1;
/// fn requires_static<T: 'static>() {}
/// requires_static::<OwnedConditionalFillRefinementExecutionV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::OwnedConditionalFillRefinementExecutionV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<OwnedConditionalFillRefinementExecutionV1>();
/// ```
#[derive(Debug)]
#[must_use]
pub struct OwnedConditionalFillRefinementExecutionV1 {
    inputs: ValidatedConditionalCompilerProofInputsV1,
    lineage: ValidatedCompilerTargetLineageV1,
    analysis: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    source: Box<[u8]>,
    obligation: Box<[u8]>,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}

impl OwnedConditionalFillRefinementExecutionV1 {
    pub const fn inputs(&self) -> &ValidatedConditionalCompilerProofInputsV1 {
        &self.inputs
    }
    pub const fn lineage(&self) -> &ValidatedCompilerTargetLineageV1 {
        &self.lineage
    }
    pub const fn analysis_execution(&self) -> &AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        &self.analysis
    }
    pub fn generated_source(&self) -> &[u8] {
        &self.source
    }
    pub fn obligation_preimage(&self) -> &[u8] {
        &self.obligation
    }
    pub const fn boundary(&self) -> FunctionalRefinementBoundaryV2 {
        self.retained.proof().boundary()
    }
    pub const fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.retained.proof().binding()
    }
    pub const fn signed_receipt_wire(&self) -> &[u8] {
        self.retained.wire()
    }
    pub const fn receipt_verifying_key(&self) -> &[u8; 32] {
        self.retained.verifying_key()
    }
    pub const fn retains_strictly_imported_signed_receipt(&self) -> bool {
        self.retained.proof().signature_and_policy_verified()
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Consumes the three original owners, checks their exact association, and executes
/// the generated proof once. Failure also consumes the supplied owners.
///
/// No detached analyzer receipt or caller-supplied proof is accepted.
///
/// ```compile_fail,E0382
/// use fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1;
/// use fe2o3_verifier::{FunctionalRefinementVerusRuntimeLeaseV1,
///     ValidatedConditionalCompilerProofInputsV1, ValidatedCompilerTargetLineageV1,
///     execute_owned_conditional_fill_refinement_v1};
/// fn consumed(runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
///     inputs: ValidatedConditionalCompilerProofInputsV1,
///     lineage: ValidatedCompilerTargetLineageV1,
///     analysis: AuthenticatedPhysicalMachineAnalysisExecutionV1) {
///     let _ = execute_owned_conditional_fill_refinement_v1(
///         runtime, inputs, lineage, analysis, 180);
///     let _ = (&inputs, &lineage, &analysis);
/// }
/// ```
pub fn execute_owned_conditional_fill_refinement_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    inputs: ValidatedConditionalCompilerProofInputsV1,
    lineage: ValidatedCompilerTargetLineageV1,
    analysis: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    timeout_seconds: u32,
) -> Result<OwnedConditionalFillRefinementExecutionV1, ConditionalFillRefinementErrorV1> {
    let (source, obligation, retained) = {
        let program = check_conditional_fill_program_v1(&inputs, &lineage)
            .map_err(ConditionalFillRefinementErrorV1::Program)?;
        let machine = check_gfx942_fill_analysis_v1(&analysis, program.function_symbol())
            .map_err(ConditionalFillRefinementErrorV1::Machine)?;
        let ProductionConditionalFillRefinementExecutionV1 {
            source,
            obligation,
            retained,
            ..
        } = execute_conditional_fill_refinement_v1(runtime, &program, &machine, timeout_seconds)?;
        (source, obligation, retained)
    };
    Ok(OwnedConditionalFillRefinementExecutionV1 {
        inputs,
        lineage,
        analysis,
        source,
        obligation,
        retained,
    })
}
