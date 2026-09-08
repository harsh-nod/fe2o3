//! Move-only binding of a simulation Bundle V8 to one native V5 compiler transaction.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

const TRANSACTION_IDENTITY_DOMAIN_V8: &[u8] = b"FE2O3/PRODUCTION-BUNDLE-TRANSACTION/V8\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ProductionBundleTransactionIdentityV8([u8; 32]);

impl ProductionBundleTransactionIdentityV8 {
    pub(super) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) enum ProductionBundleTransactionErrorV8 {
    Bundle(fe2o3_kernel_ir::SimulationBundleErrorV8),
    InertBundle(fe2o3_compiler_ffi::InertSimulationBundleErrorV8),
    CanonicalKirMismatch,
    FinalGraphMismatch,
    SourceLineageMismatch,
    SemanticMirMismatch,
    TargetMismatch,
    AttemptMismatch,
    CapabilityHandoffIdentityMismatch,
    CapabilityHandoffLengthMismatch,
    TransactionIdentityMismatch,
    CompilerExecutionSubjectMismatch,
}

impl fmt::Display for ProductionBundleTransactionErrorV8 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundle(error) => write!(formatter, "Bundle V8 is not canonical: {error}"),
            Self::InertBundle(error) => {
                write!(formatter, "Bundle V8 cannot enter the durable transaction: {error}")
            }
            Self::CanonicalKirMismatch => formatter.write_str(
                "Bundle V8 and the native V5 handoff do not retain the same canonical KIR V13",
            ),
            Self::FinalGraphMismatch => formatter.write_str(
                "Bundle V8 and the native V5 handoff do not name the same final graph and epoch",
            ),
            Self::SourceLineageMismatch => formatter.write_str(
                "Bundle V8 and the native V5 handoff do not retain the same source receipts",
            ),
            Self::SemanticMirMismatch => formatter.write_str(
                "Bundle V8 and the native V5 handoff do not retain the same semantic MIR",
            ),
            Self::TargetMismatch => formatter.write_str(
                "Bundle V8 and the native V5 handoff do not name the same exact target",
            ),
            Self::AttemptMismatch => formatter.write_str(
                "Bundle V8 was presented to a different protected build attempt",
            ),
            Self::CapabilityHandoffIdentityMismatch => formatter.write_str(
                "Bundle V8 was presented with a different native V5 handoff identity",
            ),
            Self::CapabilityHandoffLengthMismatch => formatter.write_str(
                "Bundle V8 was presented with a different native V5 handoff length",
            ),
            Self::TransactionIdentityMismatch => formatter.write_str(
                "Bundle V8 was presented with a different native V5 transaction identity",
            ),
            Self::CompilerExecutionSubjectMismatch => formatter.write_str(
                "Bundle V8 and the protected compiler-execution subject do not name the same transaction",
            ),
        }
    }
}

impl Error for ProductionBundleTransactionErrorV8 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bundle(error) => Some(error),
            Self::InertBundle(error) => Some(error),
            _ => None,
        }
    }
}

impl From<fe2o3_kernel_ir::SimulationBundleErrorV8> for ProductionBundleTransactionErrorV8 {
    fn from(error: fe2o3_kernel_ir::SimulationBundleErrorV8) -> Self {
        Self::Bundle(error)
    }
}

impl From<fe2o3_compiler_ffi::InertSimulationBundleErrorV8> for ProductionBundleTransactionErrorV8 {
    fn from(error: fe2o3_compiler_ffi::InertSimulationBundleErrorV8) -> Self {
        Self::InertBundle(error)
    }
}

/// Exact Bundle V8 produced while the protected build attempt is still live.
pub(super) struct PreparedProductionBundleTransactionV8 {
    expected_attempt: fe2o3_artifact_transaction::BuildAttempt,
    bundle: fe2o3_kernel_ir::VerifiedSimulationBundleV8,
}

