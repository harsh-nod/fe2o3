//! Transactional adaptation of structured general-GEMM KIR diagnostics.
//!
//! The semantic vocabulary and verifier live in `fe2o3-kernel-ir`. This
//! module only binds one KIR graph to a compile request and turns a verified
//! counterexample into a fail-closed compiler diagnostic before any downstream
//! proof or artifact backend runs.

use core::fmt;

use fe2o3_compiler_api::{
    CompileOutputV1, CompileRequestV1, CompilerStageV1, ObligationSetIdentityV1, RequestIdentityV1,
    SnapshotIdentityV1,
};
use fe2o3_kernel_ir::{
    GeneralGemmKirDiagnosticV1, GeneralGemmKirV1, GeneralGemmPropertyV1,
    GeneralGemmVerificationStageV1, verify_general_gemm_kir_v1,
};
use sha2::{Digest, Sha256};

use crate::gemm_proof_required::{
    semantic_binding_mismatch_output, semantic_counterexample_output, semantic_malformed_output,
};
use crate::{
    CompilerBackendFailureV1, GEMM_REQUIRED_SAFETY_PROPERTIES_V1, GemmProofDiagnosticV1,
    GemmSafetyPropertyV1, TransactionalCompilerBackendV1,
};

const GENERAL_GEMM_SEMANTIC_OBLIGATION_SET_DOMAIN_V1: &[u8] =
    b"fe2o3.compiler-driver.general-gemm.semantic-obligation-set.v1";

#[derive(Clone, Copy)]
struct GemmSemanticPropertySchemaEntryV1 {
    property: GemmSafetyPropertyV1,
    spelling: &'static str,
    code: u32,
    stage: CompilerStageV1,
}

fn semantic_property_schema_v1()
-> [GemmSemanticPropertySchemaEntryV1; GEMM_REQUIRED_SAFETY_PROPERTIES_V1.len()] {
    GEMM_REQUIRED_SAFETY_PROPERTIES_V1.map(|property| GemmSemanticPropertySchemaEntryV1 {
        property,
        spelling: property.as_str(),
        code: GemmProofDiagnosticV1::for_property(property).code(),
        stage: property.verification_stage(),
    })
}

fn semantic_obligation_set_identity_from_schema_v1(
    source_snapshot_identity: SnapshotIdentityV1,
    kir: &GeneralGemmKirV1,
    schema: &[GemmSemanticPropertySchemaEntryV1],
) -> ObligationSetIdentityV1 {
    fn field(hasher: &mut Sha256, value: &[u8]) {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value);
    }

    let mut hasher = Sha256::new();
    field(&mut hasher, GENERAL_GEMM_SEMANTIC_OBLIGATION_SET_DOMAIN_V1);
    field(&mut hasher, source_snapshot_identity.as_bytes());
    field(&mut hasher, kir.identity().as_bytes());
    hasher.update((schema.len() as u64).to_le_bytes());
    for entry in schema {
        hasher.update([entry.property as u8]);
        field(&mut hasher, entry.spelling.as_bytes());
        hasher.update(entry.code.to_le_bytes());
        hasher.update([entry.stage as u8]);
    }
    ObligationSetIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

/// Computes the exact V1 semantic-obligation commitment for one source
/// snapshot and one complete bounded general-GEMM KIR graph.
///
/// The commitment also covers the driver-owned ordered property vocabulary,
/// including every spelling, numeric diagnostic code, and verification stage.
/// It is deterministic binding material, not authentication or proof evidence.
pub fn general_gemm_semantic_obligation_set_identity_v1(
    source_snapshot_identity: SnapshotIdentityV1,
    kir: &GeneralGemmKirV1,
) -> ObligationSetIdentityV1 {
    semantic_obligation_set_identity_from_schema_v1(
        source_snapshot_identity,
        kir,
        &semantic_property_schema_v1(),
    )
}

/// One KIR counterexample translated into compiler-driver vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmSemanticCounterexampleV1 {
    property: GemmSafetyPropertyV1,
    event_index: Option<usize>,
}

impl GemmSemanticCounterexampleV1 {
    /// Returns the independently violated safety or refinement property.
    pub const fn property(self) -> GemmSafetyPropertyV1 {
        self.property
    }

    /// Returns the phase-event index retained by KIR, when applicable.
    pub const fn event_index(self) -> Option<usize> {
        self.event_index
    }
}

/// Failure from the KIR-to-driver semantic adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmSemanticAnalysisErrorV1 {
    /// The structured KIR verifier found a concrete property violation.
    Counterexample(GemmSemanticCounterexampleV1),
    /// KIR property, code, and stage vocabularies did not match the driver.
    SchemaMismatch,
}

/// Why structured semantic KIR could not be bound to a compile request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmSemanticProgramBindingErrorV1 {
    /// The request does not commit to this source snapshot, KIR, and exact
    /// ordered property schema.
    ObligationSetMismatch,
}

