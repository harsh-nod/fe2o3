//! Verifier-owned join from retained `rust_verify`/Z3 execution to a signable V2 receipt.
//!
//! This path is workload-neutral: it derives a bounded Verus program from a validated ranked
//! scalar/effect request and binds the exact source, process result, retained runtime closure, and
//! functional-refinement statement. The public producer accepts no caller-authored Verus source.
//! It does not establish Rust source-to-MIR correspondence; current MIR subjects are supplied by
//! the compiler frontend.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fmt::Write as _,
    time::{Duration, Instant},
};

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportErrorV2, FunctionalRefinementImportExpectationV2,
    FunctionalRefinementImportPolicyV2, FunctionalRefinementReceiptImporterV2,
    FunctionalRefinementResultV2, FunctionalRefinementSubjectsV2,
    ImportedFunctionalRefinementProofV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_pliron::{
    ProductionFunctionalRefinementTrustPolicyV2, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    normalized_effect_refinement_hash_for_kernel_v2,
    normalized_functional_refinement_formula_hash_for_kernel_v2,
};
use fe2o3_proof_contracts::DigestV1;
use rand_core::OsRng;
use sha2::{Digest, Sha256};

use crate::functional_refinement_runtime_v1::{
    FunctionalRefinementRuntimeProcessOutputV1, FunctionalRefinementVerusRuntimeLeaseV1,
};
use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementRuntimeErrorV1};

pub const MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2: u32 = 600;
pub const MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2: usize = 16 * 1024;
pub const MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2: usize = 8_192;
pub const MAX_FUNCTIONAL_REFINEMENT_FORMULA_EDGES_V2: usize = 16_384;
pub const MAX_FUNCTIONAL_REFINEMENT_FORMULA_WORK_V2: usize = 32_768;
pub const MAX_FUNCTIONAL_REFINEMENT_FORMULA_DEPTH_V2: usize = 512;

const VERUS_EXECUTABLE_SHA256: [u8; 32] = [
    0xd1, 0xb6, 0x1f, 0xde, 0xd9, 0x13, 0x28, 0xc7, 0xdd, 0x7b, 0xf4, 0x9d, 0x26, 0x4a, 0xdc, 0x6e,
    0xdb, 0x7c, 0xd3, 0x64, 0xe5, 0xcd, 0xe3, 0x4c, 0x46, 0x2f, 0xd1, 0x4b, 0x5c, 0x9a, 0x6f, 0xcb,
];
const SOLVER_EXECUTABLE_SHA256: [u8; 32] = [
    0xe5, 0x83, 0xc4, 0x18, 0x6a, 0x45, 0xe7, 0x24, 0x11, 0xfa, 0x2c, 0xb2, 0x04, 0x84, 0x01, 0xee,
    0xd0, 0x3f, 0x0f, 0x8e, 0x5f, 0x24, 0x69, 0x46, 0x76, 0xa8, 0xf6, 0x27, 0x1a, 0x50, 0xb7, 0x65,
];
const VERUS_CONFIGURATION_DOMAIN: &[u8] =
    b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-RUST-VERIFY-CONFIG/V2\0";
const SOLVER_CONFIGURATION_DOMAIN: &[u8] = b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-Z3-CONFIG/V2\0";
const EXECUTION_IDENTITY_DOMAIN: &[u8] = b"FE2O3/FUNCTIONAL-REFINEMENT/VERUS-EXECUTION/V2\0";

/// Returns the exact toolchain identity enforced by the retained runtime lease.
pub fn functional_refinement_verus_toolchain_identity_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
) -> Result<VerusToolchainIdentityV2, FunctionalRefinementVerusExecutionErrorV2> {
    runtime
        .revalidate()
        .map_err(FunctionalRefinementVerusExecutionErrorV2::runtime)?;
    VerusToolchainIdentityV2::new(
        DigestV1::from_untrusted_bytes(VERUS_EXECUTABLE_SHA256),
        domain_digest(
            VERUS_CONFIGURATION_DOMAIN,
            b"sealed-generated-source-fd;fixed-env",
        ),
        DigestV1::from_untrusted_bytes(SOLVER_EXECUTABLE_SHA256),
        domain_digest(
            SOLVER_CONFIGURATION_DOMAIN,
            b"rust_verify-managed-z3;fixed-env",
        ),
        DigestV1::from_untrusted_bytes(runtime.identity().as_bytes()),
    )
    .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)
}

/// Executes the exact generated proof source before creating a signable `Proved` statement.
///
/// The signer identity comes from compiler configuration. Kernel source cannot select it. This
/// producer deliberately supports only the reference-MIR to kernel-MIR boundary; a source hash is
/// not evidence of source-to-MIR refinement.
fn execute_functional_refinement_verus_and_prepare_receipt_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    source: CanonicalGeneratedVerusProofInputV3,
    binding: FunctionalRefinementBindingV2,
    signer_identity: DigestV1,
    timeout_seconds: u32,
) -> Result<UnsignedFunctionalRefinementReceiptV2, FunctionalRefinementVerusExecutionErrorV2> {
    if timeout_seconds == 0 || timeout_seconds > MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2
    {
        return Err(FunctionalRefinementVerusExecutionErrorV2::new(
            FunctionalRefinementVerusExecutionErrorKindV2::InvalidTimeout,
        ));
    }
    let toolchain = functional_refinement_verus_toolchain_identity_v2(runtime)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or_else(|| {
            FunctionalRefinementVerusExecutionErrorV2::new(
                FunctionalRefinementVerusExecutionErrorKindV2::InvalidTimeout,
            )
        })?;
    let observed = runtime
        .execute_generated_rust_verify(
            &source,
            deadline,
            MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
        )
        .map_err(FunctionalRefinementVerusExecutionErrorV2::runtime)?;
    validate_proved_output(&observed)?;
    runtime
        .revalidate()
        .map_err(FunctionalRefinementVerusExecutionErrorV2::runtime)?;
    if Instant::now() >= deadline {
        return Err(FunctionalRefinementVerusExecutionErrorV2::new(
            FunctionalRefinementVerusExecutionErrorKindV2::TimedOut,
        ));
    }
    let execution_identity = execution_identity(runtime, &source, binding, &observed);
    UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        signer_identity,
        binding,
        toolchain,
        execution_identity,
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)
}

/// Typed output of the compiler-owned ranked-formula generator and Verus execution.
pub struct PreparedFunctionalRefinementReceiptV2 {
    binding: FunctionalRefinementBindingV2,
    unsigned: UnsignedFunctionalRefinementReceiptV2,
}

impl PreparedFunctionalRefinementReceiptV2 {
    pub const fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.binding
    }
    pub fn into_unsigned(self) -> UnsignedFunctionalRefinementReceiptV2 {
        self.unsigned
    }
}

