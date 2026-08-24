//! Verifier-owned join from retained `rust_verify`/Z3 execution to a signable V2 receipt.
//!
//! This path is workload-neutral: it derives a bounded Verus program from a validated ranked
//! scalar/effect request and binds the exact source, process result, retained runtime closure, and
//! functional-refinement statement. The public producer accepts no caller-authored Verus source.
//! It does not establish Rust source-to-MIR correspondence; current MIR subjects are supplied by
//! the compiler frontend.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
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

use crate::{
    CanonicalGeneratedVerusProofInputV3, GeneralGemmRuntimeClosureErrorV2,
    GeneralGemmRuntimeProcessOutputV2, GeneralGemmVerusRuntimeClosureLeaseV2,
};

pub const MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2: u32 = 600;
pub const MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2: usize = 16 * 1024;

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
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
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
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
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
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
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
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
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
        } if *request_subjects == subjects => (
            normalized_effect_refinement_hash_for_kernel_v2(
                kernel,
                block_index,
                operation_index,
                contract,
                subjects,
            )
            .map_err(|_| invalid_ranked_recipe())?,
            vec![
                (contract.gpu_domain(), contract.reference_domain()),
                (
                    contract.gpu_precondition(),
                    contract.reference_precondition(),
                ),
                (contract.gpu_value(), contract.reference_value()),
            ],
        ),
        _ => return Err(invalid_ranked_recipe()),
    };
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation)
        .map_err(FunctionalRefinementVerusExecutionErrorV2::receipt)?;
    let mut symbols = BTreeSet::new();
    let mut rendered = Vec::new();
    for (actual, expected) in pairs {
        let actual =
            render_ranked_semantic_formula(kernel, actual, &mut symbols, &mut BTreeSet::new())?;
        let expected =
            render_ranked_semantic_formula(kernel, expected, &mut symbols, &mut BTreeSet::new())?;
        rendered.push((actual, expected));
    }
    let parameters = symbols
        .iter()
        .map(|symbol| format!("s{symbol}: int"))
        .collect::<Vec<_>>()
        .join(", ");
    let ensures = rendered
        .iter()
        .map(|(actual, expected)| format!("        ({actual}) == ({expected})"))
        .collect::<Vec<_>>()
        .join(",\n");
    let source = format!(
        "use vstd::prelude::*;\n\nverus! {{\n    proof fn fe2o3_functional_refinement_v2({parameters})\n        ensures\n{ensures}\n    {{\n    }}\n}}\n\nfn main() {{}}\n"
    );
    let source =
        CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(|error| {
            FunctionalRefinementVerusExecutionErrorV2 {
                kind: FunctionalRefinementVerusExecutionErrorKindV2::GeneratedSource,
                detail: Some(error.to_string()),
            }
        })?;
    Ok((binding, source))
}

fn render_ranked_semantic_formula(
    kernel: &ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
    symbols: &mut BTreeSet<u32>,
    active: &mut BTreeSet<ProductionRankedValueIdV1>,
) -> Result<String, FunctionalRefinementVerusExecutionErrorV2> {
    let ProductionRankedValueV1::Local(identity) = value else {
        return Err(invalid_ranked_recipe());
    };
    if !active.insert(identity) {
        return Err(invalid_ranked_recipe());
    }
    let definition = kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .find(|operation| match operation {
            ProductionRankedOperationV1::SemanticSymbol { result, .. }
            | ProductionRankedOperationV1::SemanticConstant { result, .. }
            | ProductionRankedOperationV1::SemanticBinary { result, .. } => *result == identity,
            _ => false,
        })
        .ok_or_else(invalid_ranked_recipe)?;
    let rendered = match definition {
        ProductionRankedOperationV1::SemanticSymbol { symbol, .. } => {
            symbols.insert(*symbol);
            format!("s{symbol}")
        }
        ProductionRankedOperationV1::SemanticConstant { value, .. } => value.to_string(),
        ProductionRankedOperationV1::SemanticBinary { kind, lhs, rhs, .. } => {
            let lhs = render_ranked_semantic_formula(kernel, *lhs, symbols, active)?;
            let rhs = render_ranked_semantic_formula(kernel, *rhs, symbols, active)?;
            let operator = match kind {
                dialect_kernel::SemanticBinaryKindAttr::Add => "+",
                dialect_kernel::SemanticBinaryKindAttr::Multiply => "*",
            };
            format!("({lhs} {operator} {rhs})")
        }
        _ => unreachable!(),
    };
    active.remove(&identity);
    Ok(rendered)
}

fn invalid_ranked_recipe() -> FunctionalRefinementVerusExecutionErrorV2 {
    FunctionalRefinementVerusExecutionErrorV2::new(
        FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe,
    )
}

fn validate_proved_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
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
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    source: &CanonicalGeneratedVerusProofInputV3,
    binding: FunctionalRefinementBindingV2,
    observed: &GeneralGemmRuntimeProcessOutputV2,
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

    fn runtime(error: GeneralGemmRuntimeClosureErrorV2) -> Self {
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
        ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedTerminatorV1,
    };

    fn output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> GeneralGemmRuntimeProcessOutputV2 {
        GeneralGemmRuntimeProcessOutputV2 {
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
        assert!(source.contains("((s0 + s1)) == ((s1 + s0))"));
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
                .contains("((s1 * s0))")
        );
        assert_ne!(positive_source.source(), mutated_source.source());
        assert_ne!(
            positive_binding.normalized_obligation_effect_ir_hash(),
            mutated_binding.normalized_obligation_effect_ir_hash(),
        );
    }
}
