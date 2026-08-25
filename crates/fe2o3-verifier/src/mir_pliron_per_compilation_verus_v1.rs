//! Per-compilation Verus join for the exact MIR-to-live-PLIRON contract.
//!
//! This workload-neutral module consumes the move-only structural verifier
//! result, derives subjects from retained authenticated effect receipts,
//! generates one contract-specific composition proof, executes it in the
//! retained runtime, and imports the signed result. Neither source nor
//! semantic subjects are accepted from the caller.

use std::{error::Error, fmt, fmt::Write as _};

use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementReceiptIdentityV2, FunctionalRefinementSubjectsV2,
    MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1, VerusToolchainIdentityV2,
};
use fe2o3_pliron::{
    ProductionFunctionalRefinementTrustPolicyV2, ProductionMiddleEndEvidenceV5,
    ProductionMirPlironSemanticContractErrorV1, ProductionMirPlironSemanticContractReportV1,
    ProductionRankedKernelLoweringInputV1, ProductionTotalOutputRefinementErrorV2,
    ProductionVerifiedMirPlironKernelV1, require_mir_pliron_semantic_contract_v1,
    require_total_output_refinement_v2,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::functional_refinement_receipt_v2::execute_and_import_generated_functional_refinement_locally_v2;
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
    pliron_evidence_identity: DigestV1,
    shared_theorem_identity: DigestV1,
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
    pub const fn shared_theorem_identity(self) -> DigestV1 {
        self.shared_theorem_identity
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
    pub const fn retained_effect_receipts_supply_arithmetic_premises(self) -> bool {
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
    structural: ProductionVerifiedMirPlironKernelV1,
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
    pub const fn shared_theorem_is_conditional_composition(&self) -> bool {
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
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes and imports one aggregate proof for the exact structurally admitted
/// compilation. Runtime absence, unsupported source, proof failure, stale
/// identities, and receipt import failure all return `Err`.
pub fn execute_mir_pliron_semantic_contract_per_compilation_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    structural: ProductionVerifiedMirPlironKernelV1,
    timeout_seconds: u32,
) -> Result<ProductionVerusVerifiedMirPlironKernelV1, ProductionMirPlironPerCompilationVerusErrorV1>
{
    let (aggregate, policy) = execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1(
        runtime,
        structural.ranked(),
        structural.evidence(),
        structural.semantic_contract(),
        structural.semantic_contract_report(),
        timeout_seconds,
    )?;
    Ok(ProductionVerusVerifiedMirPlironKernelV1 {
        structural,
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
    let subjects = derive_compiler_subjects(ranked, contract)?;
    let source = generate_contract_source(contract, structural_report)?;
    let generated_source_identity = DigestV1::from_untrusted_bytes(source.identity().as_bytes());
    let obligation_identity =
        aggregate_obligation_identity(ranked, contract, generated_source_identity, subjects);
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation_identity)
        .map_err(|error| {
            ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(error.to_string())
        })?;
    let (imported, policy) = execute_and_import_generated_functional_refinement_locally_v2(
        runtime,
        source,
        binding,
        timeout_seconds,
    )
    .map_err(ProductionMirPlironPerCompilationVerusErrorV1::Execution)?;
    if imported.binding() != binding
        || imported.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
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
        pliron_evidence_identity: contract.pliron_evidence(),
        shared_theorem_identity: MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1,
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
        "// reviewed shared theorem: {}",
        hex(MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1),
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
    source.push_str(GENERATED_COMPOSITION_THEOREM_V1);
    CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(|error| {
        ProductionMirPlironPerCompilationVerusErrorV1::GeneratedSource(error.to_string())
    })
}

fn aggregate_obligation_identity(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
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
        pliron_evidence: contract.pliron_evidence(),
        theorem: MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1,
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
    pliron_evidence: DigestV1,
    theorem: DigestV1,
    source: DigestV1,
    subjects: FunctionalRefinementSubjectsV2,
    receipts: Vec<AggregateReceiptBindingV1>,
}

fn aggregate_obligation_from_input(input: &AggregateObligationInputV1) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, AGGREGATE_OBLIGATION_DOMAIN_V1);
    for identity in [
        input.contract,
        input.pliron_evidence,
        input.theorem,
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
    use fe2o3_functional_proof::SafeReferenceKindV2;

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    fn obligation_input() -> AggregateObligationInputV1 {
        AggregateObligationInputV1 {
            contract: digest(1),
            pliron_evidence: digest(2),
            theorem: digest(3),
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

    #[test]
    fn generated_composition_theorem_is_canonical_and_workload_neutral() {
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
        for field in 0..4 {
            let mut changed = original.clone();
            match field {
                0 => changed.contract = digest(31),
                1 => changed.pliron_evidence = digest(31),
                2 => changed.theorem = digest(31),
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

        assert_eq!(mutations.len(), 20);
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