/// Generates Verus source from the ranked semantic DAG and executes it through
/// the retained runtime. There is no caller-provided source parameter.
pub fn prepare_ranked_functional_refinement_receipt_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    kernel: &ProductionRankedKernelV1,
    block_index: usize,
    operation_index: usize,
    subjects: FunctionalRefinementSubjectsV2,
    signer_identity: DigestV1,
    timeout_seconds: u32,
) -> Result<PreparedFunctionalRefinementReceiptV2, FunctionalRefinementVerusExecutionErrorV2> {
    let (binding, source) = generate_ranked_functional_refinement_proof_v2(
        kernel,
        block_index,
        operation_index,
        subjects,
    )?;
    let unsigned = execute_functional_refinement_verus_and_prepare_receipt_v2(
        runtime,
        source,
        binding,
        signer_identity,
        timeout_seconds,
    )?;
    Ok(PreparedFunctionalRefinementReceiptV2 { binding, unsigned })
}

/// Local compilation path with an ephemeral compiler-owned trust root.
pub fn execute_and_import_ranked_functional_refinement_locally_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    kernel: &ProductionRankedKernelV1,
    block_index: usize,
    operation_index: usize,
    subjects: FunctionalRefinementSubjectsV2,
    timeout_seconds: u32,
) -> Result<
    (
        FunctionalRefinementBindingV2,
        ImportedFunctionalRefinementProofV2,
        ProductionFunctionalRefinementTrustPolicyV2,
    ),
    FunctionalRefinementVerusExecutionErrorV2,
> {
    let signing = SigningKey::generate(&mut OsRng);
    let toolchain = functional_refinement_verus_toolchain_identity_v2(runtime)?;
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)?;
    let production_policy =
        ProductionFunctionalRefinementTrustPolicyV2::new([policy.signer_identity()], toolchain)
            .map_err(|_| invalid_ranked_recipe())?;
    let prepared = prepare_ranked_functional_refinement_receipt_v2(
        runtime,
        kernel,
        block_index,
        operation_index,
        subjects,
        policy.signer_identity(),
        timeout_seconds,
    )?;
    let binding = prepared.binding();
    let unsigned = prepared.into_unsigned();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)?;
    let proof = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)?;
    Ok((binding, proof, production_policy))
}

fn generate_ranked_functional_refinement_proof_v2(
    kernel: &ProductionRankedKernelV1,
    block_index: usize,
    operation_index: usize,
    subjects: FunctionalRefinementSubjectsV2,
) -> Result<
    (
        FunctionalRefinementBindingV2,
        CanonicalGeneratedVerusProofInputV3,
    ),
    FunctionalRefinementVerusExecutionErrorV2,
> {
    let operation = kernel
        .blocks()
        .get(block_index)
        .and_then(|block| block.operations().get(operation_index))
        .ok_or_else(invalid_ranked_recipe)?;
    let (obligation, pairs) = match operation {
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            subjects: request_subjects,
        } if *request_subjects == subjects => (
            normalized_functional_refinement_formula_hash_for_kernel_v2(
                kernel,
                block_index,
                operation_index,
                *actual,
                *expected,
                subjects,
            )
            .map_err(|_| invalid_ranked_recipe())?,
            vec![(*actual, *expected)],
        ),
        ProductionRankedOperationV1::RequestEffectRefinement {
            contract,
            subjects: request_subjects,
        } if *request_subjects == subjects => {
            let obligation = normalized_effect_refinement_hash_for_kernel_v2(
                kernel,
                block_index,
                operation_index,
                contract,
                subjects,
            )
            .map_err(|_| invalid_ranked_recipe())?;
            let mut pairs = contract
                .gpu_coordinates()
                .iter()
                .copied()
                .zip(contract.reference_coordinates().iter().copied())
                .collect::<Vec<_>>();
            pairs.extend([
                (contract.gpu_domain(), contract.reference_domain()),
                (
                    contract.gpu_precondition(),
                    contract.reference_precondition(),
                ),
                (contract.gpu_value(), contract.reference_value()),
            ]);
            (obligation, pairs)
        }
        _ => return Err(invalid_ranked_recipe()),
    };
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation)
        .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)?;
    let program = SemanticFormulaProgramV2::build(kernel, &pairs)?;
    let source = program.render(&pairs)?;
    let source =
        CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(|error| {
            FunctionalRefinementVerusExecutionErrorV2 {
                kind: FunctionalRefinementVerusExecutionErrorKindV2::GeneratedSource,
                detail: Some(error.to_string()),
            }
        })?;
    Ok((binding, source))
}

#[derive(Clone)]
enum SemanticDefinitionV2 {
    Symbol(u32),
    Constant(i128),
    Binary(
        dialect_kernel::SemanticBinaryKindAttr,
        ProductionRankedValueV1,
        ProductionRankedValueV1,
    ),
    TypedExpression(
        fe2o3_pliron::ProductionSemanticExpressionV2,
        fe2o3_pliron::ProductionNumericalContractV2,
    ),
}

impl SemanticDefinitionV2 {
    fn dependencies(&self) -> [Option<ProductionRankedValueV1>; 2] {
        match self {
            Self::Symbol(_) | Self::Constant(_) => [None, None],
            Self::Binary(_, lhs, rhs) => [Some(*lhs), Some(*rhs)],
            Self::TypedExpression(_, _) => [None, None],
        }
    }
}

struct SemanticFormulaProgramV2 {
    definitions: BTreeMap<ProductionRankedValueIdV1, SemanticDefinitionV2>,
    order: Vec<ProductionRankedValueIdV1>,
    symbols: BTreeSet<u32>,
}

impl SemanticFormulaProgramV2 {
    fn build(
        kernel: &ProductionRankedKernelV1,
        pairs: &[(ProductionRankedValueV1, ProductionRankedValueV1)],
    ) -> Result<Self, FunctionalRefinementVerusExecutionErrorV2> {
        let mut definitions = BTreeMap::new();
        let mut retained_expression_nodes = 0_usize;
        let mut retained_expression_edges = 0_usize;
        for operation in kernel.blocks().iter().flat_map(|block| block.operations()) {
            let definition = match operation {
                ProductionRankedOperationV1::SemanticSymbol { result, symbol } => {
                    Some((*result, SemanticDefinitionV2::Symbol(*symbol)))
                }
                ProductionRankedOperationV1::SemanticConstant { result, value } => {
                    Some((*result, SemanticDefinitionV2::Constant(i128::from(*value))))
                }
                ProductionRankedOperationV1::SemanticBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                } => Some((*result, SemanticDefinitionV2::Binary(*kind, *lhs, *rhs))),
                ProductionRankedOperationV1::SemanticExpression {
                    result,
                    expression,
                    numerical_contract,
                } => {
                    let stats = expression.validate().map_err(|_| invalid_ranked_recipe())?;
                    if !numerical_contract.is_supported()
                        || !numerical_contract.admits_expression(expression)
                    {
                        return Err(invalid_ranked_recipe());
                    }
                    expression.validate_static_domains().map_err(|error| {
                        if error
                            == fe2o3_pliron::ProductionSemanticExpressionErrorV2::IncompleteDomain
                        {
                            incomplete_semantic_domain()
                        } else {
                            invalid_ranked_recipe()
                        }
                    })?;
                    retained_expression_nodes = retained_expression_nodes
                        .checked_add(stats.nodes)
                        .ok_or_else(formula_resource_limit)?;
                    retained_expression_edges = retained_expression_edges
                        .checked_add(stats.nodes.saturating_sub(1))
                        .ok_or_else(formula_resource_limit)?;
                    Some((
                        *result,
                        SemanticDefinitionV2::TypedExpression(
                            expression.clone(),
                            *numerical_contract,
                        ),
                    ))
                }
                _ => None,
            };
            if let Some((identity, definition)) = definition {
                if definitions.len() >= MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2 {
                    return Err(formula_resource_limit());
                }
                if definitions.insert(identity, definition).is_some() {
                    return Err(invalid_ranked_recipe());
                }
            }
        }
        if retained_expression_nodes
            .checked_add(definitions.len())
            .is_none_or(|nodes| nodes > MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2)
            || retained_expression_edges > MAX_FUNCTIONAL_REFINEMENT_FORMULA_EDGES_V2
        {
            return Err(formula_resource_limit());
        }

