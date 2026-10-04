//! Exact owner association inside the retained publication audit, not admission authority.

use super::{
    CompilerGeneratedKernelExpectationV1, WorkerV3VerificationDecisionErrorV1,
    WorkerV3VerificationRequestV1, validate_request_target_lineage,
};
use fe2o3_amd_target::{AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1};
use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
use fe2o3_hsaco_finalize::ContentIdentityV1;
use fe2o3_kernel_analysis::CheckedGfx942FillAnalysisV1;
use fe2o3_verifier::CheckedConditionalFillProgramV1;
use std::{error::Error, fmt};

#[cfg(target_os = "linux")]
mod pending;
#[cfg(target_os = "linux")]
pub use pending::{
    PendingWorkerV3ConditionalFillArtifactV1, WorkerV3ConditionalFillPendingErrorV1,
};
#[cfg(target_os = "linux")]
mod subject;
#[cfg(target_os = "linux")]
pub use subject::InertWorkerV3ConditionalFillSubjectV1;

/// Borrows the request and both checked owners for the duration of its publication audit.
///
/// This cannot escape as `WorkerV3AuditorV1::Evidence`. It neither authenticates the
/// compiler producer nor establishes semantic-to-machine refinement. In particular,
/// it is not a pending executable or an unconditional verification decision.
///
/// ```compile_fail
/// use fe2o3_host::{CheckedWorkerV3ConditionalFillAssociationV1,
///     CompilerGeneratedKernelExpectationV1};
/// fn requires_clone<T: Clone>() {}
/// fn cannot_clone<K: CompilerGeneratedKernelExpectationV1>() {
///     requires_clone::<CheckedWorkerV3ConditionalFillAssociationV1<'static, 'static, K>>();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::CheckedWorkerV3ConditionalFillAssociationV1;
/// fn escape<'a, K>(value: CheckedWorkerV3ConditionalFillAssociationV1<'a, 'a, K>)
///     -> CheckedWorkerV3ConditionalFillAssociationV1<'static, 'static, K> { value }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1,
///     CheckedWorkerV3ConditionalFillAssociationV1, CompilerGeneratedKernelExpectationV1};
/// fn promote<K: CompilerGeneratedKernelExpectationV1>(
///     value: CheckedWorkerV3ConditionalFillAssociationV1<'_, '_, K>,
/// ) -> AuthenticatedWorkerV3ExecutableV1<K> { value }
/// ```
#[must_use]
pub struct CheckedWorkerV3ConditionalFillAssociationV1<'check, 'admission, K> {
    request: &'check WorkerV3VerificationRequestV1<'admission, K>,
    program: &'check CheckedConditionalFillProgramV1<'check>,
    machine: &'check CheckedGfx942FillAnalysisV1<'check>,
}

impl<'check, 'admission, K> CheckedWorkerV3ConditionalFillAssociationV1<'check, 'admission, K> {
    pub const fn request(&self) -> &'check WorkerV3VerificationRequestV1<'admission, K> {
        self.request
    }

    pub const fn program(&self) -> &'check CheckedConditionalFillProgramV1<'check> {
        self.program
    }