impl fmt::Display for GemmSemanticProgramBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("general GEMM semantic obligation-set commitment mismatch")
    }
}

impl std::error::Error for GemmSemanticProgramBindingErrorV1 {}

/// Request-bound structured KIR presented to the pre-proof checker.
///
/// Construction verifies the deterministic source/KIR/schema obligation
/// commitment, but does not authenticate the frontend producer. The eventual
/// authenticated MIR importer must be the only production producer of this
/// record. This record grants no proof or artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmSemanticProgramV1 {
    request_identity: RequestIdentityV1,
    input_snapshot_identity: SnapshotIdentityV1,
    obligation_set_identity: ObligationSetIdentityV1,
    kir: GeneralGemmKirV1,
}

impl GemmSemanticProgramV1 {
    /// Binds one structured general-GEMM KIR graph to a compile request.
    pub fn new(
        request: &CompileRequestV1,
        kir: GeneralGemmKirV1,
    ) -> Result<Self, GemmSemanticProgramBindingErrorV1> {
        let expected_obligation_set_identity =
            general_gemm_semantic_obligation_set_identity_v1(request.input().identity(), &kir);
        if request.input_obligations_identity() != expected_obligation_set_identity {
            return Err(GemmSemanticProgramBindingErrorV1::ObligationSetMismatch);
        }
        Ok(Self {
            request_identity: request.identity(),
            input_snapshot_identity: request.input().identity(),
            obligation_set_identity: request.input_obligations_identity(),
            kir,
        })
    }

    /// Returns the exact structured KIR graph.
    pub const fn kir(&self) -> &GeneralGemmKirV1 {
        &self.kir
    }

    fn is_bound_to(&self, request: &CompileRequestV1) -> bool {
        self.request_identity == request.identity()
            && self.input_snapshot_identity == request.input().identity()
            && self.obligation_set_identity == request.input_obligations_identity()
    }
}

/// Runs the canonical KIR verifier and checks its diagnostic vocabulary
/// against the compiler driver.
///
/// Success is not proof evidence. It only means the structured KIR verifier
/// found no supported semantic counterexample.
pub fn analyze_gemm_semantics_v1(
    program: &GemmSemanticProgramV1,
) -> Result<(), GemmSemanticAnalysisErrorV1> {
    let diagnostic = match verify_general_gemm_kir_v1(&program.kir) {
        Ok(_) => return Ok(()),
        Err(diagnostic) => diagnostic,
    };
    let counterexample = translate_diagnostic(diagnostic)?;
    Err(GemmSemanticAnalysisErrorV1::Counterexample(counterexample))
}

fn translate_diagnostic(
    diagnostic: GeneralGemmKirDiagnosticV1,
) -> Result<GemmSemanticCounterexampleV1, GemmSemanticAnalysisErrorV1> {
    let property = compiler_property(diagnostic.property);
    if diagnostic.code != GemmProofDiagnosticV1::for_property(property).code()
        || compiler_stage(diagnostic.stage) != property.verification_stage()
    {
        return Err(GemmSemanticAnalysisErrorV1::SchemaMismatch);
    }
    Ok(GemmSemanticCounterexampleV1 {
        property,
        event_index: diagnostic.event_index,
    })
}

const fn compiler_property(property: GeneralGemmPropertyV1) -> GemmSafetyPropertyV1 {
    match property {
        GeneralGemmPropertyV1::MemorySafe => GemmSafetyPropertyV1::MemorySafe,
        GeneralGemmPropertyV1::BoundsSafe => GemmSafetyPropertyV1::BoundsSafe,
        GeneralGemmPropertyV1::Initialized => GemmSafetyPropertyV1::Initialized,
        GeneralGemmPropertyV1::RaceFree => GemmSafetyPropertyV1::RaceFree,
        GeneralGemmPropertyV1::BarrierConvergent => GemmSafetyPropertyV1::BarrierConvergent,
        GeneralGemmPropertyV1::OutputRegionInjective => GemmSafetyPropertyV1::OutputRegionInjective,
        GeneralGemmPropertyV1::LdsEpochCorrect => GemmSafetyPropertyV1::LdsEpochCorrect,
        GeneralGemmPropertyV1::AccumulatorPhaseRefinement => {
            GemmSafetyPropertyV1::AccumulatorPhaseRefinement
        }
        GeneralGemmPropertyV1::TailRefinement => GemmSafetyPropertyV1::TailRefinement,
        GeneralGemmPropertyV1::EpilogueRefinement => GemmSafetyPropertyV1::EpilogueRefinement,
        GeneralGemmPropertyV1::NumericalContract => GemmSafetyPropertyV1::NumericalContract,
        GeneralGemmPropertyV1::MachineRefinementBoundary => {
            GemmSafetyPropertyV1::MachineRefinementBoundary
        }
    }
}