impl PreparedProductionBundleTransactionV8 {
    pub(super) fn try_new(
        expected_attempt: fe2o3_artifact_transaction::BuildAttempt,
        bundle: fe2o3_kernel_ir::VerifiedSimulationBundleV8,
    ) -> Result<Self, ProductionBundleTransactionErrorV8> {
        bundle.revalidate()?;
        Ok(Self {
            expected_attempt,
            bundle,
        })
    }

    /// Binds the bundle only after W4 has entered the exact native V5 handoff.
    pub(super) fn bind_capability_handoff(
        self,
        handoff: &fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5,
    ) -> Result<CapabilityBoundProductionBundleTransactionV8, ProductionBundleTransactionErrorV8>
    {
        self.bundle.revalidate()?;
        let legacy = handoff.legacy_handoff();
        let capsule = legacy.capsule();
        let receipts = capsule.receipts();
        let source = self.bundle.source_lineage();
        let inventory = receipts.rustc_identity_inventory().identity();
        let preflight = receipts.rustc_preflight_plan().identity();
        if source.rustc_identity_inventory_receipt_sha256() != *inventory.sha256()
            || source.rustc_identity_inventory_receipt_bytes() != inventory.byte_len()
            || source.rustc_preflight_plan_receipt_sha256() != *preflight.sha256()
            || source.rustc_preflight_plan_receipt_bytes() != preflight.byte_len()
        {
            return Err(ProductionBundleTransactionErrorV8::SourceLineageMismatch);
        }
        if self.bundle.semantic_mir() != receipts.semantic_mir().canonical_preimage() {
            return Err(ProductionBundleTransactionErrorV8::SemanticMirMismatch);
        }
        if self.bundle.target() != legacy.module_handoff().target().to_string() {
            return Err(ProductionBundleTransactionErrorV8::TargetMismatch);
        }
        if self.bundle.canonical_kir_v13() != handoff.executable_kir().canonical_preimage() {
            return Err(ProductionBundleTransactionErrorV8::CanonicalKirMismatch);
        }
        let production = self.bundle.production_kir_identity();
        let report = handoff.final_graph_report();
        if production.version() != 13
            || production.digest() != *self.bundle.canonical_kir_v13_digest()
            || production.canonical_length() != self.bundle.canonical_kir_v13_length()
            || report.final_graph() != *self.bundle.canonical_kir_v13_digest()
            || report.final_graph_bytes() != self.bundle.canonical_kir_v13_length()
            || report.final_epoch() != self.bundle.final_graph_epoch()
        {
            return Err(ProductionBundleTransactionErrorV8::FinalGraphMismatch);
        }

        let handoff_identity = handoff.identity();
        let legacy_identity = legacy.identity();
        Ok(CapabilityBoundProductionBundleTransactionV8 {
            expected_attempt: self.expected_attempt,
            handoff_identity: handoff_identity.sha256(),
            handoff_length: handoff_identity.byte_len(),
            legacy_handoff_identity: *legacy_identity.sha256(),
            legacy_handoff_length: legacy_identity.byte_len(),
            executable_kir_receipt: handoff.executable_kir().identity().sha256(),
            w4_witness: report.w4_witness().identity().sha256(),
            bundle: self.bundle,
        })
    }
}

/// Bundle custody bound to the V5 handoff that contains the exact W4 witness.
pub(super) struct CapabilityBoundProductionBundleTransactionV8 {
    expected_attempt: fe2o3_artifact_transaction::BuildAttempt,
    handoff_identity: [u8; 32],
    handoff_length: u64,
    legacy_handoff_identity: [u8; 32],
    legacy_handoff_length: u64,
    executable_kir_receipt: [u8; 32],
    w4_witness: [u8; 32],
    bundle: fe2o3_kernel_ir::VerifiedSimulationBundleV8,
}

