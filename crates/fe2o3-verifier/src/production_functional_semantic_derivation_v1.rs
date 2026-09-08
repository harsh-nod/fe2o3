//! Verifier-owned derivation of functional expressions from retained IR.
//!
//! This gate deliberately accepts no caller-authored expression or semantic
//! plan. It resolves each reference output from an independently retained Rust
//! MIR owner, normalizes that MIR expression, and compares it with the exact
//! reference expression embedded in the final ranked kernel. It also checks
//! that the corresponding ranked write consumes the declared actual root.
//! Digests bind the resulting receipt; they never establish these facts.
//!
//! The accepted reference-expression class is currently straight-line scalar
//! bool, integer, and f32/f64 operator DAGs over constants and whole scalar
//! arguments. Projected loads, joins/phis, loop-carried state, transcendental
//! functions such as exp/sigmoid, relaxed finite-error claims, permutation and
//! fold semantics, and cooperative-tensor composition fail closed. In
//! particular, the bounded KDA recurrence generator remains test-only until
//! typed recurrence state/input roles are independently extractable from both
//! retained IRs.

use std::{collections::BTreeSet, error::Error, fmt};

use dialect_kernel::AccessKindAttr;
use fe2o3_functional_proof::ParallelReferenceContractV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticCastKindV1, SemanticConstantValueV1, SemanticFunctionDeclV1,
    SemanticLocalRoleV1, SemanticOperandV1, SemanticRvalueKindV1, SemanticScalarTypeV1,
    SemanticStatementKindV1, SemanticTypeShapeV1, SemanticUnaryOpV1,
};
use fe2o3_pliron::{
    PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2, ProductionEffectRefinementContractV2,
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractReportV1,
    ProductionNumericalContractV2, ProductionOverflowContractV2,
    ProductionParallelReferenceContractReportV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedValueV1,
    ProductionReconciledMirPlironKernelV1, ProductionSemanticBinaryOpV2, ProductionSemanticCastV2,
    ProductionSemanticExpressionV2, ProductionSemanticMirOwnerV1, ProductionSemanticScalarTypeV2,
    ProductionSemanticUnaryOpV2, production_effect_contract_identity_v1,
    require_parallel_reference_contract_v1,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

const OUTPUT_EXPRESSION_PRODUCT_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION/IR-DERIVED-FUNCTIONAL-OUTPUT-PRODUCT/V1\0";
const MAX_REFERENCE_EXPRESSION_DEPTH_V1: usize = 128;

/// IR-derived expression correspondence for one complete output product.
///
/// Construction is available only through
/// [`derive_production_functional_semantics_from_ir_v1`]. The value proves no
/// arithmetic theorem by itself; it is the mandatory derivation input to the
/// generated Verus stage.
#[derive(Debug)]
#[must_use = "dropping this value abandons the independently derived functional semantics"]
pub struct ProductionIrDerivedFunctionalSemanticsV1 {
    safe_reference_mir: DigestV1,
    kernel_mir: DigestV1,
    ranked_kernel: DigestV1,
    semantic_contract: DigestV1,
    parallel_contract: DigestV1,
    output_expression_product: DigestV1,
    outputs: u64,
}

impl ProductionIrDerivedFunctionalSemanticsV1 {
    pub const fn safe_reference_mir(&self) -> DigestV1 {
        self.safe_reference_mir
    }
    pub const fn kernel_mir(&self) -> DigestV1 {
        self.kernel_mir
    }
    pub const fn ranked_kernel(&self) -> DigestV1 {
        self.ranked_kernel
    }
    pub const fn semantic_contract(&self) -> DigestV1 {
        self.semantic_contract
    }
    pub const fn parallel_contract(&self) -> DigestV1 {
        self.parallel_contract
    }
    pub const fn output_expression_product(&self) -> DigestV1 {
        self.output_expression_product
    }
    pub const fn outputs(&self) -> u64 {
        self.outputs
    }
    pub const fn expressions_are_derived_from_retained_ir(&self) -> bool {
        true
    }
    pub const fn grants_proof_or_compiler_authority(&self) -> bool {
        false
    }
}

/// Fail-closed errors from independent reference-MIR/ranked-KIR derivation.
#[derive(Debug)]
pub enum ProductionFunctionalSemanticDerivationErrorV1 {
    ReferenceOwner,
    ReferenceSubjectMismatch,
    KernelSubjectMismatch,
    StructuralReportMismatch,
    ParallelContract(fe2o3_pliron::ProductionParallelReferenceContractErrorV1),
    ParallelReportMismatch,
    IncompleteOutputProduct,
    MissingOrDuplicateEffect { output: usize },
    RankedWriteMismatch { output: usize },
    ReferenceOutputSiteMismatch { output: usize },
    UnsupportedReferenceExpression { output: usize },
    RankedReferenceExpressionMismatch { output: usize },
    NumericalPolicyMismatch { output: usize },
    ResourceLimit,
}

/// Derives the complete functional output product from the policy-checked
/// ranked owner produced by rustc's authenticated reference-effect import.
///
/// The ranked owner has no public constructor and retains the imported signed
/// proof for every exact reference expression. This function accepts no
/// expression, output site, subject, digest, or semantic plan from its caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_production_functional_semantics_from_effect_ir_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    contract: &fe2o3_functional_proof::MirPlironSemanticContractV1,
    structural_report: ProductionMirPlironSemanticContractReportV1,
    parallel: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
) -> Result<ProductionIrDerivedFunctionalSemanticsV1, ProductionFunctionalSemanticDerivationErrorV1>
{
    let retained_effects = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::RequireEffectRefinement { proof, .. } => Some(proof),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some(first_effect) = retained_effects.first() else {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::ReferenceOwner);
    };
    let subjects = first_effect.binding().subjects();
    if retained_effects.len() != contract.outputs().len()
        || retained_effects
            .iter()
            .any(|proof| proof.binding().subjects() != subjects)
    {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct);
    }
    if subjects.safe_reference_mir_hash() != contract.safe_reference_mir() {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::ReferenceSubjectMismatch);
    }
    let kernel_identity = DigestV1::from_untrusted_bytes(*evidence.source_semantic_identity());
    if subjects.kernel_mir_hash() != contract.kernel_mir()
        || kernel_identity != contract.kernel_mir()
    {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::KernelSubjectMismatch);
    }
    let total_output = fe2o3_pliron::require_total_output_staging_v2(ranked, evidence)
        .map_err(|_| ProductionFunctionalSemanticDerivationErrorV1::StructuralReportMismatch)?;
    let recomputed_structural = fe2o3_pliron::require_mir_pliron_semantic_contract_v1(
        ranked,
        evidence,
        total_output,
        contract,
    )
    .map_err(|_| ProductionFunctionalSemanticDerivationErrorV1::StructuralReportMismatch)?;
    if recomputed_structural != structural_report {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::StructuralReportMismatch);
    }
    let recomputed_parallel = require_parallel_reference_contract_v1(
        ranked,
        evidence,
        structural_report,
        contract,
        parallel,
    )
    .map_err(ProductionFunctionalSemanticDerivationErrorV1::ParallelContract)?;
    if recomputed_parallel != parallel_report {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::ParallelReportMismatch);
    }
    require_complete_output_roster_v1(
        contract.outputs().iter().map(|output| output.identity()),
        parallel
            .relations()
            .iter()
            .map(|relation| relation.output_contract()),
    )?;
    let mut product = Sha256::new();
    product.update((OUTPUT_EXPRESSION_PRODUCT_DOMAIN_V1.len() as u64).to_le_bytes());
    product.update(OUTPUT_EXPRESSION_PRODUCT_DOMAIN_V1);
    product.update(subjects.safe_reference_mir_hash().as_bytes());
    product.update(subjects.kernel_mir_hash().as_bytes());
    product.update((contract.outputs().len() as u64).to_le_bytes());
    let mut seen_effects = BTreeSet::new();

    for (output_index, (output, relation)) in contract
        .outputs()
        .iter()
        .zip(parallel.relations())
        .enumerate()
    {
        let matches = ranked
            .kernel()
            .blocks()
            .iter()
            .enumerate()
            .flat_map(|(block, body)| {
                body.operations()
                    .iter()
                    .enumerate()
                    .map(move |(operation, value)| (block, operation, value))
            })
            .filter_map(|(block, operation, value)| match value {
                ProductionRankedOperationV1::RequireEffectRefinement {
                    contract: effect,
                    proof,
                } if production_effect_contract_identity_v1(effect.contract_identity())
                    == output.identity() =>
                {
                    Some((block, operation, effect, proof))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(effect_block, effect_operation, effect, proof)] = matches.as_slice() else {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::MissingOrDuplicateEffect {
                    output: output_index,
                },
            );
        };
        if !seen_effects.insert((*effect_block, *effect_operation)) {
            return Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct);
        }
        validate_ranked_write(ranked.kernel(), effect).map_err(|_| {
            ProductionFunctionalSemanticDerivationErrorV1::RankedWriteMismatch {
                output: output_index,
            }
        })?;
        let (ranked_reference, ranked_numerical) =
            ranked_expression(ranked.kernel(), effect.reference_value()).ok_or(
                ProductionFunctionalSemanticDerivationErrorV1::RankedReferenceExpressionMismatch {
                    output: output_index,
                },
            )?;
        if !ranked_numerical.is_supported() || !ranked_numerical.admits_expression(ranked_reference)
        {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::NumericalPolicyMismatch {
                    output: output_index,
                },
            );
        }
        let (ranked_actual, actual_numerical) =
            ranked_expression(ranked.kernel(), effect.gpu_value()).ok_or(
                ProductionFunctionalSemanticDerivationErrorV1::RankedWriteMismatch {
                    output: output_index,
                },
            )?;
        if actual_numerical != ranked_numerical {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::NumericalPolicyMismatch {
                    output: output_index,
                },
            );
        }
        for identity in [
            output.identity(),
            relation.identity(),
            proof.receipt_identity().digest(),
            DigestV1::from_untrusted_bytes(
                ranked_actual.canonical_transcript_sha256(actual_numerical),
            ),
            DigestV1::from_untrusted_bytes(
                ranked_reference.canonical_transcript_sha256(ranked_numerical),
            ),
        ] {
            product.update(identity.as_bytes());
        }
        let site = effect.reference_output_site();
        product.update(effect.contract_identity().to_le_bytes());
        product.update(site.argument().to_le_bytes());
        product.update(site.block().to_le_bytes());
        product.update(site.statement().to_le_bytes());
    }

    let outputs = u64::try_from(contract.outputs().len())
        .map_err(|_| ProductionFunctionalSemanticDerivationErrorV1::ResourceLimit)?;
    Ok(ProductionIrDerivedFunctionalSemanticsV1 {
        safe_reference_mir: subjects.safe_reference_mir_hash(),
        kernel_mir: kernel_identity,
        ranked_kernel: DigestV1::from_untrusted_bytes(*evidence.ranked_kernel_identity()),
        semantic_contract: contract.canonical_sha256(),
        parallel_contract: parallel.canonical_sha256(),
        output_expression_product: DigestV1::from_untrusted_bytes(product.finalize().into()),
        outputs,
    })
}

