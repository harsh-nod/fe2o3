//! Direct retained Verus execution evidence for the pinned scalar GEMM model.
//!
//! This module closes only the source-model execution edge. It deliberately
//! does not infer Rust-to-model correspondence, floating-point semantics,
//! KIR refinement, emitted-machine refinement, or GPU launch authority.

use std::fmt;
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};

use crate::general_gemm_runtime_closure_v2::{
    GeneralGemmProofSourceV2, GeneralGemmRuntimeClosureErrorKindV2,
    GeneralGemmRuntimeClosureErrorV2, GeneralGemmRuntimeProcessOutputV2,
    GeneralGemmVerusRuntimeClosureLeaseV2,
};
use crate::{Digest, ScalarGemmProofSourceV1};

/// Maximum accepted size of each scalar proof output stream.
pub const MAX_SCALAR_GEMM_VERUS_OUTPUT_BYTES_V2: usize = 64 * 1024;
/// Maximum accepted scalar proof deadline.
pub const MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2: u32 = 300;

const EXPECTED_STDOUT: &[u8] = b"verification results:: 15 verified, 0 errors\n";
const OUTPUT_IDENTITY_DOMAIN: &[u8] = b"fe2o3-scalar-gemm-verus-output-v2\0";
const EXECUTION_IDENTITY_DOMAIN: &[u8] = b"fe2o3-scalar-gemm-verus-execution-v2\0";

/// Proof functions established in the exact retained scalar source model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ScalarGemmVerifiedTheoremV2 {
    ActiveInvocationHasUniqueCoordinates = 1,
    ActiveAccessesAreInBounds = 2,
    ActiveInputReadsAreInitialized = 3,
    DistinctActiveInvocationsHaveDistinctOutputIndices = 4,
    EveryOutputHasUniqueCanonicalInvocation = 5,
    ExactDotHasFixedSequentialRecurrence = 6,
    AbstractDotStartsAtZero = 7,
}

/// Exact theorem inventory represented by this source-level receipt.
pub const SCALAR_GEMM_VERUS_THEOREMS_V2: [ScalarGemmVerifiedTheoremV2; 7] = [
    ScalarGemmVerifiedTheoremV2::ActiveInvocationHasUniqueCoordinates,
    ScalarGemmVerifiedTheoremV2::ActiveAccessesAreInBounds,
    ScalarGemmVerifiedTheoremV2::ActiveInputReadsAreInitialized,
    ScalarGemmVerifiedTheoremV2::DistinctActiveInvocationsHaveDistinctOutputIndices,
    ScalarGemmVerifiedTheoremV2::EveryOutputHasUniqueCanonicalInvocation,
    ScalarGemmVerifiedTheoremV2::ExactDotHasFixedSequentialRecurrence,
    ScalarGemmVerifiedTheoremV2::AbstractDotStartsAtZero,
];

/// Stable category for scalar retained-Verus execution failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmVerusExecutionErrorKindV2 {
    InvalidTimeout,
    PinnedSourceMismatch,
    TimedOut,
    OutputTooLarge,
    UnexpectedProofResult,
    RuntimeClosure,
}

/// Failure to execute or authenticate the exact scalar source-model proof.
#[derive(Debug)]
pub struct ScalarGemmVerusExecutionErrorV2 {
    kind: ScalarGemmVerusExecutionErrorKindV2,
    runtime: Option<GeneralGemmRuntimeClosureErrorV2>,
}

impl ScalarGemmVerusExecutionErrorV2 {
    /// Returns the stable failure category.
    pub const fn kind(&self) -> ScalarGemmVerusExecutionErrorKindV2 {
        self.kind
    }

    fn new(kind: ScalarGemmVerusExecutionErrorKindV2) -> Self {
        Self {
            kind,
            runtime: None,
        }
    }

    fn runtime(error: GeneralGemmRuntimeClosureErrorV2) -> Self {
        let kind = match error.kind() {
            GeneralGemmRuntimeClosureErrorKindV2::TimedOut => {
                ScalarGemmVerusExecutionErrorKindV2::TimedOut
            }
            GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge => {
                ScalarGemmVerusExecutionErrorKindV2::OutputTooLarge
            }
            _ => ScalarGemmVerusExecutionErrorKindV2::RuntimeClosure,
        };
        Self {
            kind,
            runtime: Some(error),
        }
    }
}