        let mut state = BTreeMap::<ProductionRankedValueIdV1, u8>::new();
        let mut depths = BTreeMap::<ProductionRankedValueIdV1, usize>::new();
        let mut order = Vec::new();
        let mut symbols = BTreeSet::new();
        let mut edge_count = 0_usize;
        let mut work = 0_usize;
        for root in pairs
            .iter()
            .flat_map(|(actual, expected)| [actual, expected])
        {
            let ProductionRankedValueV1::Local(root) = *root else {
                return Err(invalid_ranked_recipe());
            };
            let mut stack = vec![(root, false)];
            while let Some((identity, expanded)) = stack.pop() {
                work = work.checked_add(1).ok_or_else(formula_resource_limit)?;
                if work > MAX_FUNCTIONAL_REFINEMENT_FORMULA_WORK_V2 {
                    return Err(formula_resource_limit());
                }
                if expanded {
                    let definition = definitions
                        .get(&identity)
                        .ok_or_else(invalid_ranked_recipe)?
                        .clone();
                    let mut depth = 1_usize;
                    for dependency in definition.dependencies().into_iter().flatten() {
                        let ProductionRankedValueV1::Local(dependency) = dependency else {
                            return Err(invalid_ranked_recipe());
                        };
                        depth = depth.max(
                            depths
                                .get(&dependency)
                                .copied()
                                .ok_or_else(invalid_ranked_recipe)?
                                .checked_add(1)
                                .ok_or_else(formula_resource_limit)?,
                        );
                    }
                    if depth > MAX_FUNCTIONAL_REFINEMENT_FORMULA_DEPTH_V2 {
                        return Err(formula_resource_limit());
                    }
                    match &definition {
                        SemanticDefinitionV2::Symbol(symbol) => {
                            symbols.insert(*symbol);
                        }
                        SemanticDefinitionV2::TypedExpression(expression, _) => {
                            expression.symbols(&mut symbols);
                        }
                        SemanticDefinitionV2::Constant(_)
                        | SemanticDefinitionV2::Binary(_, _, _) => {}
                    }
                    depths.insert(identity, depth);
                    state.insert(identity, 2);
                    order.push(identity);
                    continue;
                }
                match state.get(&identity).copied() {
                    Some(1) => return Err(invalid_ranked_recipe()),
                    Some(2) => continue,
                    _ => {}
                }
                if state.len() >= MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2 {
                    return Err(formula_resource_limit());
                }
                let definition = definitions
                    .get(&identity)
                    .ok_or_else(invalid_ranked_recipe)?
                    .clone();
                state.insert(identity, 1);
                stack.push((identity, true));
                let dependencies = definition.dependencies();
                let added_edges = dependencies.iter().flatten().count();
                edge_count = edge_count
                    .checked_add(added_edges)
                    .ok_or_else(formula_resource_limit)?;
                if edge_count > MAX_FUNCTIONAL_REFINEMENT_FORMULA_EDGES_V2 {
                    return Err(formula_resource_limit());
                }
                for dependency in dependencies.into_iter().flatten().rev() {
                    let ProductionRankedValueV1::Local(dependency) = dependency else {
                        return Err(invalid_ranked_recipe());
                    };
                    stack.push((dependency, false));
                }
            }
        }
        Ok(Self {
            definitions,
            order,
            symbols,
        })
    }

    fn render(
        &self,
        pairs: &[(ProductionRankedValueV1, ProductionRankedValueV1)],
    ) -> Result<String, FunctionalRefinementVerusExecutionErrorV2> {
        let mut source = BoundedVerusSourceV2::default();
        write!(
            source,
            "use vstd::prelude::*;\n\nverus! {{\n{BITVECTOR_SEMANTICS_V2}\n    uninterp spec fn fe2o3_ieee_operator_congruence_v2(tag: int, a: int, b: int, c: int) -> int;\n\n    proof fn fe2o3_functional_refinement_v2("
        )
        .map_err(|_| generated_source_limit())?;
        for (index, symbol) in self.symbols.iter().enumerate() {
            if index != 0 {
                source
                    .write_str(", ")
                    .map_err(|_| generated_source_limit())?;
            }
            write!(source, "s{symbol}: int").map_err(|_| generated_source_limit())?;
        }
        source
            .write_str(") {\n")
            .map_err(|_| generated_source_limit())?;
        for identity in &self.order {
            let definition = self
                .definitions
                .get(identity)
                .cloned()
                .ok_or_else(invalid_ranked_recipe)?;
            match definition {
                SemanticDefinitionV2::Symbol(symbol) => {
                    writeln!(source, "        let v{}: int = s{symbol};", identity.get())
                }
                SemanticDefinitionV2::Constant(value) => {
                    writeln!(source, "        let v{}: int = {value};", identity.get())
                }
                SemanticDefinitionV2::Binary(kind, lhs, rhs) => {
                    let (ProductionRankedValueV1::Local(lhs), ProductionRankedValueV1::Local(rhs)) =
                        (lhs, rhs)
                    else {
                        return Err(invalid_ranked_recipe());
                    };
                    let operator = match kind {
                        dialect_kernel::SemanticBinaryKindAttr::Add => "+",
                        dialect_kernel::SemanticBinaryKindAttr::Multiply => "*",
                    };
                    writeln!(
                        source,
                        "        let v{}: int = v{} {operator} v{};",
                        identity.get(),
                        lhs.get(),
                        rhs.get()
                    )
                }
                SemanticDefinitionV2::TypedExpression(expression, numerical_contract) => {
                    let rendered = match numerical_contract {
                        fe2o3_pliron::ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => {
                            render_bitvector_expression_v2(&expression)?
                        }
                        fe2o3_pliron::ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. } => {
                            format!(
                                "fe2o3_ieee_operator_congruence_v2({}, {}, 0, 0)",
                                numerical_contract_tag_v2(numerical_contract),
                                render_ieee_congruence_expression_v2(&expression)?,
                            )
                        }
                        fe2o3_pliron::ProductionNumericalContractV2::Relaxed
                        | fe2o3_pliron::ProductionNumericalContractV2::ErrorBounded { .. } => {
                            return Err(invalid_ranked_recipe());
                        }
                    };
                    writeln!(source, "        let v{}: int = {rendered};", identity.get())
                }
            }
            .map_err(|_| generated_source_limit())?;
        }
        for (actual, expected) in pairs {
            let (ProductionRankedValueV1::Local(actual), ProductionRankedValueV1::Local(expected)) =
                (*actual, *expected)
            else {
                return Err(invalid_ranked_recipe());
            };
            writeln!(
                source,
                "        assert(v{} == v{});",
                actual.get(),
                expected.get()
            )
            .map_err(|_| generated_source_limit())?;
        }
        source
            .write_str("    }\n}\n\nfn main() {}\n")
            .map_err(|_| generated_source_limit())?;
        Ok(source.into_string())
    }
}