impl fmt::Display for ProductionFunctionalSemanticDerivationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "production functional-semantic derivation failed: {self:?}"
        )
    }
}

impl Error for ProductionFunctionalSemanticDerivationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ParallelContract(error) => Some(error),
            _ => None,
        }
    }
}

/// Derives the complete functional output roster from authenticated reference
/// MIR and exact ranked KIR.
///
/// This helper is crate-private. Production callers consume the move-only
/// execution entrypoints, which retain the derived result through final-graph
/// custody instead of accepting a semantic plan.
pub(crate) fn derive_production_functional_semantics_from_ir_v1(
    safe_reference: &ProductionSemanticMirOwnerV1,
    structural: &ProductionReconciledMirPlironKernelV1,
    structural_report: ProductionMirPlironSemanticContractReportV1,
    parallel: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
) -> Result<ProductionIrDerivedFunctionalSemanticsV1, ProductionFunctionalSemanticDerivationErrorV1>
{
    safe_reference
        .verify_equivalence()
        .map_err(|_| ProductionFunctionalSemanticDerivationErrorV1::ReferenceOwner)?;
    let contract = structural.semantic_contract();
    if structural.semantic_contract_report() != structural_report {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::StructuralReportMismatch);
    }
    let reference_identity =
        DigestV1::from_untrusted_bytes(*safe_reference.semantic().semantic_sha256().as_bytes());
    if reference_identity != contract.safe_reference_mir() {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::ReferenceSubjectMismatch);
    }
    let kernel_identity =
        DigestV1::from_untrusted_bytes(*structural.evidence().source_semantic_identity());
    if kernel_identity != contract.kernel_mir() {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::KernelSubjectMismatch);
    }
    let recomputed_parallel = require_parallel_reference_contract_v1(
        structural.ranked(),
        structural.evidence(),
        structural_report,
        contract,
        parallel,
    )
    .map_err(ProductionFunctionalSemanticDerivationErrorV1::ParallelContract)?;
    if recomputed_parallel != parallel_report {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::ParallelReportMismatch);
    }
    require_complete_output_roster_v1(
        contract.outputs().iter().map(|output| output.identity()),
        parallel
            .relations()
            .iter()
            .map(|relation| relation.output_contract()),
    )?;

    let selection = safe_reference
        .semantic()
        .select_kernel_body_v1()
        .ok_or(ProductionFunctionalSemanticDerivationErrorV1::ReferenceSubjectMismatch)?;
    let reference_function = safe_reference
        .resolve_function(selection.body())
        .ok_or(ProductionFunctionalSemanticDerivationErrorV1::ReferenceSubjectMismatch)?;
    let mut product = Sha256::new();
    product.update((OUTPUT_EXPRESSION_PRODUCT_DOMAIN_V1.len() as u64).to_le_bytes());
    product.update(OUTPUT_EXPRESSION_PRODUCT_DOMAIN_V1);
    product.update((contract.outputs().len() as u64).to_le_bytes());
    let mut seen_effects = BTreeSet::new();

    for (output_index, (output, relation)) in contract
        .outputs()
        .iter()
        .zip(parallel.relations())
        .enumerate()
    {
        let matches = structural
            .ranked()
            .kernel()
            .blocks()
            .iter()
            .enumerate()
            .flat_map(|(block, body)| {
                body.operations()
                    .iter()
                    .enumerate()
                    .map(move |(operation, value)| (block, operation, value))
            })
            .filter_map(|(block, operation, value)| match value {
                ProductionRankedOperationV1::RequireEffectRefinement { contract, .. }
                    if production_effect_contract_identity_v1(contract.contract_identity())
                        == output.identity() =>
                {
                    Some((block, operation, contract))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(effect_block, effect_operation, effect)] = matches.as_slice() else {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::MissingOrDuplicateEffect {
                    output: output_index,
                },
            );
        };
        if !seen_effects.insert((*effect_block, *effect_operation)) {
            return Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct);
        }
        validate_ranked_write(structural.ranked().kernel(), effect).map_err(|_| {
            ProductionFunctionalSemanticDerivationErrorV1::RankedWriteMismatch {
                output: output_index,
            }
        })?;
        let (reference_expression, numerical) = normalize_reference_output(
            safe_reference,
            reference_function,
            effect,
        )
        .map_err(|kind| match kind {
            ReferenceNormalizationFailureV1::Site => {
                ProductionFunctionalSemanticDerivationErrorV1::ReferenceOutputSiteMismatch {
                    output: output_index,
                }
            }
            ReferenceNormalizationFailureV1::Expression => {
                ProductionFunctionalSemanticDerivationErrorV1::UnsupportedReferenceExpression {
                    output: output_index,
                }
            }
        })?;
        let (ranked_reference, ranked_numerical) =
            ranked_expression(structural.ranked().kernel(), effect.reference_value()).ok_or(
                ProductionFunctionalSemanticDerivationErrorV1::RankedReferenceExpressionMismatch {
                    output: output_index,
                },
            )?;
        if reference_expression != *ranked_reference {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::RankedReferenceExpressionMismatch {
                    output: output_index,
                },
            );
        }
        if numerical != ranked_numerical
            || !ranked_numerical.is_supported()
            || !ranked_numerical.admits_expression(ranked_reference)
        {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::NumericalPolicyMismatch {
                    output: output_index,
                },
            );
        }
        let (ranked_actual, actual_numerical) =
            ranked_expression(structural.ranked().kernel(), effect.gpu_value()).ok_or(
                ProductionFunctionalSemanticDerivationErrorV1::RankedWriteMismatch {
                    output: output_index,
                },
            )?;
        if actual_numerical != ranked_numerical {
            return Err(
                ProductionFunctionalSemanticDerivationErrorV1::NumericalPolicyMismatch {
                    output: output_index,
                },
            );
        }
        for identity in [
            output.identity(),
            relation.identity(),
            DigestV1::from_untrusted_bytes(
                ranked_actual.canonical_transcript_sha256(actual_numerical),
            ),
            DigestV1::from_untrusted_bytes(
                ranked_reference.canonical_transcript_sha256(ranked_numerical),
            ),
        ] {
            product.update(identity.as_bytes());
        }
    }

    let outputs = u64::try_from(contract.outputs().len())
        .map_err(|_| ProductionFunctionalSemanticDerivationErrorV1::ResourceLimit)?;
    Ok(ProductionIrDerivedFunctionalSemanticsV1 {
        safe_reference_mir: reference_identity,
        kernel_mir: kernel_identity,
        ranked_kernel: DigestV1::from_untrusted_bytes(
            *structural.evidence().ranked_kernel_identity(),
        ),
        semantic_contract: contract.canonical_sha256(),
        parallel_contract: parallel.canonical_sha256(),
        output_expression_product: DigestV1::from_untrusted_bytes(product.finalize().into()),
        outputs,
    })
}