impl fmt::Display for ScalarGemmVerusExecutionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scalar GEMM retained Verus execution failed: {:?}",
            self.kind
        )?;
        if let Some(runtime) = &self.runtime {
            write!(formatter, ": {runtime}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ScalarGemmVerusExecutionErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.runtime
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Linear receipt for one exact retained Verus execution of the scalar model.
///
/// This value is intentionally not `Clone`. It authenticates process execution
/// and exact output only; it grants no compiler, artifact, or runtime authority.
#[derive(Debug)]
#[must_use = "source-model proof evidence must be joined to compiler and machine refinement"]
pub struct AuthenticatedScalarGemmVerusProofV2 {
    source: ScalarGemmProofSourceV1,
    runtime_closure_identity: Digest,
    output_identity: Digest,
    identity: Digest,
}

impl AuthenticatedScalarGemmVerusProofV2 {
    /// Returns the pinned scalar proof-source identity.
    pub const fn source(&self) -> ScalarGemmProofSourceV1 {
        self.source
    }

    /// Returns the exact retained Verus closure identity.
    pub const fn runtime_closure_identity(&self) -> Digest {
        self.runtime_closure_identity
    }

    /// Returns the domain-separated identity of exact process status and output.
    pub const fn output_identity(&self) -> Digest {
        self.output_identity
    }

    /// Returns the aggregate retained-execution identity.
    pub const fn identity(&self) -> Digest {
        self.identity
    }

    /// Returns the exact source-level theorem inventory.
    pub const fn verified_source_model_theorems(&self) -> &[ScalarGemmVerifiedTheoremV2; 7] {
        &SCALAR_GEMM_VERUS_THEOREMS_V2
    }

    /// The exact source was accepted by the exact retained Verus closure.
    pub const fn authenticates_retained_verus_execution(&self) -> bool {
        true
    }

    /// The complete reviewed source and tool closure was retained during execution.
    pub const fn has_complete_retained_execution_closure(&self) -> bool {
        true
    }

    /// No Worker V3 challenge is included in this source-model execution.
    pub const fn binds_worker_v3_challenge(&self) -> bool {
        false
    }

    /// The scalar model uses mathematical integers, not Rust or IEEE `f32` semantics.
    pub const fn proves_rust_or_f32_semantics(&self) -> bool {
        false
    }

    /// Rust/MIR/KIR correspondence remains an independently owned obligation.
    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    /// LLVM, ISA, and post-link machine refinement remain open.
    pub const fn proves_emitted_machine_refinement(&self) -> bool {
        false
    }

    /// Source-model execution alone cannot satisfy strict Worker V3 admission.
    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Executes the pinned scalar source with the exact retained Verus closure.
pub fn execute_scalar_gemm_verus_proof_v2(
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    timeout_seconds: u32,
) -> Result<AuthenticatedScalarGemmVerusProofV2, ScalarGemmVerusExecutionErrorV2> {
    if timeout_seconds == 0 || timeout_seconds > MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2 {
        return Err(ScalarGemmVerusExecutionErrorV2::new(
            ScalarGemmVerusExecutionErrorKindV2::InvalidTimeout,
        ));
    }
    let source_bytes = GeneralGemmProofSourceV2::ScalarGemm.embedded_bytes();
    let source = ScalarGemmProofSourceV1::measure(source_bytes).map_err(|_| {
        ScalarGemmVerusExecutionErrorV2::new(
            ScalarGemmVerusExecutionErrorKindV2::PinnedSourceMismatch,
        )
    })?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or_else(|| {
            ScalarGemmVerusExecutionErrorV2::new(
                ScalarGemmVerusExecutionErrorKindV2::InvalidTimeout,
            )
        })?;
    let observed = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::ScalarGemm,
            deadline,
            MAX_SCALAR_GEMM_VERUS_OUTPUT_BYTES_V2,
        )
        .map_err(ScalarGemmVerusExecutionErrorV2::runtime)?;
    validate_exact_output(&observed)?;
    runtime
        .revalidate()
        .map_err(ScalarGemmVerusExecutionErrorV2::runtime)?;
    if Instant::now() >= deadline {
        return Err(ScalarGemmVerusExecutionErrorV2::new(
            ScalarGemmVerusExecutionErrorKindV2::TimedOut,
        ));
    }

    let runtime_closure_identity = Digest::from_bytes(runtime.identity().as_bytes());
    let output_identity = output_identity(&observed);
    let identity = execution_identity(source, runtime_closure_identity, output_identity);
    Ok(AuthenticatedScalarGemmVerusProofV2 {
        source,
        runtime_closure_identity,
        output_identity,
        identity,
    })
}

fn validate_exact_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
) -> Result<(), ScalarGemmVerusExecutionErrorV2> {
    if observed.exit_code != Some(0)
        || observed.signal.is_some()
        || observed.stdout != EXPECTED_STDOUT
        || !observed.stderr.is_empty()
    {
        return Err(ScalarGemmVerusExecutionErrorV2::new(
            ScalarGemmVerusExecutionErrorKindV2::UnexpectedProofResult,
        ));
    }
    Ok(())
}

fn output_identity(observed: &GeneralGemmRuntimeProcessOutputV2) -> Digest {
    let mut digest = Sha256::new();
    digest.update(OUTPUT_IDENTITY_DOMAIN);
    digest.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
    digest.update(observed.signal.unwrap_or(0).to_le_bytes());
    put_blob(&mut digest, &observed.stdout);
    put_blob(&mut digest, &observed.stderr);
    Digest::from_bytes(digest.finalize().into())
}

fn execution_identity(source: ScalarGemmProofSourceV1, runtime: Digest, output: Digest) -> Digest {
    let mut digest = Sha256::new();
    digest.update(EXECUTION_IDENTITY_DOMAIN);
    digest.update(source.identity().as_bytes());
    digest.update(source.content_identity().as_bytes());
    digest.update(source.byte_len().to_le_bytes());
    digest.update(runtime.as_bytes());
    digest.update(output.as_bytes());
    Digest::from_bytes(digest.finalize().into())
}

fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(
        exit_code: Option<i32>,
        signal: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> GeneralGemmRuntimeProcessOutputV2 {
        GeneralGemmRuntimeProcessOutputV2 {
            exit_code,
            signal,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn exact_output_is_required() {
        validate_exact_output(&output(Some(0), None, EXPECTED_STDOUT, b"")).unwrap();
        for substituted in [
            output(Some(1), None, EXPECTED_STDOUT, b""),
            output(None, None, EXPECTED_STDOUT, b""),
            output(Some(0), Some(9), EXPECTED_STDOUT, b""),
            output(
                Some(0),
                None,
                b"verification results:: 14 verified, 0 errors\n",
                b"",
            ),
            output(Some(0), None, EXPECTED_STDOUT, b"warning\n"),
        ] {
            assert_eq!(
                validate_exact_output(&substituted).unwrap_err().kind(),
                ScalarGemmVerusExecutionErrorKindV2::UnexpectedProofResult
            );
        }
    }

    #[test]
    fn output_identity_rejects_every_observation_substitution() {
        let exact = output(Some(0), None, EXPECTED_STDOUT, b"");
        let expected = output_identity(&exact);
        for substituted in [
            output(Some(1), None, EXPECTED_STDOUT, b""),
            output(Some(0), Some(9), EXPECTED_STDOUT, b""),
            output(Some(0), None, b"different", b""),
            output(Some(0), None, EXPECTED_STDOUT, b"different"),
        ] {
            assert_ne!(output_identity(&substituted), expected);
        }
    }

    #[test]
    fn source_model_receipt_remains_non_authoritative() {
        assert_eq!(SCALAR_GEMM_VERUS_THEOREMS_V2.len(), 7);
        let source =
            ScalarGemmProofSourceV1::measure(GeneralGemmProofSourceV2::ScalarGemm.embedded_bytes())
                .unwrap();
        let receipt = AuthenticatedScalarGemmVerusProofV2 {
            source,
            runtime_closure_identity: Digest::from_bytes([1; 32]),
            output_identity: Digest::from_bytes([2; 32]),
            identity: Digest::from_bytes([3; 32]),
        };
        assert!(receipt.authenticates_retained_verus_execution());
        assert!(receipt.has_complete_retained_execution_closure());
        assert!(!receipt.binds_worker_v3_challenge());
        assert!(!receipt.proves_rust_or_f32_semantics());
        assert!(!receipt.proves_compiler_refinement());
        assert!(!receipt.proves_emitted_machine_refinement());
        assert!(!receipt.can_enter_worker_v3_gate());
        assert!(!receipt.grants_artifact_authority());
        assert!(!receipt.grants_load_authority());
        assert!(!receipt.grants_launch_authority());
    }
}