const BITVECTOR_SEMANTICS_V2: &str = r#"
    pub open spec fn fe2o3_pow2_v2(exponent: nat) -> nat
        decreases exponent,
    {
        if exponent == 0 {
            1
        } else {
            2 * fe2o3_pow2_v2((exponent - 1) as nat)
        }
    }

    pub open spec fn fe2o3_bv_modulus_v2(width: nat) -> int {
        if width == 1 { 2 }
        else if width == 8 { 256 }
        else if width == 16 { 65536 }
        else if width == 32 { 4294967296 }
        else if width == 64 { 18446744073709551616 }
        else { fe2o3_pow2_v2(width) as int }
    }

    pub open spec fn fe2o3_bv_norm_v2(value: int, width: nat) -> int {
        value % fe2o3_bv_modulus_v2(width)
    }

    pub open spec fn fe2o3_bv_signed_v2(value: int, width: nat) -> int
        recommends width > 0,
    {
        let normalized = fe2o3_bv_norm_v2(value, width);
        let sign = fe2o3_bv_modulus_v2(width) / 2;
        if normalized >= sign {
            normalized - fe2o3_bv_modulus_v2(width)
        } else {
            normalized
        }
    }

    pub open spec fn fe2o3_signed_div_v2(lhs: int, rhs: int) -> int
        recommends rhs != 0,
    {
        if lhs < 0 {
            if rhs < 0 { (-lhs) / (-rhs) } else { -((-lhs) / rhs) }
        } else if rhs < 0 {
            -(lhs / (-rhs))
        } else {
            lhs / rhs
        }
    }

    pub open spec fn fe2o3_signed_rem_v2(lhs: int, rhs: int) -> int
        recommends rhs != 0,
    {
        lhs - fe2o3_signed_div_v2(lhs, rhs) * rhs
    }

    pub open spec fn fe2o3_bit_v2(value: int, bit: nat) -> int {
        (value / (fe2o3_pow2_v2(bit) as int)) % 2
    }

    pub open spec fn fe2o3_bitwise_v2(kind: nat, lhs: int, rhs: int, width: nat) -> int
        decreases width,
    {
        if width == 0 {
            0
        } else {
            let bit = (width - 1) as nat;
            let lhs_set = fe2o3_bit_v2(lhs, bit) == 1;
            let rhs_set = fe2o3_bit_v2(rhs, bit) == 1;
            let result_set =
                if kind == 0 { lhs_set != rhs_set }
                else if kind == 1 { lhs_set && rhs_set }
                else { lhs_set || rhs_set };
            fe2o3_bitwise_v2(kind, lhs, rhs, bit)
                + if result_set { fe2o3_pow2_v2(bit) as int } else { 0 }
        }
    }

    pub open spec fn fe2o3_shift_left_v2(value: int, shift: nat, width: nat) -> int {
        if shift >= width {
            0
        } else {
            fe2o3_bv_norm_v2(value * fe2o3_pow2_v2(shift) as int, width)
        }
    }

    pub open spec fn fe2o3_shift_right_v2(
        value: int,
        shift: nat,
        width: nat,
        signed: bool,
    ) -> int {
        if shift >= width {
            0
        } else {
            let normalized = fe2o3_bv_norm_v2(value, width);
            let logical = normalized / (fe2o3_pow2_v2(shift) as int);
            if signed && fe2o3_bv_signed_v2(value, width) < 0 {
                logical + (fe2o3_pow2_v2(width) - fe2o3_pow2_v2((width - shift) as nat)) as int
            } else {
                logical
            }
        }
    }
"#;

