//! Executed conditional semantic/KIR-to-machine projection, not native launch admission.
//!
//! The closed recognizer and intrinsic/ISA interpretations remain trusted. Native
//! entry registers, actual argument backing, scheduling and completion are premises,
//! not observations made by this proof. Compiler origin is a separate host gate.

use crate::functional_refinement_receipt_v2::{
    RetainedImportedFunctionalRefinementReceiptV2,
    execute_and_import_generated_conditional_fill_composition_locally_v1,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3, CheckedConditionalFillProgramV1,
    FunctionalRefinementVerusExecutionErrorV2, FunctionalRefinementVerusRuntimeLeaseV1,
    GeneratedVerusProofInputErrorV3,
};
use fe2o3_functional_proof::{FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2};
use fe2o3_kernel_analysis::CheckedGfx942FillAnalysisV1;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};
use std::{error::Error, fmt};

mod generate;
mod native;
mod owned;
pub use native::{
    NativeConditionalFillRefinementErrorV1, NativeConditionalFillRefinementExecutionV1,
    NativeConditionalFillRefinementStorageV1, execute_native_conditional_fill_refinement_v1,
};
pub use owned::{
    OwnedConditionalFillRefinementExecutionV1, execute_owned_conditional_fill_refinement_v1,
};

const DOMAIN: &[u8] = b"FE2O3/SEMANTIC-GFX942-FILL-CONDITIONAL-REFINEMENT/V1\0";
const BOUNDARY: FunctionalRefinementBoundaryV2 =
    FunctionalRefinementBoundaryV2::SemanticMirToGfx942FillDispatchConditional;

/// Retains actual proof execution and borrows both exact checked owners.
///
/// The local signing key is not protected compiler-origin attestation. This owner
/// cannot be promoted into an unconditional executable or used as a launch permit.
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionConditionalFillRefinementExecutionV1;
/// fn clone_required<T: Clone>() {}
/// clone_required::<ProductionConditionalFillRefinementExecutionV1<'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionConditionalFillRefinementExecutionV1;
/// fn escape<'a>(value: ProductionConditionalFillRefinementExecutionV1<'a>)
///     -> ProductionConditionalFillRefinementExecutionV1<'static> { value }
/// ```
#[derive(Debug)]
#[must_use]
pub struct ProductionConditionalFillRefinementExecutionV1<'a> {
    program: &'a CheckedConditionalFillProgramV1<'a>,
    machine: &'a CheckedGfx942FillAnalysisV1<'a>,
    source: Box<[u8]>,
    obligation: Box<[u8]>,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}

impl<'a> ProductionConditionalFillRefinementExecutionV1<'a> {
    pub const fn program(&self) -> &'a CheckedConditionalFillProgramV1<'a> {
        self.program
    }
    pub const fn machine(&self) -> &'a CheckedGfx942FillAnalysisV1<'a> {
        self.machine
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

/// Generates expressions from the retained compiler operand graphs and executes
/// the shared machine bodies before strictly importing the distinct proof boundary.
/// No caller-supplied theorem, proof output, receipt, or success flag is accepted.
pub fn execute_conditional_fill_refinement_v1<'a>(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    program: &'a CheckedConditionalFillProgramV1<'a>,
    machine: &'a CheckedGfx942FillAnalysisV1<'a>,
    timeout_seconds: u32,
) -> Result<ProductionConditionalFillRefinementExecutionV1<'a>, ConditionalFillRefinementErrorV1> {
    use ConditionalFillRefinementErrorV1 as E;
    let target = program
        .lineage()
        .target_binding()
        .inputs()
        .map_err(|_| E::Profile)?;
    if program.function_symbol() != machine.entry_symbol()
        || target.configured_target != fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1
        || target.wave_width_bits != 64
    {
        return Err(E::Profile);
    }
    let source = generate::source(&program.recipes, machine.kernel().kernarg_storage_bytes())?;
    let obligation = obligation(program, machine, &source);
    let binding = FunctionalRefinementBindingV2::from_subjects(
        program.inputs().verus_execution().obligation().subjects(),
        DigestV1::from_untrusted_bytes(Sha256::digest(&obligation).into()),
    )
    .map_err(|_| E::Profile)?;
    let source_bytes = source.source().to_vec().into_boxed_slice();
    let (retained, policy) = execute_and_import_generated_conditional_fill_composition_locally_v1(
        runtime,
        source,
        binding,
        timeout_seconds,
    )
    .map_err(E::Execution)?;
    let proof = retained.proof();
    if proof.binding() != binding
        || proof.boundary() != BOUNDARY
        || !proof.signature_and_policy_verified()
        || !policy.accepts_signer(proof.signer_identity())
        || proof.toolchain() != policy.toolchain()
    {
        return Err(E::Receipt);
    }
    Ok(ProductionConditionalFillRefinementExecutionV1 {
        program,
        machine,
        source: source_bytes,
        obligation: obligation.into_boxed_slice(),
        retained,
    })
}

fn obligation(
    program: &CheckedConditionalFillProgramV1<'_>,
    machine: &CheckedGfx942FillAnalysisV1<'_>,
    source: &CanonicalGeneratedVerusProofInputV3,
) -> Vec<u8> {
    let mut bytes = DOMAIN.to_vec();
    let mut record = |name: &str, value: &[u8]| {
        bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&Sha256::digest(value));
    };
    let inputs = program.inputs();
    let lineage = program.lineage();
    let execution = machine.execution();
    for (name, value) in [
        ("semantic", inputs.semantic_mir().canonical_encoding()),
        ("middle", inputs.middle_end().canonical_bytes()),
        ("kir", inputs.kernel_ir().canonical_bytes()),
        ("correspondence", inputs.exact_correspondence_bytes()),
        ("formal", inputs.formal_memory().canonical_bytes()),
        ("proof", inputs.association().canonical_bytes()),
        ("conditional", inputs.verus_execution().canonical_bytes()),
        ("target", lineage.target_binding().canonical_bytes()),
        ("layout", lineage.data_layout().canonical_bytes()),
        ("llvm", lineage.semantic_to_llvm().canonical_bytes()),
        (
            "target-kir",
            lineage.replay().replay().target_bound_kernel_ir_bytes(),
        ),
        (
            "replay",
            lineage.replay().replay().evidence().canonical_bytes(),
        ),
        ("symbol", machine.entry_symbol().as_bytes()),
        ("payload", machine.kernel().code_object()),
        ("analysis-request", execution.request().canonical_bytes()),
        ("analysis", execution.analysis().canonical_bytes()),
        ("analysis-receipt", execution.canonical_receipt_bytes()),
        ("generated-source", source.source()),
    ] {
        record(name, value);
    }
    for (name, recipe) in ["semantic-recipe", "neutral-recipe", "target-recipe"]
        .into_iter()
        .zip(&program.recipes)
    {
        record(name, &recipe.canonical_bytes());
    }
    bytes
}

#[derive(Debug)]
pub enum ConditionalFillRefinementErrorV1 {
    Program(crate::ConditionalFillProgramErrorV1),
    Machine(fe2o3_kernel_analysis::Gfx942FillAnalysisErrorV1),
    Profile,
    Source(GeneratedVerusProofInputErrorV3),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
    Receipt,
}
impl fmt::Display for ConditionalFillRefinementErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional fill refinement failed: {self:?}")
    }
}
impl Error for ConditionalFillRefinementErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Program(error) => Some(error),
            Self::Machine(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