fn require_complete_output_roster_v1(
    outputs: impl ExactSizeIterator<Item = DigestV1>,
    relations: impl ExactSizeIterator<Item = DigestV1>,
) -> Result<(), ProductionFunctionalSemanticDerivationErrorV1> {
    let output_count = outputs.len();
    if output_count == 0
        || output_count != relations.len()
        || output_count > fe2o3_functional_proof::HARD_MAX_AGGREGATE_FUNCTIONAL_OUTPUTS_V1
        || !outputs
            .zip(relations)
            .all(|(output, relation)| output == relation)
    {
        return Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct);
    }
    Ok(())
}

fn validate_ranked_write(
    kernel: &ProductionRankedKernelV1,
    effect: &ProductionEffectRefinementContractV2,
) -> Result<(), ()> {
    let site = effect.gpu_write_site();
    match kernel
        .blocks()
        .get(site.block() as usize)
        .and_then(|block| block.operations().get(site.operation() as usize))
    {
        Some(ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view,
            indices,
            value,
        }) if *view == effect.view()
            && indices.as_slice() == effect.indices()
            && *value == effect.gpu_value() =>
        {
            Ok(())
        }
        _ => Err(()),
    }
}

fn ranked_expression(
    kernel: &ProductionRankedKernelV1,
    root: ProductionRankedValueV1,
) -> Option<(
    &ProductionSemanticExpressionV2,
    ProductionNumericalContractV2,
)> {
    let ProductionRankedValueV1::Local(root) = root else {
        return None;
    };
    let matches = kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } if *result == root => Some((expression, *numerical_contract)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        return None;
    };
    Some(*expression)
}