fn render_bitvector_expression_v2(
    expression: &fe2o3_pliron::ProductionSemanticExpressionV2,
) -> Result<String, FunctionalRefinementVerusExecutionErrorV2> {
    use fe2o3_pliron::{
        ProductionSemanticBinaryOpV2 as Binary, ProductionSemanticCastV2 as Cast,
        ProductionSemanticComparisonV2 as Comparison, ProductionSemanticExpressionV2 as Expression,
        ProductionSemanticScalarTypeV2 as Scalar, ProductionSemanticUnaryOpV2 as Unary,
    };

    let width = u64::from(expression.scalar().bit_width());
    let rendered = match expression {
        Expression::Symbol { symbol, .. } => {
            format!("fe2o3_bv_norm_v2(s{symbol}, {width})")
        }
        Expression::Constant { bits, .. } => {
            format!("fe2o3_bv_norm_v2({bits}, {width})")
        }
        Expression::Unary {
            operation,
            scalar,
            operand,
        } => {
            let operand = render_bitvector_expression_v2(operand)?;
            match operation {
                Unary::Not if *scalar == Scalar::Bool => {
                    format!("if {operand} == 0 {{ 1 }} else {{ 0 }}")
                }
                Unary::Not => format!("(fe2o3_bv_modulus_v2({width}) - 1) - {operand}"),
                Unary::Negate => format!("fe2o3_bv_norm_v2(-({operand}), {width})"),
            }
        }
        Expression::Binary {
            operation,
            scalar,
            lhs,
            rhs,
            ..
        } => {
            let lhs = render_bitvector_expression_v2(lhs)?;
            let rhs = render_bitvector_expression_v2(rhs)?;
            let signed = matches!(scalar, Scalar::Integer { signed: true, .. });
            match operation {
                Binary::Add => {
                    format!("fe2o3_bv_norm_v2(({lhs}) + ({rhs}), {width})")
                }
                Binary::Subtract => {
                    format!("fe2o3_bv_norm_v2(({lhs}) - ({rhs}), {width})")
                }
                Binary::Multiply => {
                    format!("fe2o3_bv_norm_v2(({lhs}) * ({rhs}), {width})")
                }
                Binary::Divide if signed => format!(
                    "fe2o3_bv_norm_v2(fe2o3_signed_div_v2(fe2o3_bv_signed_v2({lhs}, {width}), fe2o3_bv_signed_v2({rhs}, {width})), {width})"
                ),
                Binary::Divide => format!("({lhs}) / ({rhs})"),
                Binary::Remainder if signed => format!(
                    "fe2o3_bv_norm_v2(fe2o3_signed_rem_v2(fe2o3_bv_signed_v2({lhs}, {width}), fe2o3_bv_signed_v2({rhs}, {width})), {width})"
                ),
                Binary::Remainder => format!("({lhs}) % ({rhs})"),
                Binary::BitXor => {
                    format!("fe2o3_bitwise_v2(0, {lhs}, {rhs}, {width})")
                }
                Binary::BitAnd => {
                    format!("fe2o3_bitwise_v2(1, {lhs}, {rhs}, {width})")
                }
                Binary::BitOr => {
                    format!("fe2o3_bitwise_v2(2, {lhs}, {rhs}, {width})")
                }
                Binary::ShiftLeft => format!(
                    "fe2o3_shift_left_v2({lhs}, fe2o3_bv_norm_v2({rhs}, {width}) as nat, {width})"
                ),
                Binary::ShiftRight => format!(
                    "fe2o3_shift_right_v2({lhs}, fe2o3_bv_norm_v2({rhs}, {width}) as nat, {width}, {signed})"
                ),
            }
        }
        Expression::Compare {
            operation,
            operand_scalar,
            lhs,
            rhs,
        } => {
            let lhs = render_bitvector_expression_v2(lhs)?;
            let rhs = render_bitvector_expression_v2(rhs)?;
            let signed = matches!(operand_scalar, Scalar::Integer { signed: true, .. });
            let operand_width = u64::from(operand_scalar.bit_width());
            let (lhs, rhs) = if signed {
                (
                    format!("fe2o3_bv_signed_v2({lhs}, {operand_width})"),
                    format!("fe2o3_bv_signed_v2({rhs}, {operand_width})"),
                )
            } else {
                (lhs, rhs)
            };
            let operator = match operation {
                Comparison::Equal => "==",
                Comparison::LessThan => "<",
                Comparison::LessOrEqual => "<=",
                Comparison::NotEqual => "!=",
                Comparison::GreaterOrEqual => ">=",
                Comparison::GreaterThan => ">",
            };
            format!("if ({lhs}) {operator} ({rhs}) {{ 1 }} else {{ 0 }}")
        }
        Expression::Select {
            condition,
            when_true,
            when_false,
            ..
        } => format!(
            "if {} != 0 {{ {} }} else {{ {} }}",
            render_bitvector_expression_v2(condition)?,
            render_bitvector_expression_v2(when_true)?,
            render_bitvector_expression_v2(when_false)?,
        ),
        Expression::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            if *kind != Cast::Integer {
                return Err(invalid_ranked_recipe());
            }
            let operand = render_bitvector_expression_v2(operand)?;
            let source_width = u64::from(source.bit_width());
            let target_width = u64::from(target.bit_width());
            if *target == Scalar::Bool {
                format!("if {operand} == 0 {{ 0 }} else {{ 1 }}")
            } else if matches!(source, Scalar::Integer { signed: true, .. })
                && target_width > source_width
            {
                format!(
                    "fe2o3_bv_norm_v2(fe2o3_bv_signed_v2({operand}, {source_width}), {target_width})"
                )
            } else {
                format!("fe2o3_bv_norm_v2({operand}, {target_width})")
            }
        }
    };
    if rendered.len() > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
        return Err(generated_source_limit());
    }
    Ok(rendered)
}

fn render_ieee_congruence_expression_v2(
    expression: &fe2o3_pliron::ProductionSemanticExpressionV2,
) -> Result<String, FunctionalRefinementVerusExecutionErrorV2> {
    use fe2o3_pliron::ProductionSemanticExpressionV2 as Expression;
    let scalar = scalar_tag_v2(expression.scalar());
    let rendered = match expression {
        Expression::Symbol { symbol, .. } => {
            format!(
                "fe2o3_ieee_operator_congruence_v2({}, s{symbol}, 0, 0)",
                semantic_operation_tag_v2(1, 0, scalar, 0)
            )
        }
        Expression::Constant { bits, .. } => {
            format!(
                "fe2o3_ieee_operator_congruence_v2({}, {bits}, 0, 0)",
                semantic_operation_tag_v2(2, 0, scalar, 0)
            )
        }
        Expression::Unary {
            operation, operand, ..
        } => format!(
            "fe2o3_ieee_operator_congruence_v2({}, {}, 0, 0)",
            semantic_operation_tag_v2(3, *operation as u64, scalar, 0),
            render_ieee_congruence_expression_v2(operand)?,
        ),
        Expression::Binary {
            operation,
            overflow,
            lhs,
            rhs,
            ..
        } => format!(
            "fe2o3_ieee_operator_congruence_v2({}, {}, {}, 0)",
            semantic_operation_tag_v2(4, *operation as u64, scalar, *overflow as u64),
            render_ieee_congruence_expression_v2(lhs)?,
            render_ieee_congruence_expression_v2(rhs)?,
        ),
        Expression::Compare {
            operation,
            lhs,
            rhs,
            ..
        } => format!(
            "fe2o3_ieee_operator_congruence_v2({}, {}, {}, 0)",
            semantic_operation_tag_v2(5, *operation as u64, scalar, 0),
            render_ieee_congruence_expression_v2(lhs)?,
            render_ieee_congruence_expression_v2(rhs)?,
        ),
        Expression::Select {
            condition,
            when_true,
            when_false,
            ..
        } => format!(
            "fe2o3_ieee_operator_congruence_v2({}, {}, {}, {})",
            semantic_operation_tag_v2(6, 0, scalar, 0),
            render_ieee_congruence_expression_v2(condition)?,
            render_ieee_congruence_expression_v2(when_true)?,
            render_ieee_congruence_expression_v2(when_false)?,
        ),
        Expression::Cast {
            kind,
            source,
            target,
            operand,
        } => format!(
            "fe2o3_ieee_operator_congruence_v2({}, {}, 0, 0)",
            semantic_operation_tag_v2(
                7,
                *kind as u64,
                scalar_tag_v2(*source),
                scalar_tag_v2(*target),
            ),
            render_ieee_congruence_expression_v2(operand)?,
        ),
    };
    if rendered.len() > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
        return Err(generated_source_limit());
    }
    Ok(rendered)
}

fn semantic_operation_tag_v2(category: u64, operation: u64, scalar: u64, auxiliary: u64) -> u64 {
    debug_assert!(category < 10);
    debug_assert!(operation < 1_000);
    debug_assert!(scalar < 1_000_000);
    debug_assert!(auxiliary < 1_000_000);
    category * 1_000_000_000_000_000
        + operation * 1_000_000_000_000
        + scalar * 1_000_000
        + auxiliary
}

fn scalar_tag_v2(scalar: fe2o3_pliron::ProductionSemanticScalarTypeV2) -> u64 {
    match scalar {
        fe2o3_pliron::ProductionSemanticScalarTypeV2::Bool => 1,
        fe2o3_pliron::ProductionSemanticScalarTypeV2::Integer { signed, bits } => {
            100 + u64::from(signed) * 1_000 + u64::from(bits)
        }
        fe2o3_pliron::ProductionSemanticScalarTypeV2::Float { bits } => 10_000 + u64::from(bits),
    }
}

