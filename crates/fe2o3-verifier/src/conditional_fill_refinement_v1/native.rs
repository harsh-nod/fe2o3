//! Final native KIR to the exact authenticated gfx942 fill machine, not source erasure.
use super::generate;
use crate::conditional_fill_program_v1::native_v1::input_storage;
use crate::functional_refinement_receipt_v2::{
    RetainedImportedFunctionalRefinementReceiptV2,
    execute_and_import_generated_native_fill_composition_locally_v1,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3, ConditionalFillRefinementErrorV1,
    FunctionalRefinementVerusExecutionErrorV2, FunctionalRefinementVerusRuntimeLeaseV1,
    NativeConditionalFillProgramErrorV1, RecoveredCompilerConditionalNativeSemanticHandoffV5,
    check_native_conditional_fill_program_v1,
};
use fe2o3_functional_proof::{FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, check_gfx942_fill_analysis_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

const DOMAIN: &[u8] = b"FE2O3/NATIVE-F-GFX942-FILL-CONDITIONAL-REFINEMENT/V1\0";
const BOUNDARY: FunctionalRefinementBoundaryV2 =
    FunctionalRefinementBoundaryV2::FinalKernelIrToGfx942FillDispatchConditional;
const SCRATCH: usize = 4 * crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 + 256 * 1024;
const GENERATION_WORK: usize = 8 * crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3;

/// Actual executed/imported final-KIR-to-machine proof, retaining both original
/// owners by borrow. Source-to-F remains the original recovered V5 relation.
/// No original compiler, application currentness or native execution is attested.
///
/// ```compile_fail
/// use fe2o3_verifier::{NativeConditionalFillRefinementExecutionV1 as Native,
///     OwnedConditionalFillRefinementExecutionV1 as Legacy};
/// fn erase(value: Native<'_>) -> Legacy { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::NativeConditionalFillRefinementExecutionV1 as Proof;
/// fn escape<'a>(value: Proof<'a>) -> Proof<'static> { value }
/// ```
#[must_use = "reserve the returned storage and retain both borrowed input owners"]
pub struct NativeConditionalFillRefinementExecutionV1<'a> {
    owner: &'a RecoveredCompilerConditionalNativeSemanticHandoffV5,
    analysis: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    source: Box<[u8]>,
    obligation: Box<[u8]>,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}

impl NativeConditionalFillRefinementExecutionV1<'_> {
    pub const fn owner(&self) -> &RecoveredCompilerConditionalNativeSemanticHandoffV5 {
        self.owner
    }
    pub const fn analysis_execution(&self) -> &AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.analysis
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
    pub const fn authenticates_currentness(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Additional returned proof/source/obligation storage, unreserved on success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalFillRefinementStorageV1(usize);
impl NativeConditionalFillRefinementStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum NativeConditionalFillRefinementErrorV1 {
    Resource(Resource),
    Program(NativeConditionalFillProgramErrorV1),
    Machine(fe2o3_kernel_analysis::Gfx942FillAnalysisErrorV1),
    Generation(ConditionalFillRefinementErrorV1),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
    Profile,
    Receipt,
}
impl From<Resource> for NativeConditionalFillRefinementErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for NativeConditionalFillRefinementErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native conditional fill refinement failed: {self:?}")
    }
}
impl std::error::Error for NativeConditionalFillRefinementErrorV1 {}
type Error = NativeConditionalFillRefinementErrorV1;

/// Checks the exact native owner and actual authenticated analyzer execution,
/// generates only the final-KIR relation, executes the pinned closed-fill proof
/// runtime and strictly imports boundary 6. No external receipt or source string
/// is accepted. This entry does not authenticate artifact publication/currentness.
///
/// The recovered owner, transport and metadata must remain paid on `budget`.
/// Recognizer/generator scratch, hashing work and returned proof storage use that
/// same account. Analyzer/ELF parsing and the bounded subprocess runtime retain
/// their existing separate resource domains; this is not aggregate RSS credit.
/// Failure or unwind retains terminal scratch. Only complete success releases
/// scratch and returns an UNRESERVED output charge. Do not blanket-refund errors.
pub fn execute_native_conditional_fill_refinement_v1<'a>(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    owner: &'a RecoveredCompilerConditionalNativeSemanticHandoffV5,
    analysis: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    timeout_seconds: u32,
    budget: &mut Budget<'_>,
) -> Result<
    (
        NativeConditionalFillRefinementExecutionV1<'a>,
        NativeConditionalFillRefinementStorageV1,
    ),
    Error,
