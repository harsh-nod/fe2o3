//! Per-compilation Verus join for the exact MIR-to-live-PLIRON contract.
//!
//! This workload-neutral module consumes the move-only structural verifier
//! result, derives subjects from retained authenticated effect receipts,
//! generates contract-specific conditional-lemma instantiations, executes them
//! in the retained runtime, and imports the signed result. The exact contracts
//! and prior effect receipts are authenticated binding inputs, not logical
//! premises inside this Verus execution. Neither source nor semantic subjects
//! are accepted from the caller.

use std::{error::Error, fmt, fmt::Write as _};

use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementReceiptIdentityV2, FunctionalRefinementSubjectsV2,
    ParallelReferenceContractV1, SemanticCollectiveKindV1, SemanticEvaluationOrderV1,
    VerusToolchainIdentityV2,
};
use fe2o3_pliron::{
    ProductionFunctionalRefinementTrustPolicyV2, ProductionMiddleEndEvidenceV5,
    ProductionMirPlironSemanticContractErrorV1, ProductionMirPlironSemanticContractReportV1,
    ProductionParallelReferenceContractErrorV1, ProductionParallelReferenceContractReportV1,
    ProductionRankedKernelLoweringInputV1, ProductionTotalOutputRefinementErrorV2,
    ProductionVerifiedMirPlironKernelV1, require_mir_pliron_semantic_contract_v1,
    require_parallel_reference_contract_v1, require_total_output_refinement_v2,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::functional_refinement_receipt_v2::execute_and_import_generated_mir_pliron_composition_locally_v1;
use crate::{
    CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusExecutionErrorV2,
    FunctionalRefinementVerusRuntimeLeaseV1,
};

const AGGREGATE_OBLIGATION_DOMAIN_V1: &[u8] =
    b"FE2O3/MIR-PLIRON/PER-COMPILATION-VERUS-OBLIGATION/V1\0";

/// Exact authenticated identities produced by one aggregate Verus execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMirPlironPerCompilationVerusReportV1 {
    contract_identity: DigestV1,
    parallel_contract_identity: DigestV1,
    pliron_evidence_identity: DigestV1,
    composition_template_identity: DigestV1,
    generated_source_identity: DigestV1,
    obligation_identity: DigestV1,
    toolchain: VerusToolchainIdentityV2,
    execution_identity: DigestV1,
    receipt_identity: FunctionalRefinementReceiptIdentityV2,
    retained_effect_receipts: u64,
}

impl ProductionMirPlironPerCompilationVerusReportV1 {
    pub const fn contract_identity(self) -> DigestV1 {
        self.contract_identity
    }
    pub const fn pliron_evidence_identity(self) -> DigestV1 {
        self.pliron_evidence_identity
    }
    pub const fn parallel_contract_identity(self) -> DigestV1 {
        self.parallel_contract_identity
    }
    pub const fn composition_template_identity(self) -> DigestV1 {
        self.composition_template_identity
    }
    pub const fn generated_source_identity(self) -> DigestV1 {
        self.generated_source_identity
    }
    pub const fn obligation_identity(self) -> DigestV1 {
        self.obligation_identity
    }
    pub const fn toolchain(self) -> VerusToolchainIdentityV2 {
        self.toolchain
    }
    pub const fn execution_identity(self) -> DigestV1 {
        self.execution_identity
    }
    pub const fn receipt_identity(self) -> FunctionalRefinementReceiptIdentityV2 {
        self.receipt_identity
    }
    pub const fn retained_effect_receipts(self) -> u64 {
        self.retained_effect_receipts
    }
    pub const fn has_authenticated_per_compilation_verus_execution(self) -> bool {
        true
    }
    pub const fn binding_includes_exact_safe_reference_kernel_mir_and_live_pliron(self) -> bool {
        true
    }
    pub const fn proves_conditional_composition_for_the_admitted_contract(self) -> bool {
        true
    }
    pub const fn compiler_extraction_projection_and_pass_soundness_remain_trusted(self) -> bool {
        true
    }
    pub const fn generated_identity_comments_are_binding_inputs_not_verus_premises(self) -> bool {
        true
    }
    pub const fn retained_effect_receipts_are_authenticated_binding_inputs(self) -> bool {
        true
    }
    pub const fn retained_effect_receipts_are_logical_verus_premises(self) -> bool {
        false
    }
    pub const fn grants_llvm_or_later_authority(self) -> bool {
        false
    }
}

