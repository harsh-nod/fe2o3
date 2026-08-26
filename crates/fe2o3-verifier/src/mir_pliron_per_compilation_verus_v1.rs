//! Per-compilation Verus join for the exact MIR-to-live-PLIRON contract.
//!
//! This workload-neutral module consumes the move-only structural verifier
//! result, derives subjects from retained authenticated effect receipts,
//! regenerates each supported role-bound ranked effect formula, executes those
//! formulas in the retained runtime, and imports the signed result. Receipts
//! remain authenticated binding inputs, never logical axioms. PLIRON performs
//! total-coverage and output-product reconciliation. Schedules and numerical
//! policies without replayable formulas fail closed. Neither source nor
//! semantic subjects are accepted from the caller.

use std::{error::Error, fmt, fmt::Write as _};

use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementReceiptIdentityV2, FunctionalRefinementSubjectsV2,
    ParallelNumericalPolicyV1, ParallelReferenceContractV1, ParallelScheduleRelationV1,
    VerusToolchainIdentityV2,
};
use fe2o3_pliron::{
    HARD_MAX_SESSION_OPERATION_TREE_ITEMS, ProductionMiddleEndEvidenceV5,
    ProductionMirPlironSemanticContractErrorV1, ProductionMirPlironSemanticContractReportV1,
    ProductionParallelReferenceContractErrorV1, ProductionParallelReferenceContractReportV1,
    ProductionRankedKernelLoweringInputV1, ProductionReconciledMirPlironKernelV1,
    ProductionRefinementStagingPolicyV2, ProductionTotalOutputStagingErrorV2,
    require_mir_pliron_semantic_contract_v1, require_parallel_reference_contract_v1,
    require_total_output_staging_v2,
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
pub const MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1: usize =
    fe2o3_functional_proof::HARD_MAX_AGGREGATE_FUNCTIONAL_OUTPUTS_V1;
const MAX_PRODUCTION_AGGREGATE_FORMULA_SCAN_WORK_V1: usize =
    MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1 * HARD_MAX_SESSION_OPERATION_TREE_ITEMS;

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
    retained_refinement_receipts: u64,
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
    pub const fn retained_refinement_receipts(self) -> u64 {
        self.retained_refinement_receipts
    }
    pub const fn has_authenticated_per_compilation_verus_execution(self) -> bool {
        true
    }
    pub const fn binding_includes_exact_safe_reference_kernel_mir_and_live_pliron(self) -> bool {
        true
    }
    pub const fn replays_each_supported_output_effect_formula(self) -> bool {
        true
    }
    pub const fn total_coverage_and_product_are_structurally_reconciled(self) -> bool {
        true
    }
    pub const fn ieee_replay_is_compiler_operator_congruence_not_target_value_semantics(
        self,
    ) -> bool {
        true
    }
    pub const fn compiler_extraction_projection_and_pass_soundness_remain_trusted(self) -> bool {
        true
    }
    pub const fn generated_identity_comments_are_binding_inputs_not_verus_premises(self) -> bool {
        true
    }
    pub const fn retained_refinement_receipts_are_authenticated_binding_inputs(self) -> bool {
        true
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
    structural: ProductionReconciledMirPlironKernelV1,
    parallel_contract: ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
    aggregate: ProductionMirPlironPerCompilationVerusReportV1,
    _staging_policy: ProductionRefinementStagingPolicyV2,
}

impl ProductionVerusVerifiedMirPlironKernelV1 {
    pub const fn structural(&self) -> &ProductionReconciledMirPlironKernelV1 {
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
    pub const fn retains_authenticated_output_effect_formula_replay(&self) -> bool {
        true
    }
    pub const fn compiler_extraction_projection_and_pass_soundness_remain_trusted(&self) -> bool {
        true
    }
    pub const fn generated_identity_comments_are_not_verus_premises(&self) -> bool {
        true
    }
    pub const fn output_product_composition_is_structural_not_a_verus_sequence_model(
        &self,
    ) -> bool {
        true
    }
    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionMirPlironPerCompilationVerusErrorV1 {
    TotalOutput(ProductionTotalOutputStagingErrorV2),
    SemanticContract(ProductionMirPlironSemanticContractErrorV1),
    ParallelContract(ProductionParallelReferenceContractErrorV1),
    ParallelReportMismatch,
    StructuralReportMismatch,
    MissingRetainedEffectReceipt,
    InconsistentRetainedSubjects,
    WrongRetainedBoundary,
    CounterOverflow,
    GeneratedSource(String),
    UnsupportedFormulaReplayRole { output: usize, role: &'static str },
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
            Self::UnsupportedFormulaReplayRole { output, role } => write!(
                formatter,
                "output {output} requires unsupported formula-replay role `{role}`; no aggregate functional authority was granted",
            ),
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

/// Executes and imports one aggregate formula-replay program for the exact
/// structurally admitted compilation. Validated receipts select exact roles;
/// the generated Verus source independently reconstructs their ranked formulas.
/// Runtime absence, unsupported source, proof failure, stale identities, and
/// receipt import failure all return `Err`.
pub fn execute_mir_pliron_semantic_contract_per_compilation_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    structural: ProductionReconciledMirPlironKernelV1,
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
        _staging_policy: policy,
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
        ProductionRefinementStagingPolicyV2,
    ),
    ProductionMirPlironPerCompilationVerusErrorV1,
> {
    let total_output = require_total_output_staging_v2(ranked, evidence)
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
        ranked,
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
    let retained_refinement_receipts =
        u64::try_from(ranked.retained_policy_checked_refinement_staging().len())
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
        retained_refinement_receipts,
    };
    Ok((aggregate, policy))
}

fn derive_compiler_subjects(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
) -> Result<FunctionalRefinementSubjectsV2, ProductionMirPlironPerCompilationVerusErrorV1> {
    let receipts = ranked.retained_policy_checked_refinement_staging();
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
    ranked: &ProductionRankedKernelLoweringInputV1,
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
        "// output product: {}",
        hex(parallel_contract.output_product_identity()),
    )
    .map_err(generated_format_error)?;
    for (index, relation) in parallel_contract.relations().iter().enumerate() {
        writeln!(
            source,
            "// output[{index}] relation={} view={} ownership={} frame={} effect_receipt={}",
            hex(relation.identity()),
            hex(relation.ranked_view_identity()),
            hex(relation.ownership_identity()),
            hex(relation.frame_identity()),
            hex(relation.policy_checked_staging_identity()),
        )
        .map_err(generated_format_error)?;
    }
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
        "// relations={} frames={} pointwise={} permutations={} folds={} recurrences={}",
        parallel_report.output_relations(),
        parallel_report.output_frames(),
        parallel_report.pointwise_relations(),
        parallel_report.permutation_relations(),
        parallel_report.fold_relations(),
        parallel_report.bounded_recurrences(),
    )
    .map_err(generated_format_error)?;
    source.push_str(GENERATED_COMPOSITION_THEOREM_V1);
    append_contract_instantiations_v1(&mut source, ranked.kernel(), contract, parallel_contract)?;
    CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(|error| {
        ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(error.to_string())
    })
}

fn append_contract_instantiations_v1(
    source: &mut String,
    kernel: &fe2o3_pliron::ProductionRankedKernelV1,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    parallel_contract: &ParallelReferenceContractV1,
) -> Result<(), ProductionMirPlironPerCompilationVerusErrorV1> {
    require_aggregate_output_limit_v1(contract.outputs().len())?;
    if parallel_contract.relations().len() != contract.outputs().len() {
        return Err(
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(
                "parallel output product arity differs from semantic outputs".to_owned(),
            ),
        );
    }
    let mut effect_sites =
        std::collections::BTreeMap::<(DigestV1, DigestV1), Vec<(usize, usize)>>::new();
    for (block, body) in kernel.blocks().iter().enumerate() {
        for (operation, item) in body.operations().iter().enumerate() {
            if let fe2o3_pliron::ProductionRankedOperationV1::RequireEffectRefinement {
                contract,
                proof,
            } = item
            {
                effect_sites
                    .entry((
                        fe2o3_pliron::production_effect_contract_identity_v1(
                            contract.contract_identity(),
                        ),
                        proof.receipt_identity().digest(),
                    ))
                    .or_default()
                    .push((block, operation));
            }
        }
    }
    let mut replays = Vec::with_capacity(contract.outputs().len());
    for (index, output) in contract.outputs().iter().enumerate() {
        let relation = &parallel_contract.relations()[index];
        if relation.tensor_refinement_identity().is_some() {
            return Err(
                ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                    output: index,
                    role: "tensor-component-composition",
                },
            );
        }
        let unsupported_schedule = match relation.schedule() {
            ParallelScheduleRelationV1::PointwiseBijection => None,
            ParallelScheduleRelationV1::Permutation { .. } => Some("permutation-schedule-proof"),
            ParallelScheduleRelationV1::Fold { .. } => Some("fold-schedule-proof"),
            ParallelScheduleRelationV1::BoundedRecurrence { .. } => {
                Some("bounded-recurrence-schedule-proof")
            }
        };
        if let Some(role) = unsupported_schedule {
            return Err(
                ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                    output: index,
                    role,
                },
            );
        }
        match relation.numerical_policy() {
            ParallelNumericalPolicyV1::ExactBitVector
            | ParallelNumericalPolicyV1::IeeeOperatorCongruence { .. } => {}
            ParallelNumericalPolicyV1::ErrorBounded { .. } => {
                return Err(
                    ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                        output: index,
                        role: "finite-error-formula-replay",
                    },
                );
            }
            ParallelNumericalPolicyV1::UnboundedRelaxed => {
                return Err(
                    ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                        output: index,
                        role: "unbounded-relaxed-numerical-proof",
                    },
                );
            }
        }
        let candidates = effect_sites
            .get(&(
                output.identity(),
                relation.policy_checked_staging_identity(),
            ))
            .map(Vec::as_slice)
            .unwrap_or_default();
        let [(block, operation)] = candidates else {
            return Err(
                ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(format!(
                    "output {index} does not have exactly one role-bound effect formula to replay"
                )),
            );
        };
        let replay =
            crate::functional_refinement_receipt_v2::generate_ranked_effect_formula_replay_v2(
                kernel,
                *block,
                *operation,
                &format!("fe2o3_output_{index}_effect_formula_v1"),
            )
            .map_err(|error| {
                ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(format!(
                    "output {index} effect-formula replay failed: {error}",
                ))
            })?;
        replays.push(replay);
    }

    source.push_str("\nverus! {\n");
    source.push_str(
        crate::functional_refinement_receipt_v2::ranked_effect_formula_replay_prelude_v2(),
    );
    source.push_str(
        crate::functional_refinement_receipt_v2::ranked_effect_ieee_congruence_declaration_v2(),
    );
    source.push('\n');
    let mut all_symbols = std::collections::BTreeSet::new();
    for replay in &replays {
        if source
            .len()
            .checked_add(replay.lemma().len())
            .is_none_or(|length| length > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3)
        {
            return Err(
                ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(
                    "aggregate effect-formula replay exceeds the generated-source byte limit"
                        .to_owned(),
                ),
            );
        }
        source.push_str(replay.lemma());
        all_symbols.extend(replay.symbols().iter().copied());
    }
    source.push_str("    proof fn fe2o3_replay_all_output_effect_formulas_v1(");
    for (index, symbol) in all_symbols.iter().enumerate() {
        if index != 0 {
            source.push_str(", ");
        }
        write!(source, "s{symbol}: int").map_err(generated_format_error)?;
    }
    source.push_str(") {\n");
    for (index, replay) in replays.iter().enumerate() {
        write!(source, "        fe2o3_output_{index}_effect_formula_v1(")
            .map_err(generated_format_error)?;
        for (symbol_index, symbol) in replay.symbols().iter().enumerate() {
            if symbol_index != 0 {
                source.push_str(", ");
            }
            write!(source, "s{symbol}").map_err(generated_format_error)?;
        }
        source.push_str(");\n");
    }
    source.push_str("    }\n}\n\nfn fe2o3_contract_instantiations_v1() {}\n");
    if source.len() > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
        return Err(
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(
                "aggregate effect-formula replay exceeds the generated-source byte limit"
                    .to_owned(),
            ),
        );
    }
    Ok(())
}