impl CapabilityBoundProductionBundleTransactionV8 {
    pub(super) fn inert_simulation_bundle(
        &self,
    ) -> Result<fe2o3_compiler_ffi::InertSimulationBundleV8, ProductionBundleTransactionErrorV8>
    {
        self.bundle.revalidate()?;
        Ok(
            fe2o3_compiler_ffi::InertSimulationBundleV8::from_verified_canonical_bytes(
                *self.bundle.identity().as_bytes(),
                self.bundle.canonical_bytes().to_vec(),
            )?,
        )
    }

    pub(super) fn bind_publication(
        self,
        receipt: &fe2o3_artifact_transaction::CompilerCapabilityHandoffReceiptV5,
    ) -> Result<PublicationBoundProductionBundleTransactionV8, ProductionBundleTransactionErrorV8>
    {
        let observed = PublicationCoordinatesV8 {
            attempt: receipt.attempt(),
            handoff_identity: receipt.handoff_identity().sha256(),
            handoff_length: receipt.handoff_identity().byte_len(),
            transaction_identity: *receipt.transaction_identity().as_bytes(),
            published_length: receipt.length() as u64,
        };
        require_publication_coordinates(
            PublicationExpectationV8 {
                attempt: self.expected_attempt,
                handoff_identity: self.handoff_identity,
                handoff_length: self.handoff_length,
            },
            observed,
        )?;
        Ok(PublicationBoundProductionBundleTransactionV8 {
            expected_attempt: self.expected_attempt,
            transaction_identity: observed.transaction_identity,
            handoff_identity: self.handoff_identity,
            legacy_handoff_identity: self.legacy_handoff_identity,
            legacy_handoff_length: self.legacy_handoff_length,
            executable_kir_receipt: self.executable_kir_receipt,
            w4_witness: self.w4_witness,
            bundle: self.bundle,
        })
    }
}

/// Bundle custody bound to the durable native V5 publication coordinates.
pub(super) struct PublicationBoundProductionBundleTransactionV8 {
    expected_attempt: fe2o3_artifact_transaction::BuildAttempt,
    transaction_identity: [u8; 32],
    handoff_identity: [u8; 32],
    legacy_handoff_identity: [u8; 32],
    legacy_handoff_length: u64,
    executable_kir_receipt: [u8; 32],
    w4_witness: [u8; 32],
    bundle: fe2o3_kernel_ir::VerifiedSimulationBundleV8,
}

impl PublicationBoundProductionBundleTransactionV8 {
    pub(super) fn bind_compiler_execution_subject(
        self,
        subject: &fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    ) -> Result<ProductionBoundBundleV8, ProductionBundleTransactionErrorV8> {
        let outer = subject.outer_handoff();
        require_subject_coordinates(
            SubjectExpectationV8 {
                attempt: self.expected_attempt,
                transaction_identity: self.transaction_identity,
                legacy_handoff_identity: self.legacy_handoff_identity,
                legacy_handoff_length: self.legacy_handoff_length,
            },
            SubjectCoordinatesV8 {
                attempt: subject.attempt(),
                transaction_identity: *subject.transaction_identity().as_bytes(),
                legacy_handoff_identity: *outer.sha256(),
                legacy_handoff_length: outer.byte_len(),
            },
        )?;
        if !subject
            .identity()
            .matches_canonical_bytes(subject.canonical_bytes())
        {
            return Err(ProductionBundleTransactionErrorV8::CompilerExecutionSubjectMismatch);
        }
        let identity = derive_transaction_identity(
            self.expected_attempt,
            self.transaction_identity,
            self.handoff_identity,
            self.executable_kir_receipt,
            self.w4_witness,
            self.bundle.identity(),
            self.bundle.subject_identity(),
            subject.identity(),
        );
        Ok(ProductionBoundBundleV8 {
            identity,
            v5_transaction_identity: self.transaction_identity,
            compiler_execution_subject_identity: subject.identity(),
            bundle: self.bundle,
        })
    }
}