/// Move-only owner admitted through both structural and executed Verus gates.
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionVerusVerifiedMirPlironKernelV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionVerusVerifiedMirPlironKernelV1>();
/// ```
#[derive(Debug)]
pub struct ProductionVerusVerifiedMirPlironKernelV1 {
    structural: ProductionVerifiedMirPlironKernelV1,
    parallel_contract: ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
    aggregate: ProductionMirPlironPerCompilationVerusReportV1,
    _compiler_owned_policy: ProductionFunctionalRefinementTrustPolicyV2,
}

impl ProductionVerusVerifiedMirPlironKernelV1 {
    pub const fn structural(&self) -> &ProductionVerifiedMirPlironKernelV1 {
        &self.structural
    }
    pub const fn per_compilation_verus_report(
        &self,
    ) -> ProductionMirPlironPerCompilationVerusReportV1 {
        self.aggregate
    }
    pub const fn parallel_contract(&self) -> &ParallelReferenceContractV1 {
        &self.parallel_contract
    }
    pub const fn parallel_report(&self) -> ProductionParallelReferenceContractReportV1 {
        self.parallel_report
    }
    pub const fn retains_authenticated_conditional_composition_for_admitted_contract(
        &self,
    ) -> bool {
        true
    }
    pub const fn compiler_extraction_projection_and_pass_soundness_remain_trusted(&self) -> bool {
        true
    }
    pub const fn generated_identity_comments_are_not_verus_premises(&self) -> bool {
        true
    }
    pub const fn generated_contract_instantiations_are_conditional_composition(&self) -> bool {
        true
    }
    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionMirPlironPerCompilationVerusErrorV1 {
    TotalOutput(ProductionTotalOutputRefinementErrorV2),
    SemanticContract(ProductionMirPlironSemanticContractErrorV1),
    ParallelContract(ProductionParallelReferenceContractErrorV1),
    ParallelReportMismatch,
    StructuralReportMismatch,
    MissingRetainedEffectReceipt,
    InconsistentRetainedSubjects,
    WrongRetainedBoundary,
    CounterOverflow,
    GeneratedSource(String),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
}

impl fmt::Display for ProductionMirPlironPerCompilationVerusErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalOutput(error) => write!(
                formatter,
                "per-compilation total-output reconciliation failed: {error}",
            ),
            Self::SemanticContract(error) => write!(
                formatter,
                "per-compilation MIR/PLIRON structural reconciliation failed: {error}",
            ),
            Self::ParallelContract(error) => write!(
                formatter,
                "per-compilation parallel-contract reconciliation failed: {error}",
            ),
            Self::ParallelReportMismatch => formatter.write_str(
                "per-compilation parallel-contract report differs from the report recomputed over the borrowed live graph",
            ),
            Self::StructuralReportMismatch => formatter.write_str(
                "per-compilation MIR/PLIRON report differs from the report recomputed over the borrowed live graph",
            ),
            Self::MissingRetainedEffectReceipt => formatter.write_str(
                "per-compilation MIR/PLIRON proof requires at least one retained authenticated effect receipt",
            ),
            Self::InconsistentRetainedSubjects => formatter.write_str(
                "retained effect receipts do not identify one exact safe-reference MIR and kernel MIR subject pair",
            ),
            Self::WrongRetainedBoundary => formatter.write_str(
                "retained effect receipt does not cover safe-reference MIR to kernel MIR",
            ),
            Self::CounterOverflow => formatter.write_str(
                "per-compilation MIR/PLIRON proof count cannot be represented",
            ),
            Self::GeneratedSource(detail) => {
                write!(formatter, "generated MIR/PLIRON Verus source was rejected: {detail}")
            }
            Self::Execution(error) => write!(
                formatter,
                "authenticated per-compilation MIR/PLIRON Verus execution failed: {error}",
            ),
        }
    }
}