#[derive(Clone, Copy)]
enum ReferenceNormalizationFailureV1 {
    Site,
    Expression,
}

fn normalize_reference_output(
    owner: &ProductionSemanticMirOwnerV1,
    function: &SemanticFunctionDeclV1,
    effect: &ProductionEffectRefinementContractV2,
) -> Result<
    (
        ProductionSemanticExpressionV2,
        ProductionNumericalContractV2,
    ),
    ReferenceNormalizationFailureV1,
> {
    let site = effect.reference_output_site();
    let block = function
        .blocks()
        .get(site.block() as usize)
        .ok_or(ReferenceNormalizationFailureV1::Site)?;
    let statement = block
        .statements()
        .get(site.statement() as usize)
        .ok_or(ReferenceNormalizationFailureV1::Site)?;
    let (destination, expression) = match statement.kind() {
        SemanticStatementKindV1::Store(store) if store.atomic().is_none() => (
            store.destination(),
            normalize_operand(
                owner,
                function,
                site.block() as usize,
                site.statement() as usize,
                store.value(),
                0,
                &mut BTreeSet::new(),
            )?,
        ),
        SemanticStatementKindV1::Assign(assignment)
            if !assignment.destination().projections().is_empty() =>
        {
            (
                assignment.destination(),
                normalize_rvalue(
                    owner,
                    function,
                    site.block() as usize,
                    site.statement() as usize,
                    assignment.value().kind(),
                    assignment.value().result_type(),
                    0,
                    &mut BTreeSet::new(),
                )?,
            )
        }
        _ => return Err(ReferenceNormalizationFailureV1::Site),
    };
    let local = function
        .locals()
        .get(destination.local().index() as usize)
        .ok_or(ReferenceNormalizationFailureV1::Site)?;
    if local.role() != SemanticLocalRoleV1::Argument(site.argument()) {
        return Err(ReferenceNormalizationFailureV1::Site);
    }
    expression
        .validate()
        .map_err(|_| ReferenceNormalizationFailureV1::Expression)?;
    expression
        .validate_static_domains()
        .map_err(|_| ReferenceNormalizationFailureV1::Expression)?;
    let numerical = ProductionNumericalContractV2::exact_for_expression(&expression);
    Ok((expression, numerical))
}