fn require_aggregate_output_limit_v1(
    outputs: usize,
) -> Result<(), ProductionMirPlironPerCompilationVerusErrorV1> {
    let scan_work = outputs
        .checked_mul(HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
        .ok_or_else(|| {
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(
                "aggregate effect-formula scan work overflowed".to_owned(),
            )
        })?;
    if outputs > MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1
        || scan_work > MAX_PRODUCTION_AGGREGATE_FORMULA_SCAN_WORK_V1
    {
        return Err(
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(format!(
                "aggregate effect-formula replay has {outputs} outputs; the production limit is {MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1}",
            )),
        );
    }
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
        .retained_policy_checked_refinement_staging()
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
    use dialect_kernel::{AccessKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr};
    use ed25519_dalek::{Signer as _, SigningKey};
    use fe2o3_functional_proof::{
        COMPLETE_GPU_HIERARCHY_V1, FunctionalRefinementBoundaryV2,
        FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
        FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
        MirPlironSemanticContractV1, ParallelFoldOrderV1, ParallelNumericalPolicyV1,
        ParallelOutputRelationV1, ParallelReferenceContractV1, ParallelScheduleRelationV1,
        SafeReferenceKindV2, SemanticEvaluationOrderV1, SemanticFiniteDomainV1,
        SemanticFiniteExtentV1, SemanticNumericalPolicyV1, SemanticOutputContractV1,
        SemanticScalarTypeV1, SemanticTypedRootV1, UnsignedFunctionalRefinementReceiptV2,
    };
    use fe2o3_pliron::{
        ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2,
        ProductionNumericalContractV2, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
        ProductionRankedValueV1, ProductionReferenceOutputSiteV2, ProductionReferenceProofV2,
        ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2,
        normalized_effect_refinement_hash_for_kernel_v2, production_effect_contract_identity_v1,
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
                    production_effect_contract_identity_v1(73),
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
        effect_proof: DigestV1,
    ) -> ParallelReferenceContractV1 {
        let output = &contract.outputs()[0];
        ParallelReferenceContractV1::new(
            contract.canonical_sha256(),
            digest(34),
            vec![
                ParallelOutputRelationV1::new(
                    digest(27),
                    output.identity(),
                    output.output_domain(),
                    digest(31),
                    digest(32),
                    digest(33),
                    ParallelScheduleRelationV1::PointwiseBijection,
                    ParallelNumericalPolicyV1::ExactBitVector,
                    COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                    None,
                    effect_proof,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn replay_subjects() -> FunctionalRefinementSubjectsV2 {
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(61),
            DigestV1::ZERO,
            digest(62),
            digest(63),
            digest(64),
        )
        .unwrap()
    }

    fn bound_effect_kernel() -> (ProductionRankedKernelV1, DigestV1) {
        let local = |value| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(value));
        let contract = ProductionEffectRefinementContractV2::new(
            73,
            ProductionGpuWriteSiteV2::new(0, 8),
            ProductionReferenceOutputSiteV2::new(0, 0, 0),
            local(0),
            vec![local(1)],
            vec![local(5)],
            vec![local(5)],
            local(4),
            local(4),
            local(4),
            local(4),
            local(2),
            local(3),
        )
        .unwrap();
        let scalar_u32 = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let skeleton = ProductionRankedKernelV1::new(
            "aggregate_formula_replay",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::View {
                        result: ProductionRankedValueIdV1::new(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(1),
                        value: 0,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(2),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(3),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(4),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: ProductionSemanticScalarTypeV2::Bool,
                            bits: 1,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(5),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: ProductionSemanticScalarTypeV2::Integer {
                                signed: false,
                                bits: 64,
                            },
                            bits: 0,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(1)],
                        value: local(2),
                    },
                    ProductionRankedOperationV1::RequestEffectRefinement {
                        contract,
                        subjects: replay_subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
            &skeleton.blocks()[0].operations()[9]
        else {
            unreachable!()
        };
        let obligation = normalized_effect_refinement_hash_for_kernel_v2(
            &skeleton,
            0,
            9,
            contract,
            replay_subjects(),
        )
        .unwrap();
        let binding =
            FunctionalRefinementBindingV2::from_subjects(replay_subjects(), obligation).unwrap();
        let signing = SigningKey::from_bytes(&[71; 32]);
        let toolchain = VerusToolchainIdentityV2::new(
            digest(72),
            digest(73),
            digest(74),
            digest(75),
            digest(76),
        )
        .unwrap();
        let policy = FunctionalRefinementImportPolicyV2::new(
            signing.verifying_key().to_bytes(),
            toolchain,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
            policy.signer_identity(),
            binding,
            toolchain,
            digest(77),
            FunctionalRefinementResultV2::Proved,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let wire = unsigned
            .clone()
            .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
        let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
        let imported = importer
            .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
            .unwrap();
        let proof = imported.receipt_identity().digest();
        let request =
            ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding);
        (
            skeleton
                .bind_functional_refinement_request_v2(0, 9, request)
                .unwrap(),
            proof,
        )
    }

    fn bound_two_effect_kernel() -> (ProductionRankedKernelV1, [DigestV1; 2]) {
        let local = |value| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(value));
        let first = ProductionEffectRefinementContractV2::new(
            73,
            ProductionGpuWriteSiteV2::new(0, 12),
            ProductionReferenceOutputSiteV2::new(0, 0, 0),
            local(0),
            vec![local(2)],
            vec![local(8)],
            vec![local(8)],
            local(7),
            local(7),
            local(7),
            local(7),
            local(3),
            local(4),
        )
        .unwrap();
        let second = ProductionEffectRefinementContractV2::new(
            74,
            ProductionGpuWriteSiteV2::new(0, 14),
            ProductionReferenceOutputSiteV2::new(0, 1, 0),
            local(1),
            vec![local(2)],
            vec![local(8)],
            vec![local(8)],
            local(7),
            local(7),
            local(7),
            local(7),
            local(5),
            local(6),
        )
        .unwrap();
        let scalar_u32 = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let exact = ProductionNumericalContractV2::ExactBitVectorOperatorCongruence;
        let skeleton = ProductionRankedKernelV1::new(
            "aggregate_two_output_formula_replay",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::View {
                        result: ProductionRankedValueIdV1::new(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::View {
                        result: ProductionRankedValueIdV1::new(1),
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 2,
                        noalias_class: 2,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(2),
                        value: 0,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(3),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(4),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(5),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 9,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(6),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 9,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(7),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: ProductionSemanticScalarTypeV2::Bool,
                            bits: 1,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(8),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: ProductionSemanticScalarTypeV2::Integer {
                                signed: false,
                                bits: 64,
                            },
                            bits: 0,
                        },
                        numerical_contract: exact,
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(1),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(2)],
                        value: local(3),
                    },
                    ProductionRankedOperationV1::RequestEffectRefinement {
                        contract: first,
                        subjects: replay_subjects(),
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(1),
                        indices: vec![local(2)],
                        value: local(5),
                    },
                    ProductionRankedOperationV1::RequestEffectRefinement {
                        contract: second,
                        subjects: replay_subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let (first_request, first_proof) = test_effect_request(&skeleton, 13, 81);
        let (second_request, second_proof) = test_effect_request(&skeleton, 15, 91);
        (
            skeleton
                .bind_functional_refinement_request_v2(0, 13, first_request)
                .unwrap()
                .bind_functional_refinement_request_v2(0, 15, second_request)
                .unwrap(),
            [first_proof, second_proof],
        )
    }

    fn test_effect_request(
        kernel: &ProductionRankedKernelV1,
        operation: usize,
        seed: u8,
    ) -> (ProductionReferenceProofV2, DigestV1) {
        let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
            &kernel.blocks()[0].operations()[operation]
        else {
            unreachable!()
        };
        let obligation = normalized_effect_refinement_hash_for_kernel_v2(
            kernel,
            0,
            operation,
            contract,
            replay_subjects(),
        )
        .unwrap();
        let binding =
            FunctionalRefinementBindingV2::from_subjects(replay_subjects(), obligation).unwrap();
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let toolchain = VerusToolchainIdentityV2::new(
            digest(seed.wrapping_add(1)),
            digest(seed.wrapping_add(2)),
            digest(seed.wrapping_add(3)),
            digest(seed.wrapping_add(4)),
            digest(seed.wrapping_add(5)),
        )
        .unwrap();
        let policy = FunctionalRefinementImportPolicyV2::new(
            signing.verifying_key().to_bytes(),
            toolchain,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
            policy.signer_identity(),
            binding,
            toolchain,
            digest(seed.wrapping_add(6)),
            FunctionalRefinementResultV2::Proved,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let wire = unsigned
            .clone()
            .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
        let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
        let imported = importer
            .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
            .unwrap();
        (
            ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
            imported.receipt_identity().digest(),
        )
    }

    fn two_output_contract() -> MirPlironSemanticContractV1 {
        let first_domain = digest(101);
        let second_domain = digest(102);
        let root = |identity, domain| {
            SemanticTypedRootV1::new(
                digest(identity),
                digest(identity.wrapping_add(10)),
                domain,
                SemanticScalarTypeV1::Unsigned(32),
                SemanticNumericalPolicyV1::ExactBitVector,
            )
            .unwrap()
        };
        MirPlironSemanticContractV1::new(
            digest(18),
            digest(19),
            digest(20),
            vec![
                SemanticFiniteDomainV1::new(first_domain, vec![SemanticFiniteExtentV1::Static(1)])
                    .unwrap(),
                SemanticFiniteDomainV1::new(second_domain, vec![SemanticFiniteExtentV1::Static(1)])
                    .unwrap(),
            ],
            vec![
                root(103, first_domain),
                root(104, first_domain),
                root(105, second_domain),
                root(106, second_domain),
            ],
            vec![],
            vec![],
            vec![
                SemanticOutputContractV1::new(
                    production_effect_contract_identity_v1(73),
                    digest(107),
                    first_domain,
                    digest(103),
                    digest(104),
                    vec![],
                )
                .unwrap(),
                SemanticOutputContractV1::new(
                    production_effect_contract_identity_v1(74),
                    digest(108),
                    second_domain,
                    digest(105),
                    digest(106),
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn two_output_parallel_contract(
        contract: &MirPlironSemanticContractV1,
        proofs: [DigestV1; 2],
    ) -> ParallelReferenceContractV1 {
        let relations = contract
            .outputs()
            .iter()
            .zip(proofs)
            .enumerate()
            .map(|(index, (output, proof))| {
                ParallelOutputRelationV1::new(
                    digest(110 + index as u8),
                    output.identity(),
                    output.output_domain(),
                    digest(112 + index as u8),
                    digest(114 + index as u8),
                    digest(116 + index as u8),
                    ParallelScheduleRelationV1::PointwiseBijection,
                    ParallelNumericalPolicyV1::ExactBitVector,
                    COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                    None,
                    proof,
                )
                .unwrap()
            })
            .collect();
        ParallelReferenceContractV1::new(contract.canonical_sha256(), digest(118), relations)
            .unwrap()
    }
    fn relation_contract(
        contract: &MirPlironSemanticContractV1,
        schedule: ParallelScheduleRelationV1,
        numerical_policy: ParallelNumericalPolicyV1,
        effect_proof: DigestV1,
    ) -> ParallelReferenceContractV1 {
        let output = &contract.outputs()[0];
        ParallelReferenceContractV1::new(
            contract.canonical_sha256(),
            digest(34),
            vec![
                ParallelOutputRelationV1::new(
                    digest(27),
                    output.identity(),
                    output.output_domain(),
                    digest(31),
                    digest(32),
                    digest(33),
                    schedule,
                    numerical_policy,
                    COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                    None,
                    effect_proof,
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
        assert!(text.contains("caller-provided relation premises"));
        for forbidden in ["requires", "assume(", "admit(", "external_body"] {
            assert!(!text.contains(forbidden));
        }
        for workload in ["gemm", "softmax", "attention", "moe"] {
            assert!(!text.to_ascii_lowercase().contains(workload));
        }
        assert!(!source.authenticates_verus_execution());
    }

    #[test]
    fn aggregate_formula_replay_has_a_conservative_whole_compilation_output_limit() {
        assert!(require_aggregate_output_limit_v1(64).is_ok());
        let error = require_aggregate_output_limit_v1(65).unwrap_err();
        assert!(error.to_string().contains("production limit is 64"));
    }

    #[test]
    fn generated_two_output_replay_and_output_one_substitution_match_pinned_fixtures() {
        let (kernel, proofs) = bound_two_effect_kernel();
        let contract = two_output_contract();
        let parallel = two_output_parallel_contract(&contract, proofs);
        let mut generated = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        append_contract_instantiations_v1(&mut generated, &kernel, &contract, &parallel).unwrap();
        assert_eq!(
            generated,
            include_str!(
                "../verus/mir_pliron_per_compilation_generated_multi_output_fixture_v1.rs"
            )
        );

        let needle = "let v6: int = fe2o3_bv_norm_v2(9, 32);";
        assert_eq!(generated.matches(needle).count(), 1);
        let substituted = generated.replacen(needle, "let v6: int = fe2o3_bv_norm_v2(10, 32);", 1);
        assert_eq!(
            substituted,
            include_str!(
                "../verus/negative/mir_pliron_per_compilation_multi_output_substitution_v1.rs"
            )
        );
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
    fn exact_output_replays_the_role_bound_ranked_formula_without_relation_premises() {
        let (kernel, proof) = bound_effect_kernel();
        let contract = pointwise_contract(SemanticFiniteExtentV1::Static(1));
        let parallel = pointwise_parallel_contract(&contract, proof);
        let mut generated = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        append_contract_instantiations_v1(&mut generated, &kernel, &contract, &parallel).unwrap();
        assert!(generated.contains("proof fn fe2o3_output_0_effect_formula_v1("));
        assert!(generated.contains("assert(v2 == v3);"));
        assert!(generated.contains("fe2o3_output_0_effect_formula_v1("));
        assert!(generated.contains("proof fn fe2o3_replay_all_output_effect_formulas_v1("));
        assert!(!generated.contains("output_product_refines"));
        for forbidden in ["requires", "assume(", "admit(", "external_body"] {
            assert!(!generated.contains(forbidden));
        }
        assert_eq!(
            generated,
            include_str!("../verus/mir_pliron_per_compilation_generated_fixture_v1.rs")
        );
    }

    #[test]
    fn stale_or_swapped_effect_receipt_role_has_no_formula_to_replay() {
        let (kernel, _) = bound_effect_kernel();
        let contract = pointwise_contract(SemanticFiniteExtentV1::Static(1));
        let parallel = pointwise_parallel_contract(&contract, digest(99));
        let error = append_contract_instantiations_v1(
            &mut GENERATED_COMPOSITION_THEOREM_V1.to_owned(),
            &kernel,
            &contract,
            &parallel,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not have exactly one role-bound effect formula to replay")
        );
    }

    #[test]
    fn finite_error_never_falls_back_to_exact_integer_or_sequence_equality() {
        let (kernel, proof) = bound_effect_kernel();
        let contract = pointwise_contract(SemanticFiniteExtentV1::Static(1));
        let parallel = relation_contract(
            &contract,
            ParallelScheduleRelationV1::PointwiseBijection,
            ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: 1.0_f64.to_bits(),
                relative_error_f64_bits: 0.125_f64.to_bits(),
                witness_root: digest(41),
                proof: digest(42),
            },
            proof,
        );
        let mut generated = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        let error =
            append_contract_instantiations_v1(&mut generated, &kernel, &contract, &parallel)
                .unwrap_err();
        assert!(matches!(
            error,
            ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                output: 0,
                role: "finite-error-formula-replay",
            }
        ));
        assert_eq!(generated, GENERATED_COMPOSITION_THEOREM_V1);
    }

    #[test]
    fn tensor_component_receipt_never_grants_unreplayed_aggregate_authority() {
        let (kernel, proof) = bound_effect_kernel();
        let contract = pointwise_contract(SemanticFiniteExtentV1::Static(1));
        let scalar_parallel = relation_contract(
            &contract,
            ParallelScheduleRelationV1::PointwiseBijection,
            ParallelNumericalPolicyV1::ExactBitVector,
            proof,
        );
        let relation = &scalar_parallel.relations()[0];
        let parallel = ParallelReferenceContractV1::new(
            contract.canonical_sha256(),
            digest(34),
            vec![
                ParallelOutputRelationV1::new(
                    relation.identity(),
                    relation.output_contract(),
                    relation.logical_domain(),
                    relation.ranked_view_identity(),
                    relation.ownership_identity(),
                    relation.frame_identity(),
                    relation.schedule(),
                    relation.numerical_policy(),
                    relation.hierarchy().to_vec(),
                    Some(digest(74)),
                    relation.policy_checked_staging_identity(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut generated = GENERATED_COMPOSITION_THEOREM_V1.to_owned();
        let error =
            append_contract_instantiations_v1(&mut generated, &kernel, &contract, &parallel)
                .unwrap_err();
        assert!(matches!(
            error,
            ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                output: 0,
                role: "tensor-component-composition",
            }
        ));
        assert_eq!(generated, GENERATED_COMPOSITION_THEOREM_V1);
    }

    #[test]
    fn schedules_without_replayable_formula_roles_fail_closed() {
        let contract = pointwise_contract(SemanticFiniteExtentV1::Static(16));
        let schedules = [
            ParallelScheduleRelationV1::Permutation {
                collective: digest(50),
            },
            ParallelScheduleRelationV1::Fold {
                collective: digest(51),
                order: ParallelFoldOrderV1::Preserved,
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            },
            ParallelScheduleRelationV1::BoundedRecurrence {
                collective: digest(52),
                loop_contract: digest(53),
                dynamic_bound_proof: None,
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            },
        ];
        for schedule in schedules {
            let parallel = relation_contract(
                &contract,
                schedule,
                ParallelNumericalPolicyV1::ExactBitVector,
                digest(28),
            );
            let error = append_contract_instantiations_v1(
                &mut GENERATED_COMPOSITION_THEOREM_V1.to_owned(),
                &bound_effect_kernel().0,
                &contract,
                &parallel,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ProductionMirPlironPerCompilationVerusErrorV1::UnsupportedFormulaReplayRole {
                    output: 0,
                    ..
                }
            ));
            assert!(
                error
                    .to_string()
                    .contains("no aggregate functional authority")
            );
        }
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