impl Error for ProductionMirPlironPerCompilationVerusErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TotalOutput(error) => Some(error),
            Self::SemanticContract(error) => Some(error),
            Self::ParallelContract(error) => Some(error),
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes and imports one aggregate conditional-lemma program for the exact
/// structurally admitted compilation. This authenticates composition inputs;
/// it does not lift prior receipt theorems into a single Verus proof context.
/// Runtime absence, unsupported source, proof failure, stale identities, and
/// receipt import failure all return `Err`.
pub fn execute_mir_pliron_semantic_contract_per_compilation_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    structural: ProductionVerifiedMirPlironKernelV1,
    parallel_contract: ParallelReferenceContractV1,
    timeout_seconds: u32,
) -> Result<ProductionVerusVerifiedMirPlironKernelV1, ProductionMirPlironPerCompilationVerusErrorV1>
{
    let parallel_report = require_parallel_reference_contract_v1(
        structural.ranked(),
        structural.evidence(),
        structural.semantic_contract_report(),
        structural.semantic_contract(),
        &parallel_contract,
    )
    .map_err(ProductionMirPlironPerCompilationVerusErrorV1::ParallelContract)?;
    let (aggregate, policy) = execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1(
        runtime,
        structural.ranked(),
        structural.evidence(),
        structural.semantic_contract(),
        structural.semantic_contract_report(),
        &parallel_contract,
        parallel_report,
        timeout_seconds,
    )?;
    Ok(ProductionVerusVerifiedMirPlironKernelV1 {
        structural,
        parallel_contract,
        parallel_report,
        aggregate,
        _compiler_owned_policy: policy,
    })
}

/// Borrowed production entrypoint used while the compiler projection receipt
/// retains ownership of the ranked lowering input.
///
/// The structural report is not trusted on declaration: this function
/// recomputes total-output and exact semantic-contract reconciliation over the
/// borrowed live graph before executing Verus. The returned policy must remain
/// under compiler custody alongside the aggregate report.
pub fn execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    structural_report: ProductionMirPlironSemanticContractReportV1,
    parallel_contract: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
    timeout_seconds: u32,
) -> Result<
    (
        ProductionMirPlironPerCompilationVerusReportV1,
        ProductionFunctionalRefinementTrustPolicyV2,
    ),
    ProductionMirPlironPerCompilationVerusErrorV1,
> {
    let total_output = require_total_output_refinement_v2(ranked, evidence)
        .map_err(ProductionMirPlironPerCompilationVerusErrorV1::TotalOutput)?;
    let recomputed =
        require_mir_pliron_semantic_contract_v1(ranked, evidence, total_output, contract)
            .map_err(ProductionMirPlironPerCompilationVerusErrorV1::SemanticContract)?;
    if recomputed != structural_report {
        return Err(ProductionMirPlironPerCompilationVerusErrorV1::StructuralReportMismatch);
    }
    let recomputed_parallel = require_parallel_reference_contract_v1(
        ranked,
        evidence,
        structural_report,
        contract,
        parallel_contract,
    )
    .map_err(ProductionMirPlironPerCompilationVerusErrorV1::ParallelContract)?;
    if recomputed_parallel != parallel_report {
        return Err(ProductionMirPlironPerCompilationVerusErrorV1::ParallelReportMismatch);
    }
    let subjects = derive_compiler_subjects(ranked, contract)?;
    let source = generate_contract_source(
        contract,
        structural_report,
        parallel_contract,
        parallel_report,
    )?;
    let generated_source_identity = DigestV1::from_untrusted_bytes(source.identity().as_bytes());
    let obligation_identity = aggregate_obligation_identity(
        ranked,
        contract,
        parallel_contract,
        generated_source_identity,
        subjects,
    );
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation_identity)
        .map_err(|error| {
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(error.to_string())
        })?;
    let (imported, policy) = execute_and_import_generated_mir_pliron_composition_locally_v1(
        runtime,
        source,
        binding,
        timeout_seconds,
    )
    .map_err(ProductionMirPlironPerCompilationVerusErrorV1::Execution)?;
    if imported.binding() != binding
        || imported.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron
        || !policy.accepts_signer(imported.signer_identity())
        || policy.toolchain() != imported.toolchain()
        || !imported.signature_and_policy_verified()
    {
        return Err(ProductionMirPlironPerCompilationVerusErrorV1::InconsistentRetainedSubjects);
    }
    let retained_effect_receipts =
        u64::try_from(ranked.retained_functional_refinement_receipts().len())
            .map_err(|_| ProductionMirPlironPerCompilationVerusErrorV1::CounterOverflow)?;
    let aggregate = ProductionMirPlironPerCompilationVerusReportV1 {
        contract_identity: structural_report.contract_identity(),
        parallel_contract_identity: parallel_report.contract_identity(),
        pliron_evidence_identity: contract.pliron_evidence(),
        composition_template_identity: composition_template_identity_v1(),
        generated_source_identity,
        obligation_identity,
        toolchain: imported.toolchain(),
        execution_identity: imported.execution_identity(),
        receipt_identity: imported.receipt_identity(),
        retained_effect_receipts,
    };
    Ok((aggregate, policy))
}