#[allow(clippy::too_many_arguments)]
fn normalize_operand(
    owner: &ProductionSemanticMirOwnerV1,
    function: &SemanticFunctionDeclV1,
    block: usize,
    before_statement: usize,
    operand: &SemanticOperandV1,
    depth: usize,
    visiting: &mut BTreeSet<u32>,
) -> Result<ProductionSemanticExpressionV2, ReferenceNormalizationFailureV1> {
    require_depth(depth)?;
    match operand {
        SemanticOperandV1::Constant(constant) => {
            let scalar = semantic_scalar(owner, constant.ty())?;
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err(ReferenceNormalizationFailureV1::Expression);
            };
            let bits = u64::try_from(value.bits())
                .map_err(|_| ReferenceNormalizationFailureV1::Expression)?;
            Ok(ProductionSemanticExpressionV2::Constant { scalar, bits })
        }
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            if !place.projections().is_empty() {
                return Err(ReferenceNormalizationFailureV1::Expression);
            }
            let local_id = place.local();
            let local = function
                .locals()
                .get(local_id.index() as usize)
                .ok_or(ReferenceNormalizationFailureV1::Expression)?;
            if let SemanticLocalRoleV1::Argument(argument) = local.role() {
                let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                    .checked_add(argument)
                    .ok_or(ReferenceNormalizationFailureV1::Expression)?;
                return Ok(ProductionSemanticExpressionV2::Symbol {
                    symbol,
                    scalar: semantic_scalar(owner, local.ty())?,
                });
            }
            if !visiting.insert(local_id.index()) {
                return Err(ReferenceNormalizationFailureV1::Expression);
            }
            let source_block = function
                .blocks()
                .get(block)
                .ok_or(ReferenceNormalizationFailureV1::Expression)?;
            let definitions = source_block
                .statements()
                .iter()
                .take(before_statement)
                .enumerate()
                .filter_map(|(statement, source)| match source.kind() {
                    SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().projections().is_empty()
                            && assignment.destination().local() == local_id =>
                    {
                        Some((statement, assignment.value()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [(statement, value)] = definitions.as_slice() else {
                visiting.remove(&local_id.index());
                return Err(ReferenceNormalizationFailureV1::Expression);
            };
            let result = normalize_rvalue(
                owner,
                function,
                block,
                *statement,
                value.kind(),
                value.result_type(),
                depth + 1,
                visiting,
            );
            visiting.remove(&local_id.index());
            result
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn normalize_rvalue(
    owner: &ProductionSemanticMirOwnerV1,
    function: &SemanticFunctionDeclV1,
    block: usize,
    before_statement: usize,
    value: &SemanticRvalueKindV1,
    result_type: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
    depth: usize,
    visiting: &mut BTreeSet<u32>,
) -> Result<ProductionSemanticExpressionV2, ReferenceNormalizationFailureV1> {
    require_depth(depth)?;
    let scalar = semantic_scalar(owner, result_type)?;
    let recurse = |operand: &SemanticOperandV1, visiting: &mut BTreeSet<u32>| {
        normalize_operand(
            owner,
            function,
            block,
            before_statement,
            operand,
            depth + 1,
            visiting,
        )
    };
    Ok(match value {
        SemanticRvalueKindV1::Use(operand) => recurse(operand, visiting)?,
        SemanticRvalueKindV1::Unary { operation, operand } => {
            ProductionSemanticExpressionV2::Unary {
                operation: match operation {
                    SemanticUnaryOpV1::Not => ProductionSemanticUnaryOpV2::Not,
                    SemanticUnaryOpV1::Negate => ProductionSemanticUnaryOpV2::Negate,
                    SemanticUnaryOpV1::PointerMetadata => {
                        return Err(ReferenceNormalizationFailureV1::Expression);
                    }
                },
                scalar,
                operand: Box::new(recurse(operand, visiting)?),
            }
        }
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } => {
            let (operation, comparison) = semantic_binary(*operation)?;
            let lhs = Box::new(recurse(left, visiting)?);
            let rhs = Box::new(recurse(right, visiting)?);
            if let Some(comparison) = comparison {
                ProductionSemanticExpressionV2::Compare {
                    operation: comparison,
                    operand_scalar: lhs.scalar(),
                    lhs,
                    rhs,
                }
            } else {
                ProductionSemanticExpressionV2::Binary {
                    operation: operation.ok_or(ReferenceNormalizationFailureV1::Expression)?,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs,
                    rhs,
                }
            }
        }
        SemanticRvalueKindV1::Cast { kind, operand } => {
            let operand = recurse(operand, visiting)?;
            let source = operand.scalar();
            let kind = match (
                kind,
                source.is_integer(),
                scalar.is_integer(),
                scalar.is_float(),
            ) {
                (SemanticCastKindV1::Integer, true, true, false) => {
                    ProductionSemanticCastV2::Integer
                }
                (SemanticCastKindV1::Float, true, false, true) => {
                    ProductionSemanticCastV2::IntegerToFloat
                }
                (SemanticCastKindV1::Float, false, false, true) => {
                    ProductionSemanticCastV2::FloatToFloat
                }
                (SemanticCastKindV1::Float, false, true, false) => {
                    ProductionSemanticCastV2::FloatToIntegerSaturating
                }
                _ => return Err(ReferenceNormalizationFailureV1::Expression),
            };
            ProductionSemanticExpressionV2::Cast {
                kind,
                source,
                target: scalar,
                operand: Box::new(operand),
            }
        }
        SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Borrow { .. }
        | SemanticRvalueKindV1::AddressOf { .. }
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_)
        | SemanticRvalueKindV1::Aggregate(_)
        | SemanticRvalueKindV1::Load(_) => {
            return Err(ReferenceNormalizationFailureV1::Expression);
        }
    })
}

fn semantic_scalar(
    owner: &ProductionSemanticMirOwnerV1,
    ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
) -> Result<ProductionSemanticScalarTypeV2, ReferenceNormalizationFailureV1> {
    let declaration = owner
        .semantic()
        .types()
        .get(ty.index() as usize)
        .ok_or(ReferenceNormalizationFailureV1::Expression)?;
    match declaration.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => {
            Ok(ProductionSemanticScalarTypeV2::Bool)
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits })
            if matches!(bits, 8 | 16 | 32 | 64) =>
        {
            Ok(ProductionSemanticScalarTypeV2::Integer {
                signed: *signed,
                bits: *bits,
            })
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits })
            if matches!(bits, 32 | 64) =>
        {
            Ok(ProductionSemanticScalarTypeV2::Float { bits: *bits })
        }
        _ => Err(ReferenceNormalizationFailureV1::Expression),
    }
}

fn semantic_binary(
    operation: SemanticBinaryOpV1,
) -> Result<
    (
        Option<ProductionSemanticBinaryOpV2>,
        Option<fe2o3_pliron::ProductionSemanticComparisonV2>,
    ),
    ReferenceNormalizationFailureV1,
> {
    use fe2o3_pliron::ProductionSemanticComparisonV2 as Comparison;
    Ok(match operation {
        SemanticBinaryOpV1::Add => (Some(ProductionSemanticBinaryOpV2::Add), None),
        SemanticBinaryOpV1::Subtract => (Some(ProductionSemanticBinaryOpV2::Subtract), None),
        SemanticBinaryOpV1::Multiply => (Some(ProductionSemanticBinaryOpV2::Multiply), None),
        SemanticBinaryOpV1::Divide => (Some(ProductionSemanticBinaryOpV2::Divide), None),
        SemanticBinaryOpV1::Remainder => (Some(ProductionSemanticBinaryOpV2::Remainder), None),
        SemanticBinaryOpV1::BitXor => (Some(ProductionSemanticBinaryOpV2::BitXor), None),
        SemanticBinaryOpV1::BitAnd => (Some(ProductionSemanticBinaryOpV2::BitAnd), None),
        SemanticBinaryOpV1::BitOr => (Some(ProductionSemanticBinaryOpV2::BitOr), None),
        SemanticBinaryOpV1::ShiftLeft => (Some(ProductionSemanticBinaryOpV2::ShiftLeft), None),
        SemanticBinaryOpV1::ShiftRight => (Some(ProductionSemanticBinaryOpV2::ShiftRight), None),
        SemanticBinaryOpV1::Equal => (None, Some(Comparison::Equal)),
        SemanticBinaryOpV1::LessThan => (None, Some(Comparison::LessThan)),
        SemanticBinaryOpV1::LessOrEqual => (None, Some(Comparison::LessOrEqual)),
        SemanticBinaryOpV1::NotEqual => (None, Some(Comparison::NotEqual)),
        SemanticBinaryOpV1::GreaterOrEqual => (None, Some(Comparison::GreaterOrEqual)),
        SemanticBinaryOpV1::GreaterThan => (None, Some(Comparison::GreaterThan)),
        SemanticBinaryOpV1::Offset => return Err(ReferenceNormalizationFailureV1::Expression),
    })
}

fn require_depth(depth: usize) -> Result<(), ReferenceNormalizationFailureV1> {
    if depth > MAX_REFERENCE_EXPRESSION_DEPTH_V1 {
        Err(ReferenceNormalizationFailureV1::Expression)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_pliron::ProductionRankedValueIdV1;

    use super::*;

    #[test]
    fn complete_output_roster_rejects_omission_and_order_drift() {
        let first = DigestV1::from_untrusted_bytes([1; 32]);
        let second = DigestV1::from_untrusted_bytes([2; 32]);

        assert!(
            require_complete_output_roster_v1(
                [first, second].into_iter(),
                [first, second].into_iter(),
            )
            .is_ok()
        );
        assert!(matches!(
            require_complete_output_roster_v1([first, second].into_iter(), [first].into_iter(),),
            Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct)
        ));
        assert!(matches!(
            require_complete_output_roster_v1(
                [first, second].into_iter(),
                [second, first].into_iter(),
            ),
            Err(ProductionFunctionalSemanticDerivationErrorV1::IncompleteOutputProduct)
        ));
    }

    #[test]
    fn ranked_write_validation_rejects_a_matching_contract_with_a_swapped_rhs() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 64,
        };
        let expression = |result, operation| ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(result),
            expression: ProductionSemanticExpressionV2::Binary {
                operation,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                    symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                    scalar,
                }),
                rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 1 }),
            },
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        };
        let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let actual = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
        let swapped = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(2));
        let index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(3));
        let operations = vec![
            ProductionRankedOperationV1::View {
                result: ProductionRankedValueIdV1::new(0),
                element_width: 64,
                writable: true,
                shape: vec![1],
                dynamic_extents: vec![],
                allocation_origin: 1,
                noalias_class: 1,
            },
            expression(1, ProductionSemanticBinaryOpV2::Subtract),
            expression(2, ProductionSemanticBinaryOpV2::Add),
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(3),
                value: 0,
            },
            ProductionRankedOperationV1::ValueAccess {
                kind: AccessKindAttr::Write,
                view,
                indices: vec![index],
                value: swapped,
            },
        ];
        let kernel = ProductionRankedKernelV1::new(
            "hostile_rhs",
            1,
            vec![fe2o3_pliron::ProductionRankedBlockV1::new(
                operations,
                fe2o3_pliron::ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let effect = ProductionEffectRefinementContractV2::new(
            7,
            fe2o3_pliron::ProductionGpuWriteSiteV2::new(0, 4),
            fe2o3_pliron::ProductionReferenceOutputSiteV2::new(0, 0, 0),
            view,
            vec![index],
            vec![index],
            vec![index],
            index,
            index,
            index,
            index,
            actual,
            actual,
        )
        .unwrap();
        assert_eq!(validate_ranked_write(&kernel, &effect), Err(()));
    }
}
