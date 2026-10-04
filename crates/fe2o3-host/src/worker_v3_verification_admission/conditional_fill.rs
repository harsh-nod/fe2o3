//! Exact owner association inside the retained publication audit, not admission authority.

use super::{
    CompilerGeneratedKernelExpectationV1, WorkerV3VerificationDecisionErrorV1,
    WorkerV3VerificationRequestV1, validate_artifact_target_lineage,
};
use fe2o3_amd_target::{AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1};
use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, InertSemanticCompilerModuleHandoffV3};
use fe2o3_hsaco::{CodeObjectVersion, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{ContentIdentityV1, RevalidatedProtectedWorkerV3FinalizerDerivationV1};
use fe2o3_kernel_analysis::CheckedGfx942FillAnalysisV1;
use fe2o3_kernel_descriptor::KernelDescriptorV1;
use fe2o3_verifier::CheckedConditionalFillProgramV1;
use std::{error::Error, fmt};

mod contract;
pub use contract::derive_worker_v3_conditional_fill_host_contract_v1;

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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod remote;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use remote::{
    RemoteConditionalFillArtifactV1, WorkerV3ConditionalFillInvocationErrorV1,
    WorkerV3RemoteConditionalFillErrorV1,
};

#[cfg(target_os = "linux")]
mod retained;
#[cfg(target_os = "linux")]
pub use retained::{
    RetainedWorkerV3ConditionalFillProofV1, WorkerV3ConditionalFillRetainedErrorV1,
    execute_retained_worker_v3_conditional_fill_v1,
};

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
    generated_host_contract: [u8; 32],
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

    /// Independently derived contract; the marker's declaration is not trusted here.
    pub const fn generated_host_contract_identity(&self) -> [u8; 32] {
        self.generated_host_contract
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
        let generated_host_contract =
            ConditionalFillArtifactView::from_request(self).check(program, machine)?;
        Ok(CheckedWorkerV3ConditionalFillAssociationV1 {
            request: self,
            program,
            machine,
            generated_host_contract,
        })
    }
}

/// Common immutable inputs; neither construction path manufactures publication custody.
struct ConditionalFillArtifactView<'evidence> {
    handoff: &'evidence InertSemanticCompilerModuleHandoffV3,
    finalizer: &'evidence RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    finalized: &'evidence [u8],
    descriptor: &'evidence KernelDescriptorV1,
    binding: KernelDescriptorBinding,
    target: AmdTargetId,
    code_object_version: CodeObjectVersion,
}

impl<'evidence> ConditionalFillArtifactView<'evidence> {
    fn from_request<K: CompilerGeneratedKernelExpectationV1>(
        request: &'evidence WorkerV3VerificationRequestV1<'_, K>,
    ) -> Self {
        Self {
            handoff: request.handoff,
            finalizer: request.finalizer_derivation(),
            finalized: request.finalized_hsaco_bytes(),
            descriptor: request.descriptor(),
            binding: request.descriptor_binding(),
            target: request.target(),
            code_object_version: request.code_object_version(),
        }
    }

    #[cfg(target_os = "linux")]
    fn from_closure(closure: &'evidence crate::CheckedWorkerV3CompilerClosureV1<'_>) -> Self {
        Self {
            handoff: closure.semantic_compiler_handoff(),
            finalizer: closure.finalizer_derivation(),
            finalized: closure.finalized_hsaco_bytes(),
            descriptor: closure.descriptor(),
            binding: closure.descriptor_binding(),
            target: closure.target(),
            code_object_version: closure.code_object_version(),
        }
    }

    fn check(
        &self,
        program: &CheckedConditionalFillProgramV1<'_>,
        machine: &CheckedGfx942FillAnalysisV1<'_>,
    ) -> Result<[u8; 32], WorkerV3ConditionalFillAssociationErrorV1> {
        use WorkerV3ConditionalFillAssociationErrorV1 as E;
        let source = self.check_program(program)?;
        let bytes = machine.kernel().code_object();
        if bytes != self.finalized {
            return Err(E::Machine("finalized payload"));
        }
        if ContentIdentityV1::calculate(bytes) != self.finalizer.finalized_hsaco_identity() {
            return Err(E::Machine("finalizer identity"));
        }
        if machine.kernel().binding() != self.binding {
            return Err(E::Machine("selected descriptor"));
        }
        if program.function_symbol() != self.descriptor.entry_name().as_str()
            || machine.entry_symbol() != program.function_symbol()
        {
            return Err(E::Machine("entry symbol"));
        }
        derive_worker_v3_conditional_fill_host_contract_v1(program, &source)
    }

    fn check_program(
        &self,
        program: &CheckedConditionalFillProgramV1<'_>,
    ) -> Result<CompilerDescriptorSourceV1, WorkerV3ConditionalFillAssociationErrorV1> {
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
        if descriptor != self.descriptor {
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
        validate_artifact_target_lineage(
            self.handoff,
            self.finalizer,
            self.descriptor,
            self.code_object_version,
            program.lineage(),
        )
        .map_err(E::TargetLineage)?;
        let target = program
            .lineage()
            .target_binding()
            .inputs()
            .map_err(|_| E::Target)?;
        let expected = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
            .expect("the production gfx942 target is valid");
        if self.target != expected
            || target.configured_target != PRODUCTION_GFX942_DEVICE_TARGET_V1
            || target.wave_width_bits != 64
        {
            return Err(E::Target);
        }
        Ok(source)
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