fn derive_compiler_subjects(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
) -> Result<FunctionalRefinementSubjectsV2, ProductionMirPlironPerCompilationVerusErrorV1> {
    let receipts = ranked.retained_functional_refinement_receipts();
    let first = receipts
        .first()
        .ok_or(ProductionMirPlironPerCompilationVerusErrorV1::MissingRetainedEffectReceipt)?;
    let subjects = first.binding().subjects();
    if first.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
        || receipts.iter().any(|receipt| {
            receipt.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
        })
    {
        return Err(ProductionMirPlironPerCompilationVerusErrorV1::WrongRetainedBoundary);
    }
    if subjects.safe_reference_mir_hash() != contract.safe_reference_mir()
        || subjects.kernel_mir_hash() != contract.kernel_mir()
        || receipts
            .iter()
            .any(|receipt| receipt.binding().subjects() != subjects)
    {
        return Err(ProductionMirPlironPerCompilationVerusErrorV1::InconsistentRetainedSubjects);
    }
    Ok(subjects)
}

fn generate_contract_source(
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    report: ProductionMirPlironSemanticContractReportV1,
    parallel_contract: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
) -> Result<CanonicalGeneratedVerusProofInputV3, ProductionMirPlironPerCompilationVerusErrorV1> {
    let mut source = String::new();
    writeln!(
        source,
        "// fe2o3 MIR/PLIRON contract: {}",
        hex(contract.canonical_sha256())
    )
    .map_err(generated_format_error)?;
    writeln!(
        source,
        "// safe-reference MIR: {}",
        hex(contract.safe_reference_mir())
    )
    .map_err(generated_format_error)?;
    writeln!(source, "// kernel MIR: {}", hex(contract.kernel_mir()))
        .map_err(generated_format_error)?;
    writeln!(
        source,
        "// live PLIRON evidence: {}",
        hex(contract.pliron_evidence())
    )
    .map_err(generated_format_error)?;
    writeln!(
        source,
        "// reviewed composition template: {}",
        hex(composition_template_identity_v1()),
    )
    .map_err(generated_format_error)?;
    writeln!(
        source,
        "// parallel contract: {}",
        hex(parallel_contract.canonical_sha256()),
    )
    .map_err(generated_format_error)?;
    writeln!(
        source,
        "// domains={} roots={} loops={} collectives={} outputs={}",
        report.finite_domains(),
        report.typed_roots(),
        report.bounded_loops(),
        report.finite_collectives(),
        report.total_outputs(),
    )
    .map_err(generated_format_error)?;
    writeln!(
        source,
        "// relations={} pointwise={} permutations={} folds={} recurrences={}",
        parallel_report.output_relations(),
        parallel_report.pointwise_relations(),
        parallel_report.permutation_relations(),
        parallel_report.fold_relations(),
        parallel_report.bounded_recurrences(),
    )
    .map_err(generated_format_error)?;
    source.push_str(GENERATED_COMPOSITION_THEOREM_V1);
    append_contract_instantiations_v1(&mut source, contract, parallel_contract)?;
    CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(|error| {
        ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(error.to_string())
    })
}