fn numerical_contract_tag_v2(contract: fe2o3_pliron::ProductionNumericalContractV2) -> u64 {
    match contract {
        fe2o3_pliron::ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => {
            semantic_operation_tag_v2(9, 0, 0, 0)
        }
        fe2o3_pliron::ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding,
            exceptional_values,
        } => semantic_operation_tag_v2(9, 1, rounding as u64, exceptional_values as u64),
        fe2o3_pliron::ProductionNumericalContractV2::Relaxed => {
            semantic_operation_tag_v2(9, 2, 0, 0)
        }
        fe2o3_pliron::ProductionNumericalContractV2::ErrorBounded { .. } => {
            semantic_operation_tag_v2(9, 3, 0, 0)
        }
    }
}

#[derive(Default)]
struct BoundedVerusSourceV2 {
    source: String,
}

impl BoundedVerusSourceV2 {
    fn into_string(self) -> String {
        self.source
    }
}

impl fmt::Write for BoundedVerusSourceV2 {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let length = self
            .source
            .len()
            .checked_add(text.len())
            .ok_or(fmt::Error)?;
        if length > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
            return Err(fmt::Error);
        }
        self.source.push_str(text);
        Ok(())
    }
}

fn formula_resource_limit() -> FunctionalRefinementVerusExecutionErrorV2 {
    FunctionalRefinementVerusExecutionErrorV2 {
        kind: FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe,
        detail: Some(
            "semantic formula DAG exceeds its node, edge, work, or depth bound".to_owned(),
        ),
    }
}

fn generated_source_limit() -> FunctionalRefinementVerusExecutionErrorV2 {
    FunctionalRefinementVerusExecutionErrorV2 {
        kind: FunctionalRefinementVerusExecutionErrorKindV2::GeneratedSource,
        detail: Some("generated Verus source exceeds its byte bound".to_owned()),
    }
}

fn invalid_ranked_recipe() -> FunctionalRefinementVerusExecutionErrorV2 {
    FunctionalRefinementVerusExecutionErrorV2::new(
        FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe,
    )
}

fn incomplete_semantic_domain() -> FunctionalRefinementVerusExecutionErrorV2 {
    FunctionalRefinementVerusExecutionErrorV2 {
        kind: FunctionalRefinementVerusExecutionErrorKindV2::IncompleteSemanticDomain,
        detail: Some(
            "checked overflow, division/remainder, signed negation, or shift definedness needs an authenticated dynamic guard or stronger range proof"
                .to_owned(),
        ),
    }
}

fn validate_proved_output(
    observed: &FunctionalRefinementRuntimeProcessOutputV1,
) -> Result<(), FunctionalRefinementVerusExecutionErrorV2> {
    let prefix = b"verification results:: ";
    let suffix = b" verified, 0 errors\n";
    let count = observed
        .stdout
        .strip_prefix(prefix)
        .and_then(|body| body.strip_suffix(suffix));
    let valid_count = count.is_some_and(|digits| {
        !digits.is_empty()
            && digits.len() <= 10
            && digits.iter().all(u8::is_ascii_digit)
            && std::str::from_utf8(digits)
                .ok()
                .and_then(|digits| digits.parse::<u32>().ok())
                .is_some_and(|count| count != 0)
    });
    if observed.exit_code != Some(0)
        || observed.signal.is_some()
        || !observed.stderr.is_empty()
        || !valid_count
    {
        return Err(FunctionalRefinementVerusExecutionErrorV2::new(
            FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult,
        ));
    }
    Ok(())
}

fn execution_identity(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    source: &CanonicalGeneratedVerusProofInputV3,
    binding: FunctionalRefinementBindingV2,
    observed: &FunctionalRefinementRuntimeProcessOutputV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update(EXECUTION_IDENTITY_DOMAIN);
    put_blob(&mut digest, &runtime.identity().as_bytes());
    put_blob(&mut digest, &source.identity().as_bytes());
    put_blob(&mut digest, source.source());
    for value in [
        binding.safe_reference_identity(),
        binding.safe_reference_source_hash(),
        binding.safe_reference_mir_hash(),
        binding.kernel_subject_identity(),
        binding.kernel_mir_hash(),
        binding.normalized_obligation_effect_ir_hash(),
    ] {
        put_blob(&mut digest, value.as_bytes());
    }
    digest.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
    digest.update(observed.signal.unwrap_or(0).to_le_bytes());
    put_blob(&mut digest, &observed.stdout);
    put_blob(&mut digest, &observed.stderr);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, domain);
    put_blob(&mut digest, bytes);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionalRefinementVerusExecutionErrorKindV2 {
    InvalidTimeout,
    InvalidRankedProofRecipe,
    IncompleteSemanticDomain,
    GeneratedSource,
    Runtime,
    UnexpectedProofResult,
    TimedOut,
    Receipt,
}

#[derive(Debug)]
pub struct FunctionalRefinementVerusExecutionErrorV2 {
    kind: FunctionalRefinementVerusExecutionErrorKindV2,
    detail: Option<String>,
}

impl FunctionalRefinementVerusExecutionErrorV2 {
    pub const fn kind(&self) -> FunctionalRefinementVerusExecutionErrorKindV2 {
        self.kind
    }

    fn new(kind: FunctionalRefinementVerusExecutionErrorKindV2) -> Self {
        Self { kind, detail: None }
    }

    fn runtime(error: FunctionalRefinementRuntimeErrorV1) -> Self {
        Self {
            kind: FunctionalRefinementVerusExecutionErrorKindV2::Runtime,
            detail: Some(error.to_string()),
        }
    }

    fn receipt(error: FunctionalRefinementImportErrorV2) -> Self {
        Self {
            kind: FunctionalRefinementVerusExecutionErrorKindV2::Receipt,
            detail: Some(error.to_string()),
        }
    }
}