/// Authority-free Bundle V8 bound to one published V5/W4 compiler occurrence.
pub(super) struct ProductionBoundBundleV8 {
    identity: ProductionBundleTransactionIdentityV8,
    v5_transaction_identity: [u8; 32],
    compiler_execution_subject_identity:
        fe2o3_artifact_transaction::InertCompilerExecutionSubjectIdentityV1,
    bundle: fe2o3_kernel_ir::VerifiedSimulationBundleV8,
}

impl ProductionBoundBundleV8 {
    pub(super) fn remains_bound_to(
        &self,
        subject: &fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    ) -> bool {
        self.identity.as_bytes() != &[0; 32]
            && self.v5_transaction_identity == *subject.transaction_identity().as_bytes()
            && self.compiler_execution_subject_identity == subject.identity()
            && self.bundle.identity().as_bytes() != &[0; 32]
    }
}

#[derive(Clone, Copy)]
struct PublicationExpectationV8 {
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    handoff_identity: [u8; 32],
    handoff_length: u64,
}

#[derive(Clone, Copy)]
struct PublicationCoordinatesV8 {
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    handoff_identity: [u8; 32],
    handoff_length: u64,
    transaction_identity: [u8; 32],
    published_length: u64,
}

fn require_publication_coordinates(
    expected: PublicationExpectationV8,
    observed: PublicationCoordinatesV8,
) -> Result<(), ProductionBundleTransactionErrorV8> {
    if observed.attempt != expected.attempt {
        return Err(ProductionBundleTransactionErrorV8::AttemptMismatch);
    }
    if observed.handoff_identity != expected.handoff_identity {
        return Err(ProductionBundleTransactionErrorV8::CapabilityHandoffIdentityMismatch);
    }
    if observed.handoff_length != expected.handoff_length
        || observed.published_length != expected.handoff_length
    {
        return Err(ProductionBundleTransactionErrorV8::CapabilityHandoffLengthMismatch);
    }
    if observed.transaction_identity == [0; 32] {
        return Err(ProductionBundleTransactionErrorV8::TransactionIdentityMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SubjectExpectationV8 {
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    transaction_identity: [u8; 32],
    legacy_handoff_identity: [u8; 32],
    legacy_handoff_length: u64,
}

#[derive(Clone, Copy)]
struct SubjectCoordinatesV8 {
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    transaction_identity: [u8; 32],
    legacy_handoff_identity: [u8; 32],
    legacy_handoff_length: u64,
}

fn require_subject_coordinates(
    expected: SubjectExpectationV8,
    observed: SubjectCoordinatesV8,
) -> Result<(), ProductionBundleTransactionErrorV8> {
    if observed.attempt != expected.attempt
        || observed.legacy_handoff_identity != expected.legacy_handoff_identity
        || observed.legacy_handoff_length != expected.legacy_handoff_length
    {
        return Err(ProductionBundleTransactionErrorV8::CompilerExecutionSubjectMismatch);
    }
    if observed.transaction_identity != expected.transaction_identity {
        return Err(ProductionBundleTransactionErrorV8::TransactionIdentityMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_transaction_identity(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    v5_transaction_identity: [u8; 32],
    handoff_identity: [u8; 32],
    executable_kir_receipt: [u8; 32],
    w4_witness: [u8; 32],
    bundle_identity: fe2o3_kernel_ir::SimulationBundleIdentityV8,
    bundle_subject: &[u8; 32],
    compiler_subject: fe2o3_artifact_transaction::InertCompilerExecutionSubjectIdentityV1,
) -> ProductionBundleTransactionIdentityV8 {
    let mut digest = Sha256::new();
    digest.update(TRANSACTION_IDENTITY_DOMAIN_V8);
    digest.update(attempt.generation().to_le_bytes());
    digest.update(attempt.session().as_bytes());
    digest.update(attempt.invocation().as_bytes());
    digest.update(v5_transaction_identity);
    digest.update(handoff_identity);
    digest.update(executable_kir_receipt);
    digest.update(w4_witness);
    digest.update(bundle_identity.as_bytes());
    digest.update(bundle_subject);
    digest.update(compiler_subject.sha256());
    digest.update(compiler_subject.byte_len().to_le_bytes());
    ProductionBundleTransactionIdentityV8(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(
        generation: u64,
        session: u8,
        invocation: u8,
    ) -> fe2o3_artifact_transaction::BuildAttempt {
        let session = format!("{session:02x}").repeat(16);
        let invocation = format!("{invocation:02x}").repeat(32);
        fe2o3_artifact_transaction::BuildAttempt::from_env_value(&format!(
            "{generation}:{session}:{invocation}",
        ))
        .unwrap()
    }

    fn expectation() -> PublicationExpectationV8 {
        PublicationExpectationV8 {
            attempt: attempt(7, 0x11, 0x22),
            handoff_identity: [0x33; 32],
            handoff_length: 4096,
        }
    }

    fn coordinates() -> PublicationCoordinatesV8 {
        let expected = expectation();
        PublicationCoordinatesV8 {
            attempt: expected.attempt,
            handoff_identity: expected.handoff_identity,
            handoff_length: expected.handoff_length,
            transaction_identity: [0x44; 32],
            published_length: expected.handoff_length,
        }
    }

    #[test]
    fn exact_v5_publication_coordinates_are_accepted() {
        assert!(require_publication_coordinates(expectation(), coordinates()).is_ok());
    }

    #[test]
    fn cross_attempt_handoff_and_transaction_substitutions_fail_closed() {
        let expected = expectation();
        let mut observed = coordinates();
        observed.attempt = attempt(8, 0x11, 0x22);
        assert!(matches!(
            require_publication_coordinates(expected, observed),
            Err(ProductionBundleTransactionErrorV8::AttemptMismatch)
        ));

        let mut observed = coordinates();
        observed.handoff_identity[0] ^= 1;
        assert!(matches!(
            require_publication_coordinates(expected, observed),
            Err(ProductionBundleTransactionErrorV8::CapabilityHandoffIdentityMismatch)
        ));

        let mut observed = coordinates();
        observed.published_length += 1;
        assert!(matches!(
            require_publication_coordinates(expected, observed),
            Err(ProductionBundleTransactionErrorV8::CapabilityHandoffLengthMismatch)
        ));

        let mut observed = coordinates();
        observed.transaction_identity = [0; 32];
        assert!(matches!(
            require_publication_coordinates(expected, observed),
            Err(ProductionBundleTransactionErrorV8::TransactionIdentityMismatch)
        ));
    }

    #[test]
    fn compiler_subject_must_retain_the_native_v5_transaction_identity() {
        let expected = SubjectExpectationV8 {
            attempt: attempt(7, 0x11, 0x22),
            transaction_identity: [0x33; 32],
            legacy_handoff_identity: [0x44; 32],
            legacy_handoff_length: 4096,
        };
        let exact = SubjectCoordinatesV8 {
            attempt: expected.attempt,
            transaction_identity: expected.transaction_identity,
            legacy_handoff_identity: expected.legacy_handoff_identity,
            legacy_handoff_length: expected.legacy_handoff_length,
        };
        assert!(require_subject_coordinates(expected, exact).is_ok());

        let mut substituted = exact;
        substituted.transaction_identity[0] ^= 1;
        assert!(matches!(
            require_subject_coordinates(expected, substituted),
            Err(ProductionBundleTransactionErrorV8::TransactionIdentityMismatch)
        ));

        let mut substituted = exact;
        substituted.legacy_handoff_identity[0] ^= 1;
        assert!(matches!(
            require_subject_coordinates(expected, substituted),
            Err(ProductionBundleTransactionErrorV8::CompilerExecutionSubjectMismatch)
        ));
    }
}