fn append_contract_instantiations_v1(
    source: &mut String,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    parallel_contract: &ParallelReferenceContractV1,
) -> Result<(), ProductionMirPlironPerCompilationVerusErrorV1> {
    source.push_str("\nverus! {\n\n");
    for (index, output) in contract.outputs().iter().enumerate() {
        let domain = contract
            .domains()
            .iter()
            .find(|domain| domain.identity() == output.output_domain())
            .ok_or_else(|| {
                ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(format!(
                    "output {index} has no finite domain",
                ))
            })?;
        let cardinality = domain.maximum_cardinality().ok_or_else(|| {
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(format!(
                "output {index} finite-domain cardinality overflowed",
            ))
        })?;
        let static_domain = domain.extents().iter().all(|extent| {
            matches!(
                extent,
                fe2o3_functional_proof::SemanticFiniteExtentV1::Static(_)
            )
        });
        let length_relation = if static_domain { "==" } else { "<=" };
        writeln!(
            source,
            r#"pub open spec fn fe2o3_output_{index}_bound_v1() -> int {{ {cardinality} }}

pub proof fn fe2o3_output_{index}_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_pointwise_equal_v1(actual, reference),
        actual.len() > 0,
        actual.len() {length_relation} fe2o3_output_{index}_bound_v1(),
    ensures actual == reference,
{{
    fe2o3_exact_total_output_v1(actual, reference);
}}
"#,
        )
        .map_err(generated_format_error)?;
    }

    for (index, loop_contract) in contract.loops().iter().enumerate() {
        writeln!(
            source,
            r#"pub open spec fn fe2o3_loop_{index}_maximum_steps_v1() -> int {{ {maximum_steps} }}

pub proof fn fe2o3_loop_{index}_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_finite_recurrence_v1(actual, reference),
        actual.len() - 1 <= fe2o3_loop_{index}_maximum_steps_v1(),
    ensures actual == reference,
{{
    fe2o3_finite_recurrence_refinement_v1(actual, reference);
}}
"#,
            maximum_steps = loop_contract.maximum_steps(),
        )
        .map_err(generated_format_error)?;
    }

    for (index, collective) in contract.collectives().iter().enumerate() {
        let kind = match collective.kind() {
            SemanticCollectiveKindV1::FiniteFold => 1,
            SemanticCollectiveKindV1::FiniteRecurrence => 2,
            SemanticCollectiveKindV1::PermutationGather => 3,
        };
        let order = match collective.order() {
            SemanticEvaluationOrderV1::SequentialAscending => 1,
            SemanticEvaluationOrderV1::SequentialDescending => 2,
            SemanticEvaluationOrderV1::Lexicographic => 3,
            SemanticEvaluationOrderV1::ExplicitTree => 4,
        };
        writeln!(
            source,
            r#"pub open spec fn fe2o3_collective_{index}_kind_v1() -> int {{ {kind} }}
pub open spec fn fe2o3_collective_{index}_order_v1() -> int {{ {order} }}
pub open spec fn fe2o3_collective_{index}_domain_bound_v1() -> int {{ {domain_bound} }}
pub open spec fn fe2o3_collective_{index}_step_bound_v1() -> int {{ {step_bound} }}
"#,
            domain_bound = collective.domain_bound(),
            step_bound = collective.step_bound(),
        )
        .map_err(generated_format_error)?;
        match collective.kind() {
            SemanticCollectiveKindV1::FiniteFold | SemanticCollectiveKindV1::FiniteRecurrence => {
                writeln!(
                    source,
                    r#"pub proof fn fe2o3_collective_{index}_refines_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        fe2o3_finite_recurrence_v1(actual, reference),
        actual.len() - 1 <= fe2o3_collective_{index}_step_bound_v1(),
        actual.len() - 1 <= fe2o3_collective_{index}_domain_bound_v1(),
    ensures actual == reference,
{{
    fe2o3_finite_recurrence_refinement_v1(actual, reference);
}}
"#,
                )
                .map_err(generated_format_error)?;
            }
            SemanticCollectiveKindV1::PermutationGather => {
                writeln!(
                    source,
                    r#"pub proof fn fe2o3_collective_{index}_is_injective_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
    left: nat,
    right: nat,
)
    requires
        fe2o3_inverse_permutation_v1(mapping, inverse),
        mapping.len() == fe2o3_collective_{index}_domain_bound_v1(),
        left < mapping.len(),
        right < mapping.len(),
        mapping[left as int] == mapping[right as int],
    ensures left == right,
{{
    fe2o3_permutation_injective_v1(mapping, inverse, left, right);
}}
"#,
                )
                .map_err(generated_format_error)?;
            }
        }
    }

    // The separately reconciled parallel contract is bound into the generated
    // source and aggregate obligation identities. Its host-side witnesses are
    // deliberately not restated as tautological Verus constants here.
    let _ = parallel_contract;
    source.push_str("} // verus!\n\nfn fe2o3_contract_instantiations_v1() {}\n");
    Ok(())
}

fn composition_template_identity_v1() -> DigestV1 {
    DigestV1::from_untrusted_bytes(Sha256::digest(GENERATED_COMPOSITION_THEOREM_V1).into())
}