    pub const fn machine(&self) -> &'check CheckedGfx942FillAnalysisV1<'check> {
        self.machine
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<'admission, K: CompilerGeneratedKernelExpectationV1>
    WorkerV3VerificationRequestV1<'admission, K>
{
    /// Associates exact conditional compiler and analyzed machine owners with this request.
    ///
    /// The caller must still establish protected compiler origin and the universal
    /// fill-refinement contract. Actual invocation coverage, prepared memory/device
    /// binding and completion are not checked here. Existing unconditional admission
    /// does not accept this result.
    pub fn check_conditional_fill_analysis_v1<'check>(
        &'check self,
        program: &'check CheckedConditionalFillProgramV1<'check>,
        machine: &'check CheckedGfx942FillAnalysisV1<'check>,
    ) -> Result<
        CheckedWorkerV3ConditionalFillAssociationV1<'check, 'admission, K>,
        WorkerV3ConditionalFillAssociationErrorV1,
    > {
        use WorkerV3ConditionalFillAssociationErrorV1 as E;
        let receipts = self.handoff.capsule().receipts();
        let inputs = program.inputs();
        for (matches, field) in [
            (
                inputs.association().canonical_bytes()
                    == receipts.proof_binding().canonical_preimage(),
                "proof binding",
            ),
            (
                inputs.verus_execution().canonical_bytes()
                    == inputs.association().verus_execution_evidence(),
                "conditional Verus evidence",
            ),
            (
                inputs.semantic_mir().canonical_encoding()
                    == receipts.semantic_mir().canonical_preimage(),
                "semantic MIR",
            ),
            (
                inputs.middle_end().canonical_bytes() == receipts.middle_end().canonical_preimage(),
                "middle end",
            ),
            (
                inputs.kernel_ir().canonical_bytes() == receipts.kernel_ir().canonical_preimage(),
                "Kernel IR",
            ),
            (
                inputs.exact_correspondence_bytes()
                    == receipts.mir_to_kir_correspondence().canonical_preimage(),
                "MIR-to-KIR correspondence",
            ),
            (
                inputs.formal_memory().canonical_bytes()
                    == receipts.formal_memory().canonical_preimage(),
                "formal memory",
            ),
            (
                inputs.receipt_identity() == receipts.proof_binding().identity(),
                "proof-binding identity",
            ),
        ] {
            if !matches {
                return Err(E::CompilerInputs(field));
            }
        }
        let source = CompilerDescriptorSourceV1::decode(receipts.abi().canonical_preimage())
            .map_err(|_| E::CompilerInputs("canonical compiler ABI"))?;
        let [descriptor] = source.table().kernels() else {
            return Err(E::CompilerInputs("singleton compiler descriptor"));
        };
        if descriptor != self.descriptor() {
            return Err(E::CompilerInputs("selected compiler descriptor"));
        }
        let semantic = inputs.semantic_mir();
        let [root] = semantic.roots() else {
            return Err(E::CompilerInputs("singleton semantic root"));
        };
        let entry = semantic
            .functions()
            .get(root.index() as usize)
            .and_then(|function| function.kernel_entry())
            .ok_or(E::CompilerInputs("semantic entry"))?;
        if entry.kernel_binding_identity().as_bytes() != descriptor.kernel_id().as_bytes() {
            return Err(E::CompilerInputs("semantic kernel binding"));
        }
        validate_request_target_lineage(self, program.lineage()).map_err(E::TargetLineage)?;
        let target = program
            .lineage()
            .target_binding()
            .inputs()
            .map_err(|_| E::Target)?;
        let expected = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
            .expect("the production gfx942 target is valid");
        if self.target() != expected
            || target.configured_target != PRODUCTION_GFX942_DEVICE_TARGET_V1
            || target.wave_width_bits != 64
        {
            return Err(E::Target);
        }
        let bytes = machine.kernel().code_object();
        if bytes != self.finalized_hsaco_bytes() {
            return Err(E::Machine("finalized payload"));
        }
        if ContentIdentityV1::calculate(bytes)
            != self.finalizer_derivation().finalized_hsaco_identity()
        {
            return Err(E::Machine("finalizer identity"));
        }
        if machine.kernel().binding() != self.descriptor_binding() {
            return Err(E::Machine("selected descriptor"));
        }
        if program.function_symbol() != self.descriptor().entry_name().as_str()
            || machine.entry_symbol() != program.function_symbol()
        {
            return Err(E::Machine("entry symbol"));
        }
        Ok(CheckedWorkerV3ConditionalFillAssociationV1 {
            request: self,
            program,
            machine,
        })
    }
}

#[derive(Debug)]
pub enum WorkerV3ConditionalFillAssociationErrorV1 {
    CompilerInputs(&'static str),
    TargetLineage(WorkerV3VerificationDecisionErrorV1),
    Target,
    Machine(&'static str),
}

impl fmt::Display for WorkerV3ConditionalFillAssociationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional fill request association rejected: {self:?}")
    }
}

impl Error for WorkerV3ConditionalFillAssociationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TargetLineage(error) => Some(error),
            _ => None,
        }
    }
}