impl fmt::Display for FunctionalRefinementVerusExecutionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "functional-refinement Verus execution failed: {:?}",
            self.kind
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl Error for FunctionalRefinementVerusExecutionErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_kernel::SemanticBinaryKindAttr;
    use fe2o3_functional_proof::SafeReferenceKindV2;
    use fe2o3_pliron::{
        ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionRankedBlockV1,
        ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionSemanticBinaryOpV2,
        ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2,
    };

    fn output(
        exit_code: i32,
        stdout: &[u8],
        stderr: &[u8],
    ) -> FunctionalRefinementRuntimeProcessOutputV1 {
        FunctionalRefinementRuntimeProcessOutputV1 {
            exit_code: Some(exit_code),
            signal: None,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    fn digest(value: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([value; 32])
    }

    fn subjects() -> FunctionalRefinementSubjectsV2 {
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(1),
            DigestV1::ZERO,
            digest(2),
            digest(3),
            digest(4),
        )
        .unwrap()
    }

    fn formula_kernel(expected_kind: SemanticBinaryKindAttr) -> ProductionRankedKernelV1 {
        let lhs = ProductionRankedValueIdV1::new(0);
        let rhs = ProductionRankedValueIdV1::new(1);
        let actual = ProductionRankedValueIdV1::new(2);
        let expected = ProductionRankedValueIdV1::new(3);
        let local = ProductionRankedValueV1::Local;
        ProductionRankedKernelV1::new(
            "typed_generator",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticSymbol {
                        result: lhs,
                        symbol: 0,
                    },
                    ProductionRankedOperationV1::SemanticSymbol {
                        result: rhs,
                        symbol: 1,
                    },
                    ProductionRankedOperationV1::SemanticBinary {
                        result: actual,
                        kind: SemanticBinaryKindAttr::Add,
                        lhs: local(lhs),
                        rhs: local(rhs),
                    },
                    ProductionRankedOperationV1::SemanticBinary {
                        result: expected,
                        kind: expected_kind,
                        lhs: local(rhs),
                        rhs: local(lhs),
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: local(actual),
                        expected: local(expected),
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }

    fn shared_formula_kernel(depth: usize) -> (ProductionRankedKernelV1, usize) {
        let local = ProductionRankedValueV1::Local;
        let mut operations = vec![ProductionRankedOperationV1::SemanticSymbol {
            result: ProductionRankedValueIdV1::new(0),
            symbol: 0,
        }];
        for identity in 1..=depth {
            let previous = ProductionRankedValueIdV1::new((identity - 1) as u32);
            operations.push(ProductionRankedOperationV1::SemanticBinary {
                result: ProductionRankedValueIdV1::new(identity as u32),
                kind: SemanticBinaryKindAttr::Add,
                lhs: local(previous),
                rhs: local(previous),
            });
        }
        let result = local(ProductionRankedValueIdV1::new(depth as u32));
        let request = operations.len();
        operations.push(
            ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                actual: result,
                expected: result,
                subjects: subjects(),
            },
        );
        (
            ProductionRankedKernelV1::new(
                "shared_formula",
                0,
                vec![ProductionRankedBlockV1::new(
                    operations,
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap(),
            request,
        )
    }

    fn typed_expression_kernel(
        expected_operation: ProductionSemanticBinaryOpV2,
    ) -> ProductionRankedKernelV1 {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let expression = |operation| ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 7, scalar }),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 9 }),
        };
        let actual = ProductionRankedValueIdV1::new(0);
        let expected = ProductionRankedValueIdV1::new(1);
        let local = ProductionRankedValueV1::Local;
        ProductionRankedKernelV1::new(
            "typed_expression_generator",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: actual,
                        expression: expression(ProductionSemanticBinaryOpV2::Add),
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: expected,
                        expression: expression(expected_operation),
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: local(actual),
                        expected: local(expected),
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }

    fn wrapping_bitvector_kernel(expected_bits: u64) -> ProductionRankedKernelV1 {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 8,
        };
        let constant = |bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
        let actual = ProductionRankedValueIdV1::new(0);
        let expected = ProductionRankedValueIdV1::new(1);
        let local = ProductionRankedValueV1::Local;
        ProductionRankedKernelV1::new(
            "wrapping_bitvector_semantics",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: actual,
                        expression: ProductionSemanticExpressionV2::Binary {
                            operation: ProductionSemanticBinaryOpV2::Add,
                            scalar,
                            overflow: ProductionOverflowContractV2::Wrapping,
                            lhs: Box::new(constant(255)),
                            rhs: Box::new(constant(1)),
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: expected,
                        expression: constant(expected_bits),
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: local(actual),
                        expected: local(expected),
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }

    #[test]
    fn only_exact_nonzero_verified_success_is_proved() {
        validate_proved_output(&output(
            0,
            b"verification results:: 12 verified, 0 errors\n",
            b"",
        ))
        .unwrap();
        for hostile in [
            output(1, b"verification results:: 12 verified, 0 errors\n", b""),
            output(0, b"verification results:: 0 verified, 0 errors\n", b""),
            output(0, b"verification results:: 12 verified, 1 errors\n", b""),
            output(0, b"proved\n", b""),
            output(
                0,
                b"verification results:: 12 verified, 0 errors\n",
                b"warning",
            ),
        ] {
            assert_eq!(
                validate_proved_output(&hostile).unwrap_err().kind(),
                FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
            );
        }
    }

    #[test]
    fn typed_generator_derives_source_and_mutation_from_ranked_formula_dag() {
        let positive = formula_kernel(SemanticBinaryKindAttr::Add);
        let (positive_binding, positive_source) =
            generate_ranked_functional_refinement_proof_v2(&positive, 0, 4, subjects()).unwrap();
        let source = std::str::from_utf8(positive_source.source()).unwrap();
        assert!(source.contains("let v2: int = v0 + v1;"));
        assert!(source.contains("let v3: int = v1 + v0;"));
        assert!(source.contains("assert(v2 == v3);"));
        assert_ne!(
            positive_binding.normalized_obligation_effect_ir_hash(),
            digest(20)
        );

        let mutated = formula_kernel(SemanticBinaryKindAttr::Multiply);
        let (mutated_binding, mutated_source) =
            generate_ranked_functional_refinement_proof_v2(&mutated, 0, 4, subjects()).unwrap();
        assert!(
            std::str::from_utf8(mutated_source.source())
                .unwrap()
                .contains("let v3: int = v1 * v0;")
        );
        assert_ne!(positive_source.source(), mutated_source.source());
        assert_ne!(
            positive_binding.normalized_obligation_effect_ir_hash(),
            mutated_binding.normalized_obligation_effect_ir_hash(),
        );
    }

    #[test]
    fn typed_expression_generator_traverses_transcripts_and_binds_mutations() {
        let positive = typed_expression_kernel(ProductionSemanticBinaryOpV2::Add);
        let summary = fe2o3_pliron::typed_semantic_obligation_summary_v2(&positive).unwrap();
        assert!(summary.is_non_vacuous());
        assert_eq!(summary.expression_roots, 2);
        assert_eq!(summary.checked_operations, 0);
        assert_eq!(summary.statically_discharged_domain_roots, 2);
        assert_eq!(summary.exact_bitvector_operator_congruence_roots, 2);
        assert!(!summary.grants_target_ieee_value_authority());
        let (positive_binding, positive_source) =
            generate_ranked_functional_refinement_proof_v2(&positive, 0, 2, subjects()).unwrap();
        let source = std::str::from_utf8(positive_source.source()).unwrap();
        assert!(source.contains("open spec fn fe2o3_bv_norm_v2"));
        assert!(source.contains("uninterp spec fn fe2o3_ieee_operator_congruence_v2"));
        assert!(!source.contains("fe2o3_semantic_op_v2"));
        assert!(source.contains("s7: int"));
        assert!(source.contains("fe2o3_bv_norm_v2"));
        assert!(source.contains("assert(v0 == v1);"));

        let mutated = typed_expression_kernel(ProductionSemanticBinaryOpV2::Subtract);
        let (mutated_binding, mutated_source) =
            generate_ranked_functional_refinement_proof_v2(&mutated, 0, 2, subjects()).unwrap();
        assert_ne!(positive_source.source(), mutated_source.source());
        assert_ne!(
            positive_binding.normalized_obligation_effect_ir_hash(),
            mutated_binding.normalized_obligation_effect_ir_hash(),
        );
    }

    #[test]
    fn bitvector_generator_interprets_wrapping_arithmetic_instead_of_tagging_it() {
        let positive = wrapping_bitvector_kernel(0);
        let (positive_binding, positive_source) =
            generate_ranked_functional_refinement_proof_v2(&positive, 0, 2, subjects()).unwrap();
        let source = std::str::from_utf8(positive_source.source()).unwrap();
        assert!(source.contains("fe2o3_bv_norm_v2"));
        assert!(source.contains(
            "fe2o3_bv_norm_v2((fe2o3_bv_norm_v2(255, 8)) + (fe2o3_bv_norm_v2(1, 8)), 8)"
        ));
        assert!(!source.contains("fe2o3_semantic_op_v2"));

        let hostile = wrapping_bitvector_kernel(1);
        let (hostile_binding, hostile_source) =
            generate_ranked_functional_refinement_proof_v2(&hostile, 0, 2, subjects()).unwrap();
        assert_ne!(positive_source.source(), hostile_source.source());
        assert_ne!(
            positive_binding.normalized_obligation_effect_ir_hash(),
            hostile_binding.normalized_obligation_effect_ir_hash(),
        );
    }

    #[test]
    fn bitvector_renderer_covers_the_closed_integer_and_boolean_operator_set() {
        use fe2o3_pliron::{
            ProductionSemanticCastV2, ProductionSemanticComparisonV2, ProductionSemanticUnaryOpV2,
        };

        let u8_scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 8,
        };
        let i8_scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 8,
        };
        let constant = |scalar, bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
        let binary = |operation, scalar, lhs, rhs| {
            let rhs_scalar = if matches!(
                operation,
                ProductionSemanticBinaryOpV2::ShiftLeft | ProductionSemanticBinaryOpV2::ShiftRight
            ) {
                u8_scalar
            } else {
                scalar
            };
            ProductionSemanticExpressionV2::Binary {
                operation,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(constant(scalar, lhs)),
                rhs: Box::new(constant(rhs_scalar, rhs)),
            }
        };
        for (operation, scalar, lhs, rhs, marker) in [
            (ProductionSemanticBinaryOpV2::Add, u8_scalar, 7, 3, " + "),
            (
                ProductionSemanticBinaryOpV2::Subtract,
                u8_scalar,
                7,
                3,
                " - ",
            ),
            (
                ProductionSemanticBinaryOpV2::Multiply,
                u8_scalar,
                7,
                3,
                " * ",
            ),
            (
                ProductionSemanticBinaryOpV2::Divide,
                i8_scalar,
                249,
                3,
                "fe2o3_signed_div_v2",
            ),
            (
                ProductionSemanticBinaryOpV2::Remainder,
                i8_scalar,
                249,
                3,
                "fe2o3_signed_rem_v2",
            ),
            (
                ProductionSemanticBinaryOpV2::BitXor,
                u8_scalar,
                0xaa,
                0x0f,
                "fe2o3_bitwise_v2(0",
            ),
            (
                ProductionSemanticBinaryOpV2::BitAnd,
                u8_scalar,
                0xaa,
                0x0f,
                "fe2o3_bitwise_v2(1",
            ),
            (
                ProductionSemanticBinaryOpV2::BitOr,
                u8_scalar,
                0xaa,
                0x0f,
                "fe2o3_bitwise_v2(2",
            ),
            (
                ProductionSemanticBinaryOpV2::ShiftLeft,
                u8_scalar,
                3,
                2,
                "fe2o3_shift_left_v2",
            ),
            (
                ProductionSemanticBinaryOpV2::ShiftRight,
                i8_scalar,
                248,
                2,
                "fe2o3_shift_right_v2",
            ),
        ] {
            let expression = binary(operation, scalar, lhs, rhs);
            expression.validate().unwrap();
            expression.validate_static_domains().unwrap();
            let rendered = render_bitvector_expression_v2(&expression).unwrap();
            assert!(rendered.contains(marker), "{operation:?}: {rendered}");
            assert!(!rendered.contains("ieee_operator_congruence"));
        }

        let signed = constant(i8_scalar, 255);
        let unary = ProductionSemanticExpressionV2::Unary {
            operation: ProductionSemanticUnaryOpV2::Negate,
            scalar: i8_scalar,
            operand: Box::new(signed.clone()),
        };
        assert!(
            render_bitvector_expression_v2(&unary)
                .unwrap()
                .contains("fe2o3_bv_norm_v2(-")
        );
        let comparison = ProductionSemanticExpressionV2::Compare {
            operation: ProductionSemanticComparisonV2::LessThan,
            operand_scalar: i8_scalar,
            lhs: Box::new(signed.clone()),
            rhs: Box::new(constant(i8_scalar, 1)),
        };
        assert!(
            render_bitvector_expression_v2(&comparison)
                .unwrap()
                .contains("fe2o3_bv_signed_v2")
        );
        let cast = ProductionSemanticExpressionV2::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: i8_scalar,
            target: ProductionSemanticScalarTypeV2::Integer {
                signed: true,
                bits: 32,
            },
            operand: Box::new(signed),
        };
        assert!(
            render_bitvector_expression_v2(&cast)
                .unwrap()
                .contains("fe2o3_bv_signed_v2")
        );
    }

    #[test]
    fn dynamic_checked_overflow_fails_closed_at_ranked_admission() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let expression = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Checked,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 1, scalar }),
            rhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 2, scalar }),
        };
        let error = ProductionRankedKernelV1::new(
            "dynamic_checked_domain",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(0),
                        expression: expression.clone(),
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(1),
                        expression,
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                        expected: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap_err();
        assert_eq!(
            error,
            fe2o3_pliron::ProductionRankedKernelErrorV1::InvalidSemanticExpression(
                fe2o3_pliron::ProductionSemanticExpressionErrorV2::IncompleteDomain,
            ),
        );
    }

    #[test]
    fn shared_formula_dag_renders_once_per_node() {
        let (kernel, request) = shared_formula_kernel(128);
        let (_, source) =
            generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects())
                .unwrap();
        let source = std::str::from_utf8(source.source()).unwrap();
        assert_eq!(source.matches("        let v").count(), 129);
        assert!(source.len() < 16 * 1024);
    }

    #[test]
    fn overdeep_formula_dag_fails_before_source_construction() {
        let (kernel, request) = shared_formula_kernel(MAX_FUNCTIONAL_REFINEMENT_FORMULA_DEPTH_V2);
        let error = generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects())
            .unwrap_err();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
        );
        assert!(error.to_string().contains("depth bound"));
    }

    #[test]
    fn oversized_semantic_inventory_fails_before_source_construction() {
        let local = ProductionRankedValueV1::Local;
        let mut operations = (0..=MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2)
            .map(|identity| ProductionRankedOperationV1::SemanticConstant {
                result: ProductionRankedValueIdV1::new(identity as u32),
                value: identity as u64,
            })
            .collect::<Vec<_>>();
        let request = operations.len();
        operations.push(
            ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                actual: local(ProductionRankedValueIdV1::new(0)),
                expected: local(ProductionRankedValueIdV1::new(0)),
                subjects: subjects(),
            },
        );
        let kernel = ProductionRankedKernelV1::new(
            "oversized_semantic_inventory",
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let error = generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects())
            .unwrap_err();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
        );
        assert!(error.to_string().contains("node"));
    }
}