fn aggregate_obligation_identity(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    parallel_contract: &ParallelReferenceContractV1,
    source_identity: DigestV1,
    subjects: FunctionalRefinementSubjectsV2,
) -> DigestV1 {
    let receipts = ranked
        .retained_functional_refinement_receipts()
        .iter()
        .map(|receipt| AggregateReceiptBindingV1 {
            receipt: receipt.receipt_identity().digest(),
            effect: receipt.binding().normalized_obligation_effect_ir_hash(),
            signer: receipt.signer_identity(),
            execution: receipt.execution_identity(),
            toolchain: receipt.toolchain(),
        })
        .collect::<Vec<_>>();
    aggregate_obligation_from_input(&AggregateObligationInputV1 {
        contract: contract.canonical_sha256(),
        parallel_contract: parallel_contract.canonical_sha256(),
        pliron_evidence: contract.pliron_evidence(),
        template: composition_template_identity_v1(),
        source: source_identity,
        subjects,
        receipts,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AggregateReceiptBindingV1 {
    receipt: DigestV1,
    effect: DigestV1,
    signer: DigestV1,
    execution: DigestV1,
    toolchain: VerusToolchainIdentityV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AggregateObligationInputV1 {
    contract: DigestV1,
    parallel_contract: DigestV1,
    pliron_evidence: DigestV1,
    template: DigestV1,
    source: DigestV1,
    subjects: FunctionalRefinementSubjectsV2,
    receipts: Vec<AggregateReceiptBindingV1>,
}

fn aggregate_obligation_from_input(input: &AggregateObligationInputV1) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, AGGREGATE_OBLIGATION_DOMAIN_V1);
    for identity in [
        input.contract,
        input.parallel_contract,
        input.pliron_evidence,
        input.template,
        input.source,
        input.subjects.safe_reference_identity(),
        input.subjects.safe_reference_source_hash(),
        input.subjects.safe_reference_mir_hash(),
        input.subjects.kernel_subject_identity(),
        input.subjects.kernel_mir_hash(),
    ] {
        put_blob(&mut digest, identity.as_bytes());
    }
    digest.update((input.receipts.len() as u64).to_le_bytes());
    for receipt in &input.receipts {
        for identity in [
            receipt.receipt,
            receipt.effect,
            receipt.signer,
            receipt.execution,
        ] {
            put_blob(&mut digest, identity.as_bytes());
        }
        let toolchain = receipt.toolchain;
        for identity in [
            toolchain.verus_executable(),
            toolchain.verus_configuration(),
            toolchain.solver_executable(),
            toolchain.solver_configuration(),
            toolchain.runtime_closure(),
        ] {
            put_blob(&mut digest, identity.as_bytes());
        }
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn hex(identity: DigestV1) -> String {
    let mut value = String::with_capacity(64);
    for byte in identity.as_bytes() {
        write!(value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn generated_format_error(_: fmt::Error) -> ProductionMirPlironPerCompilationVerusErrorV1 {
    ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(
        "contract-specific source formatting failed".to_owned(),
    )
}

const GENERATED_COMPOSITION_THEOREM_V1: &str =
    include_str!("../verus/mir_pliron_per_compilation_template_v1.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_functional_proof::{
        COMPLETE_GPU_HIERARCHY_V1, MirPlironSemanticContractV1, ParallelNumericalPolicyV1,
        ParallelOutputRelationV1, ParallelReferenceContractV1, ParallelScheduleRelationV1,
        SafeReferenceKindV2, SemanticCollectiveContractV1, SemanticCollectiveKindV1,
        SemanticCoverageBindingV1, SemanticEvaluationOrderV1, SemanticFiniteDomainV1,
        SemanticFiniteExtentV1, SemanticLoopContractV1, SemanticLoopDirectionV1,
        SemanticNumericalPolicyV1, SemanticOutputContractV1, SemanticScalarTypeV1,
        SemanticTypedRootV1,
    };

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    fn obligation_input() -> AggregateObligationInputV1 {
        AggregateObligationInputV1 {
            contract: digest(1),
            parallel_contract: digest(30),
            pliron_evidence: digest(2),
            template: digest(3),
            source: digest(4),
            subjects: FunctionalRefinementSubjectsV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(5),
                digest(18),
                digest(6),
                digest(7),
                digest(8),
            )
            .unwrap(),
            receipts: vec![AggregateReceiptBindingV1 {
                receipt: digest(9),
                effect: digest(10),
                signer: digest(11),
                execution: digest(12),
                toolchain: VerusToolchainIdentityV2::new(
                    digest(13),
                    digest(14),
                    digest(15),
                    digest(16),
                    digest(17),
                )
                .unwrap(),
            }],
        }
    }

    fn pointwise_contract(extent: SemanticFiniteExtentV1) -> MirPlironSemanticContractV1 {
        let domain = digest(21);
        let roots = [22_u8, 23].map(|identity| {
            SemanticTypedRootV1::new(
                digest(identity),
                digest(24),
                domain,
                SemanticScalarTypeV1::Unsigned(32),
                SemanticNumericalPolicyV1::ExactBitVector,
            )
            .unwrap()
        });
        MirPlironSemanticContractV1::new(
            digest(18),
            digest(19),
            digest(20),
            vec![SemanticFiniteDomainV1::new(domain, vec![extent]).unwrap()],
            roots.into_iter().collect(),
            vec![],
            vec![],
            vec![
                SemanticOutputContractV1::new(
                    digest(25),
                    digest(26),
                    domain,
                    digest(22),
                    digest(23),
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn pointwise_parallel_contract(
        contract: &MirPlironSemanticContractV1,
    ) -> ParallelReferenceContractV1 {
        let output = &contract.outputs()[0];
        ParallelReferenceContractV1::new(
            contract.canonical_sha256(),
            vec![
                ParallelOutputRelationV1::new(
                    digest(27),
                    output.identity(),
                    output.output_domain(),
                    ParallelScheduleRelationV1::PointwiseBijection,
                    ParallelNumericalPolicyV1::ExactBitVector,
                    COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                    vec![],
                    digest(28),
                )
                .unwrap(),
            ],
            vec![],
        )
        .unwrap()
    }

    fn generated_fixture_contract() -> MirPlironSemanticContractV1 {
        let output_domain = digest(21);
        let permutation_source_domain = digest(29);
        let roots = (40_u8..54)
            .enumerate()
            .map(|(index, identity)| {
                SemanticTypedRootV1::new(
                    digest(identity),
                    digest(identity + 40),
                    if index >= 12 {
                        permutation_source_domain
                    } else {
                        output_domain
                    },
                    SemanticScalarTypeV1::Unsigned(32),
                    SemanticNumericalPolicyV1::ExactBitVector,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        MirPlironSemanticContractV1::new(
            digest(18),
            digest(19),
            digest(20),
            vec![
                SemanticFiniteDomainV1::new(
                    output_domain,
                    vec![SemanticFiniteExtentV1::Static(64)],
                )
                .unwrap(),
                SemanticFiniteDomainV1::new(
                    permutation_source_domain,
                    vec![SemanticFiniteExtentV1::Static(64)],
                )
                .unwrap(),
            ],
            roots,
            vec![
                SemanticLoopContractV1::new(
                    digest(30),
                    1,
                    2,
                    3,
                    output_domain,
                    digest(40),
                    digest(41),
                    digest(42),
                    digest(43),
                    digest(31),
                    digest(32),
                    SemanticLoopDirectionV1::Increasing,
                    64,
                )
                .unwrap(),
            ],
            vec![
                SemanticCollectiveContractV1::new(
                    digest(33),
                    SemanticCollectiveKindV1::FiniteFold,
                    digest(26),
                    output_domain,
                    output_domain,
                    digest(44),
                    digest(45),
                    digest(46),
                    digest(47),
                    64,
                    64,
                    SemanticEvaluationOrderV1::SequentialAscending,
                    SemanticCoverageBindingV1::TotalView,
                )
                .unwrap(),
                SemanticCollectiveContractV1::new(
                    digest(34),
                    SemanticCollectiveKindV1::PermutationGather,
                    digest(26),
                    permutation_source_domain,
                    output_domain,
                    digest(48),
                    digest(49),
                    digest(52),
                    digest(53),
                    64,
                    64,
                    SemanticEvaluationOrderV1::Lexicographic,
                    SemanticCoverageBindingV1::TotalView,
                )
                .unwrap(),
            ],
            vec![
                SemanticOutputContractV1::new(
                    digest(25),
                    digest(26),
                    output_domain,
                    digest(50),
                    digest(51),
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn generated_composition_template_is_canonical_and_workload_neutral() {
        let source = CanonicalGeneratedVerusProofInputV3::new(
            GENERATED_COMPOSITION_THEOREM_V1.as_bytes().to_vec(),
        )
        .unwrap();
        let text = std::str::from_utf8(source.source()).unwrap();
        assert!(text.contains("fe2o3_exact_total_output_v1"));
        assert!(text.contains("fe2o3_finite_recurrence_refinement_v1"));
        for workload in ["gemm", "softmax", "attention", "moe"] {
            assert!(!text.to_ascii_lowercase().contains(workload));
        }
        assert!(!source.authenticates_verus_execution());
    }

    #[test]
    fn obligation_binds_every_contract_subject_source_and_receipt_identity() {
        let original = obligation_input();
        let expected = aggregate_obligation_from_input(&original);
        let mut mutations = Vec::new();
        for field in 0..5 {
            let mut changed = original.clone();
            match field {
                0 => changed.contract = digest(31),
                1 => changed.parallel_contract = digest(31),
                2 => changed.pliron_evidence = digest(31),
                3 => changed.template = digest(31),
                _ => changed.source = digest(31),
            }
            mutations.push(changed);
        }
        for field in 0..5 {
            let mut changed = original.clone();
            changed.subjects = FunctionalRefinementSubjectsV2::new(
                SafeReferenceKindV2::SourceAndMir,
                if field == 0 { digest(31) } else { digest(5) },
                if field == 1 { digest(31) } else { digest(18) },
                if field == 2 { digest(31) } else { digest(6) },
                if field == 3 { digest(31) } else { digest(7) },
                if field == 4 { digest(31) } else { digest(8) },
            )
            .unwrap();
            mutations.push(changed);
        }
        for field in 0..9 {
            let mut changed = original.clone();
            let receipt = &mut changed.receipts[0];
            match field {
                0 => receipt.receipt = digest(31),
                1 => receipt.effect = digest(31),
                2 => receipt.signer = digest(31),
                3 => receipt.execution = digest(31),
                _ => {
                    let mut identities =
                        [digest(13), digest(14), digest(15), digest(16), digest(17)];
                    identities[field - 4] = digest(31);
                    receipt.toolchain = VerusToolchainIdentityV2::new(
                        identities[0],
                        identities[1],
                        identities[2],
                        identities[3],
                        identities[4],
                    )
                    .unwrap();
                }
            }
            mutations.push(changed);
        }
        let mut removed = original.clone();
        removed.receipts.clear();
        mutations.push(removed);
        let mut duplicated = original.clone();
        duplicated.receipts.push(duplicated.receipts[0].clone());
        mutations.push(duplicated);

        assert_eq!(mutations.len(), 21);
        for mutation in mutations {
            assert_ne!(aggregate_obligation_from_input(&mutation), expected);
        }
    }

    #[test]
    fn generated_source_identity_changes_for_any_bound_comment_substitution() {
        let base = format!(
            "// contract={}\n{GENERATED_COMPOSITION_THEOREM_V1}",
            "01".repeat(32)
        );
        let changed = format!(
            "// contract={}\n{GENERATED_COMPOSITION_THEOREM_V1}",
            "02".repeat(32)
        );
        let base = CanonicalGeneratedVerusProofInputV3::new(base.into_bytes()).unwrap();
        let changed = CanonicalGeneratedVerusProofInputV3::new(changed.into_bytes()).unwrap();
        assert_ne!(base.identity(), changed.identity());
    }

    #[test]
    fn generated_contract_data_appears_in_logical_verus_premises() {
        let static_contract = pointwise_contract(SemanticFiniteExtentV1::Static(16));
        let static_parallel_contract = pointwise_parallel_contract(&static_contract);
        let mut static_source = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        append_contract_instantiations_v1(
            &mut static_source,
            &static_contract,
            &static_parallel_contract,
        )
        .unwrap();
        assert!(static_source.contains("actual.len() == fe2o3_output_0_bound_v1()"));
        assert!(static_source.contains("fe2o3_output_0_bound_v1() -> int { 16 }"));

        let dynamic_contract = pointwise_contract(SemanticFiniteExtentV1::Dynamic {
            symbol: 7,
            inclusive_upper_bound: 32,
        });
        let dynamic_parallel_contract = pointwise_parallel_contract(&dynamic_contract);
        let mut dynamic_source = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        append_contract_instantiations_v1(
            &mut dynamic_source,
            &dynamic_contract,
            &dynamic_parallel_contract,
        )
        .unwrap();
        assert!(dynamic_source.contains("actual.len() <= fe2o3_output_0_bound_v1()"));
        assert!(dynamic_source.contains("fe2o3_output_0_bound_v1() -> int { 32 }"));
        assert_ne!(static_source, dynamic_source);
    }

    #[test]
    fn checked_verus_fixture_is_emitted_by_the_production_generator() {
        let contract = generated_fixture_contract();
        let parallel_contract = pointwise_parallel_contract(&contract);
        let mut generated = "include!(\"mir_pliron_per_compilation_template_v1.rs\");\n".to_owned();
        append_contract_instantiations_v1(&mut generated, &contract, &parallel_contract).unwrap();
        assert_eq!(
            generated,
            include_str!("../verus/mir_pliron_per_compilation_generated_fixture_v1.rs")
        );
    }

    #[test]
    fn unavailable_retained_runtime_fails_closed_before_execution() {
        let error = FunctionalRefinementVerusRuntimeLeaseV1::open(
            "/opt/fe2o3/verus-runtime-v2/definitely-absent-per-compilation-test-v1",
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("retained generated-proof runtime failed")
        );
    }
}