const fn compiler_stage(stage: GeneralGemmVerificationStageV1) -> CompilerStageV1 {
    match stage {
        GeneralGemmVerificationStageV1::Kernel => CompilerStageV1::Kernel,
        GeneralGemmVerificationStageV1::Tile => CompilerStageV1::Tile,
        GeneralGemmVerificationStageV1::Gpu => CompilerStageV1::Gpu,
        GeneralGemmVerificationStageV1::Amdgcn => CompilerStageV1::Amdgcn,
    }
}

/// Pre-proof backend that rejects KIR counterexamples before invoking any
/// downstream compiler or artifact path.
#[derive(Clone, Debug)]
pub struct GemmSemanticCheckingBackendV1<Backend> {
    program: GemmSemanticProgramV1,
    backend: Backend,
}

impl<Backend> GemmSemanticCheckingBackendV1<Backend> {
    /// Wraps request-bound KIR and a downstream backend.
    pub const fn new(program: GemmSemanticProgramV1, backend: Backend) -> Self {
        Self { program, backend }
    }

    /// Returns shared access to the semantic program and downstream backend.
    pub const fn parts(&self) -> (&GemmSemanticProgramV1, &Backend) {
        (&self.program, &self.backend)
    }

    /// Returns mutable access to the downstream backend.
    pub fn parts_mut(&mut self) -> (&GemmSemanticProgramV1, &mut Backend) {
        (&self.program, &mut self.backend)
    }
}

impl<Backend> TransactionalCompilerBackendV1 for GemmSemanticCheckingBackendV1<Backend>
where
    Backend: TransactionalCompilerBackendV1,
{
    fn compile_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        if !self.program.is_bound_to(request) {
            return Ok(semantic_binding_mismatch_output(request));
        }
        match analyze_gemm_semantics_v1(&self.program) {
            Ok(()) => self.backend.compile_transaction(request),
            Err(GemmSemanticAnalysisErrorV1::Counterexample(counterexample)) => Ok(
                semantic_counterexample_output(request, counterexample.property, None),
            ),
            Err(GemmSemanticAnalysisErrorV1::SchemaMismatch) => {
                Ok(semantic_malformed_output(request))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotV1};

    use super::*;

    fn kir() -> GeneralGemmKirV1 {
        GeneralGemmKirV1::canonical(
            GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
                dimensions: [17, 19, 18],
                strides: [23, 29, 31],
                storage_elements: [386, 512, 515],
                block_counts: [2, 2, 1],
                aql_grid_work_items: [128, 2, 1],
                reduction_phases: 2,
                alpha_bits: 2.0_f32.to_bits(),
                beta_bits: (-1.0_f32).to_bits(),
            })
            .unwrap(),
        )
    }

    fn memory_diagnostic() -> GeneralGemmKirDiagnosticV1 {
        GeneralGemmKirDiagnosticV1 {
            property: GeneralGemmPropertyV1::MemorySafe,
            stage: GeneralGemmVerificationStageV1::Gpu,
            code: 0x4647_0101,
            event_index: Some(0),
        }
    }

    #[test]
    fn diagnostic_code_or_stage_drift_fails_closed() {
        let mut code_drift = memory_diagnostic();
        code_drift.code ^= 1;
        assert_eq!(
            translate_diagnostic(code_drift),
            Err(GemmSemanticAnalysisErrorV1::SchemaMismatch)
        );

        let mut stage_drift = memory_diagnostic();
        stage_drift.stage = GeneralGemmVerificationStageV1::Kernel;
        assert_eq!(
            translate_diagnostic(stage_drift),
            Err(GemmSemanticAnalysisErrorV1::SchemaMismatch)
        );
    }

    #[test]
    fn exact_property_schema_and_cardinality_change_the_obligation_identity() {
        let source = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
        let kir = kir();
        let schema = semantic_property_schema_v1();
        let canonical = semantic_obligation_set_identity_from_schema_v1(source, &kir, &schema);

        let mut spelling_drift = schema;
        spelling_drift[0].spelling = "memory-safe";
        assert_ne!(
            semantic_obligation_set_identity_from_schema_v1(source, &kir, &spelling_drift),
            canonical
        );

        let mut code_drift = schema;
        code_drift[0].code ^= 1;
        assert_ne!(
            semantic_obligation_set_identity_from_schema_v1(source, &kir, &code_drift),
            canonical
        );

        let mut stage_drift = schema;
        stage_drift[0].stage = CompilerStageV1::Kernel;
        assert_ne!(
            semantic_obligation_set_identity_from_schema_v1(source, &kir, &stage_drift),
            canonical
        );

        assert_ne!(
            semantic_obligation_set_identity_from_schema_v1(
                source,
                &kir,
                &schema[..schema.len() - 1],
            ),
            canonical
        );
    }
}