> {
    budget.charge_work(8)?;
    if budget.storage() < input_storage(owner)? {
        return Err(Resource::Accounting.into());
    }
    let work = [
        owner.handoff().canonical_bytes().len(),
        owner.output().canonical().canonical_bytes().len(),
        analysis.request().canonical_bytes().len(),
        analysis.analysis().canonical_bytes().len(),
        analysis.canonical_receipt_bytes().len(),
        analysis.request().exact_payload_bytes().len(),
        GENERATION_WORK,
    ]
    .into_iter()
    .try_fold(0usize, |n, part| {
        n.checked_add(part).ok_or(Resource::Arithmetic)
    })?;
    budget.charge_work(work)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(SCRATCH)?;
    let (program, _) =
        check_native_conditional_fill_program_v1(owner, budget).map_err(Error::Program)?;
    let machine = check_gfx942_fill_analysis_v1(analysis, program.function_symbol())
        .map_err(Error::Machine)?;
    let recipe = program
        .final_recipe()
        .map_err(|e| Error::Generation(ConditionalFillRefinementErrorV1::Program(e)))?;
    let source = generate::native_source(&recipe, machine.kernel().kernarg_storage_bytes())
        .map_err(Error::Generation)?;
    let obligation = obligation(owner, analysis, &source, &recipe.canonical_bytes());
    let binding = FunctionalRefinementBindingV2::from_subjects(
        program.formula_report().binding().subjects(),
        DigestV1::from_untrusted_bytes(Sha256::digest(&obligation).into()),
    )
    .map_err(|_| Error::Profile)?;
    let source_bytes = source.source().to_vec().into_boxed_slice();
    let (retained, policy) = execute_and_import_generated_native_fill_composition_locally_v1(
        runtime,
        source,
        binding,
        timeout_seconds,
    )
    .map_err(Error::Execution)?;
    let proof = retained.proof();
    if proof.boundary() != BOUNDARY
        || proof.binding() != binding
        || !proof.signature_and_policy_verified()
        || !policy.accepts_signer(proof.signer_identity())
        || proof.toolchain() != policy.toolchain()
    {
        return Err(Error::Receipt);
    }
    let output = NativeConditionalFillRefinementExecutionV1 {
        owner,
        analysis,
        source: source_bytes,
        obligation: obligation.into_boxed_slice(),
        retained,
    };
    let storage = size_of::<NativeConditionalFillRefinementExecutionV1>()
        .checked_add(output.source.len())
        .and_then(|n| n.checked_add(output.obligation.len()))
        .ok_or(Resource::Arithmetic)?;
    if budget.work_ledger_identity_v1() != ledger
        || budget.storage() != floor.checked_add(SCRATCH).ok_or(Resource::Arithmetic)?
        || storage > SCRATCH
    {
        return Err(Resource::Accounting.into());
    }
    drop(recipe);
    budget.release_storage(SCRATCH)?;
    Ok((output, NativeConditionalFillRefinementStorageV1(storage)))
}

fn obligation(
    owner: &RecoveredCompilerConditionalNativeSemanticHandoffV5,
    analysis: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    source: &CanonicalGeneratedVerusProofInputV3,
    recipe: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend_from_slice(DOMAIN);
    for (name, value) in [
        ("native-v5-handoff", owner.handoff().canonical_bytes()),
        (
            "final-v12-kir",
            owner.output().canonical().canonical_bytes(),
        ),
        ("target", owner.profile().device_target().as_bytes()),
        ("final-recipe", recipe),
        ("payload", analysis.request().exact_payload_bytes()),
        ("analysis-request", analysis.request().canonical_bytes()),
        ("analysis", analysis.analysis().canonical_bytes()),
        ("analysis-receipt", analysis.canonical_receipt_bytes()),
        ("generated-source", source.source()),
    ] {
        bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&Sha256::digest(value));
    }
    bytes
}

#[cfg(test)]
mod tests;
