//! Canonical custody record for one exact aggregate MIR-to-PLIRON Verus execution.
//!
//! The record retains and independently re-imports the exact signed V2 functional-refinement
//! receipt. Its embedded key authenticates internal receipt consistency only; compiler origin is
//! established separately by protected compiler-execution custody. No field grants LLVM, machine,
//! publication, load, or launch authority.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2, FunctionalRefinementBindingV2,
    FunctionalRefinementBoundaryV2, FunctionalRefinementImportErrorV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, ImportedFunctionalRefinementProofV2,
    SafeReferenceKindV2, VerusToolchainIdentityV2,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

const MAGIC_V1: [u8; 8] = *b"F2MPVEV1";
const VERSION_V1: u16 = 1;
const FLAGS_V1: u16 = 0;
const HEADER_BYTES_V1: usize = 20;
const META_BYTES_V1: usize = 4;
const DIGEST_COUNT_V1: usize = 19;
const DIGEST_BYTES_V1: usize = 32;
const RETAINED_COUNT_BYTES_V1: usize = 8;
const VERIFYING_KEY_BYTES_V1: usize = 32;
const IDENTITY_BYTES_V1: usize = 32;
const MIR_TO_LIVE_PLIRON_BOUNDARY_TAG_V1: u8 = 3;
const PROVED_RESULT_TAG_V1: u8 = 1;
const STRICT_IMPORT_ASSURANCE_TAG_V1: u8 = 1;
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-MIR-PLIRON-VERUS-EXECUTION-EVIDENCE/V1\0";

/// Exact fixed width of one canonical aggregate execution record.
pub const PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1: usize = HEADER_BYTES_V1
    + META_BYTES_V1
    + RETAINED_COUNT_BYTES_V1
    + DIGEST_COUNT_V1 * DIGEST_BYTES_V1
    + VERIFYING_KEY_BYTES_V1
    + FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2
    + IDENTITY_BYTES_V1;

/// Authority-free view of one compiler-owned MIR/Pliron proof execution.
///
/// Implementations expose only immutable identities and the exact signed
/// receipt needed for independent import. The verifier does not depend on the
/// live Pliron graph or the compiler-side proof generator.
pub trait ProductionMirPlironVerusExecutionViewV1 {
    fn contract_identity(&self) -> DigestV1;
    fn parallel_contract_identity(&self) -> DigestV1;
    fn pliron_evidence_identity(&self) -> DigestV1;
    fn composition_template_identity(&self) -> DigestV1;
    fn generated_source_identity(&self) -> DigestV1;
    fn obligation_identity(&self) -> DigestV1;
    fn binding(&self) -> FunctionalRefinementBindingV2;
    fn signer_identity(&self) -> DigestV1;
    fn toolchain(&self) -> VerusToolchainIdentityV2;
    fn execution_identity(&self) -> DigestV1;
    fn receipt_identity(&self) -> DigestV1;
    fn retained_policy_checked_staging(&self) -> u64;
    fn receipt_verifying_key(&self) -> &[u8; VERIFYING_KEY_BYTES_V1];
    fn signed_receipt_wire(&self) -> &[u8];
}

/// Exact report claims retained beside the signed aggregate receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMirPlironVerusExecutionClaimsV1 {
    contract_identity: DigestV1,
    parallel_contract_identity: DigestV1,
    pliron_evidence_identity: DigestV1,
    composition_template_identity: DigestV1,
    generated_source_identity: DigestV1,
    binding: FunctionalRefinementBindingV2,
    signer_identity: DigestV1,
    toolchain: VerusToolchainIdentityV2,
    execution_identity: DigestV1,
    receipt_identity: DigestV1,
    retained_policy_checked_staging: u64,
}

impl ProductionMirPlironVerusExecutionClaimsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: DigestV1,
        parallel_contract_identity: DigestV1,
        pliron_evidence_identity: DigestV1,
        composition_template_identity: DigestV1,
        generated_source_identity: DigestV1,
        binding: FunctionalRefinementBindingV2,
        signer_identity: DigestV1,
        toolchain: VerusToolchainIdentityV2,
        execution_identity: DigestV1,
        receipt_identity: DigestV1,
        retained_policy_checked_staging: u64,
    ) -> Result<Self, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
        let claims = Self {
            contract_identity,
            parallel_contract_identity,
            pliron_evidence_identity,
            composition_template_identity,
            generated_source_identity,
            binding,
            signer_identity,
            toolchain,
            execution_identity,
            receipt_identity,
            retained_policy_checked_staging,
        };
        claims.validate()?;
        Ok(claims)
    }

    fn validate(self) -> Result<(), ProductionMirPlironVerusExecutionEvidenceErrorV1> {
        for (field, digest) in [
            ("semantic contract", self.contract_identity),
            ("parallel contract", self.parallel_contract_identity),
            ("live PLIRON evidence", self.pliron_evidence_identity),
            ("composition template", self.composition_template_identity),
            ("generated Verus source", self.generated_source_identity),
            ("receipt signer", self.signer_identity),
            ("Verus execution", self.execution_identity),
            ("signed receipt", self.receipt_identity),
        ] {
            if digest.is_zero() {
                return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ZeroIdentity(field));
            }
        }
        if self.retained_policy_checked_staging == 0 {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ZeroRetainedStaging);
        }
        Ok(())
    }

    const fn ordered_digests(self) -> [DigestV1; DIGEST_COUNT_V1] {
        [
            self.contract_identity,
            self.parallel_contract_identity,
            self.pliron_evidence_identity,
            self.composition_template_identity,
            self.generated_source_identity,
            self.binding.safe_reference_identity(),
            self.binding.safe_reference_source_hash(),
            self.binding.safe_reference_mir_hash(),
            self.binding.kernel_subject_identity(),
            self.binding.kernel_mir_hash(),
            self.binding.normalized_obligation_effect_ir_hash(),
            self.signer_identity,
            self.toolchain.verus_executable(),
            self.toolchain.verus_configuration(),
            self.toolchain.solver_executable(),
            self.toolchain.solver_configuration(),
            self.toolchain.runtime_closure(),
            self.execution_identity,
            self.receipt_identity,
        ]
    }

    pub const fn contract_identity(self) -> DigestV1 {
        self.contract_identity
    }

    pub const fn parallel_contract_identity(self) -> DigestV1 {
        self.parallel_contract_identity
    }

    pub const fn pliron_evidence_identity(self) -> DigestV1 {
        self.pliron_evidence_identity
    }

    pub const fn composition_template_identity(self) -> DigestV1 {
        self.composition_template_identity
    }

    pub const fn generated_source_identity(self) -> DigestV1 {
        self.generated_source_identity
    }

    pub const fn binding(self) -> FunctionalRefinementBindingV2 {
        self.binding
    }

    pub const fn signer_identity(self) -> DigestV1 {
        self.signer_identity
    }

    pub const fn toolchain(self) -> VerusToolchainIdentityV2 {
        self.toolchain
    }

    pub const fn execution_identity(self) -> DigestV1 {
        self.execution_identity
    }

    pub const fn receipt_identity(self) -> DigestV1 {
        self.receipt_identity
    }

    pub const fn retained_policy_checked_staging(self) -> u64 {
        self.retained_policy_checked_staging
    }
}

/// Domain-separated identity of one exact canonical execution-evidence record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMirPlironVerusExecutionEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProductionMirPlironVerusExecutionEvidenceIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only canonical record that independently re-imported its exact signed receipt.
///
/// ```compile_fail
/// use fe2o3_verifier::CanonicalProductionMirPlironVerusExecutionEvidenceV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<CanonicalProductionMirPlironVerusExecutionEvidenceV1>();
/// ```
#[must_use = "dropping this value abandons the independently imported signed Verus receipt"]
pub struct CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
    claims: ProductionMirPlironVerusExecutionClaimsV1,
    verifying_key: [u8; VERIFYING_KEY_BYTES_V1],
    signed_receipt: [u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
    imported: ImportedFunctionalRefinementProofV2,
    identity: ProductionMirPlironVerusExecutionEvidenceIdentityV1,
    canonical_bytes: Box<[u8]>,
}

impl CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
    pub fn new(
        claims: ProductionMirPlironVerusExecutionClaimsV1,
        verifying_key: [u8; VERIFYING_KEY_BYTES_V1],
        signed_receipt: [u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
    ) -> Result<Self, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
        claims.validate()?;
        let imported = import_signed_receipt(claims, verifying_key, &signed_receipt)?;
        let canonical_bytes = encode(claims, verifying_key, &signed_receipt);
        let identity = ProductionMirPlironVerusExecutionEvidenceIdentityV1 {
            sha256: terminal_identity(&canonical_bytes[..canonical_bytes.len() - 32]),
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            claims,
            verifying_key,
            signed_receipt,
            imported,
            identity,
            canonical_bytes,
        })
    }

    pub fn from_execution(
        execution: &impl ProductionMirPlironVerusExecutionViewV1,
    ) -> Result<Self, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
        if execution.obligation_identity()
            != execution.binding().normalized_obligation_effect_ir_hash()
        {
            return Err(
                ProductionMirPlironVerusExecutionEvidenceErrorV1::ReportMismatch(
                    "normalized obligation",
                ),
            );
        }
        let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
            execution.contract_identity(),
            execution.parallel_contract_identity(),
            execution.pliron_evidence_identity(),
            execution.composition_template_identity(),
            execution.generated_source_identity(),
            execution.binding(),
            execution.signer_identity(),
            execution.toolchain(),
            execution.execution_identity(),
            execution.receipt_identity(),
            execution.retained_policy_checked_staging(),
        )?;
        let verifying_key = *execution.receipt_verifying_key();
        let signed_receipt = execution.signed_receipt_wire().try_into().map_err(|_| {
            ProductionMirPlironVerusExecutionEvidenceErrorV1::InvalidLength {
                expected: FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2,
                actual: execution.signed_receipt_wire().len(),
            }
        })?;
        Self::new(claims, verifying_key, signed_receipt)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
        if bytes.len() != PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1 {
            return Err(
                ProductionMirPlironVerusExecutionEvidenceErrorV1::InvalidLength {
                    expected: PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1,
                    actual: bytes.len(),
                },
            );
        }
        if bytes[..8] != MAGIC_V1 {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::InvalidMagic);
        }
        if read_u16(bytes, 8)? != VERSION_V1 {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != FLAGS_V1 {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::UnsupportedFlags);
        }
        if usize::try_from(read_u32(bytes, 12)?).ok() != Some(bytes.len()) {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 16)? != 0 {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::NonzeroReserved);
        }
        let expected_identity = terminal_identity(&bytes[..bytes.len() - IDENTITY_BYTES_V1]);
        if bytes[bytes.len() - IDENTITY_BYTES_V1..] != expected_identity {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::IdentityMismatch);
        }

        let safe_reference_kind = match bytes[20] {
            1 => SafeReferenceKindV2::SourceAndMir,
            2 => SafeReferenceKindV2::Mir,
            value => {
                return Err(
                    ProductionMirPlironVerusExecutionEvidenceErrorV1::InvalidReferenceKind(value),
                );
            }
        };
        if bytes[21] != MIR_TO_LIVE_PLIRON_BOUNDARY_TAG_V1
            || bytes[22] != PROVED_RESULT_TAG_V1
            || bytes[23] != STRICT_IMPORT_ASSURANCE_TAG_V1
        {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::InvalidAssurance);
        }
        let retained_policy_checked_staging = read_u64(bytes, 24)?;
        let mut offset = HEADER_BYTES_V1 + META_BYTES_V1 + RETAINED_COUNT_BYTES_V1;
        let mut digests = [DigestV1::ZERO; DIGEST_COUNT_V1];
        for digest in &mut digests {
            *digest = DigestV1::from_untrusted_bytes(read_fixed::<32>(bytes, offset)?);
            offset += 32;
        }
        let binding = FunctionalRefinementBindingV2::new(
            safe_reference_kind,
            digests[5],
            digests[6],
            digests[7],
            digests[8],
            digests[9],
            digests[10],
        )
        .map_err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport)?;
        let toolchain = VerusToolchainIdentityV2::new(
            digests[12],
            digests[13],
            digests[14],
            digests[15],
            digests[16],
        )
        .map_err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport)?;
        let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
            digests[0],
            digests[1],
            digests[2],
            digests[3],
            digests[4],
            binding,
            digests[11],
            toolchain,
            digests[17],
            digests[18],
            retained_policy_checked_staging,
        )?;
        let verifying_key = read_fixed::<VERIFYING_KEY_BYTES_V1>(bytes, offset)?;
        offset += VERIFYING_KEY_BYTES_V1;
        let signed_receipt =
            read_fixed::<FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2>(bytes, offset)?;
        offset += FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2;
        debug_assert_eq!(offset + IDENTITY_BYTES_V1, bytes.len());
        let decoded = Self::new(claims, verifying_key, signed_receipt)?;
        if decoded.canonical_bytes() != bytes {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    pub const fn claims(&self) -> ProductionMirPlironVerusExecutionClaimsV1 {
        self.claims
    }

    pub const fn verifying_key(&self) -> &[u8; VERIFYING_KEY_BYTES_V1] {
        &self.verifying_key
    }

    pub const fn signed_receipt(&self) -> &[u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2] {
        &self.signed_receipt
    }

    pub const fn imported_proof(&self) -> &ImportedFunctionalRefinementProofV2 {
        &self.imported
    }

    pub const fn identity(&self) -> ProductionMirPlironVerusExecutionEvidenceIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn authenticates_signed_receipt_under_embedded_key(&self) -> bool {
        self.imported.signature_and_policy_verified()
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalProductionMirPlironVerusExecutionEvidenceV1")
            .field("claims", &self.claims)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn import_signed_receipt(
    claims: ProductionMirPlironVerusExecutionClaimsV1,
    verifying_key: [u8; VERIFYING_KEY_BYTES_V1],
    wire: &[u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
) -> Result<ImportedFunctionalRefinementProofV2, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying_key,
        claims.toolchain(),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .map_err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport)?;
    if policy.signer_identity() != claims.signer_identity() {
        return Err(
            ProductionMirPlironVerusExecutionEvidenceErrorV1::ReportMismatch("receipt signer"),
        );
    }
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .map_err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport)?;
    let imported = importer
        .import(
            FunctionalRefinementImportExpectationV2::new(claims.binding()),
            wire,
        )
        .map_err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport)?;
    for (matches, field) in [
        (imported.binding() == claims.binding(), "receipt binding"),
        (
            imported.toolchain() == claims.toolchain(),
            "receipt toolchain",
        ),
        (
            imported.execution_identity() == claims.execution_identity(),
            "execution identity",
        ),
        (
            imported.receipt_identity().digest() == claims.receipt_identity(),
            "receipt identity",
        ),
    ] {
        if !matches {
            return Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReportMismatch(field));
        }
    }
    Ok(imported)
}

fn encode(
    claims: ProductionMirPlironVerusExecutionClaimsV1,
    verifying_key: [u8; VERIFYING_KEY_BYTES_V1],
    signed_receipt: &[u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
) -> Box<[u8]> {
    let mut canonical = Vec::with_capacity(PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1);
    canonical.extend_from_slice(&MAGIC_V1);
    canonical.extend_from_slice(&VERSION_V1.to_le_bytes());
    canonical.extend_from_slice(&FLAGS_V1.to_le_bytes());
    canonical.extend_from_slice(
        &(PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1 as u32).to_le_bytes(),
    );
    canonical.extend_from_slice(&0_u32.to_le_bytes());
    canonical.push(claims.binding().safe_reference_kind() as u8);
    canonical.push(MIR_TO_LIVE_PLIRON_BOUNDARY_TAG_V1);
    canonical.push(PROVED_RESULT_TAG_V1);
    canonical.push(STRICT_IMPORT_ASSURANCE_TAG_V1);
    canonical.extend_from_slice(&claims.retained_policy_checked_staging().to_le_bytes());
    for digest in claims.ordered_digests() {
        canonical.extend_from_slice(digest.as_bytes());
    }
    canonical.extend_from_slice(&verifying_key);
    canonical.extend_from_slice(signed_receipt);
    let identity = terminal_identity(&canonical);
    canonical.extend_from_slice(&identity);
    debug_assert_eq!(
        canonical.len(),
        PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1
    );
    canonical.into_boxed_slice()
}

fn terminal_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], ProductionMirPlironVerusExecutionEvidenceErrorV1> {
    bytes
        .get(offset..offset.saturating_add(N))
        .and_then(|value| value.try_into().ok())
        .ok_or(ProductionMirPlironVerusExecutionEvidenceErrorV1::Truncated)
}

fn read_u16(
    bytes: &[u8],
    offset: usize,
) -> Result<u16, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
    Ok(u16::from_le_bytes(read_fixed(bytes, offset)?))
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
) -> Result<u32, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
    Ok(u32::from_le_bytes(read_fixed(bytes, offset)?))
}

fn read_u64(
    bytes: &[u8],
    offset: usize,
) -> Result<u64, ProductionMirPlironVerusExecutionEvidenceErrorV1> {
    Ok(u64::from_le_bytes(read_fixed(bytes, offset)?))
}

#[derive(Debug)]
pub enum ProductionMirPlironVerusExecutionEvidenceErrorV1 {
    InvalidLength { expected: usize, actual: usize },
    InvalidMagic,
    UnsupportedVersion,
    UnsupportedFlags,
    DeclaredLengthMismatch,
    NonzeroReserved,
    Truncated,
    InvalidReferenceKind(u8),
    InvalidAssurance,
    ZeroIdentity(&'static str),
    ZeroRetainedStaging,
    IdentityMismatch,
    ReportMismatch(&'static str),
    ReceiptImport(FunctionalRefinementImportErrorV2),
    NonCanonical,
}

impl fmt::Display for ProductionMirPlironVerusExecutionEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "aggregate Verus evidence length {actual} does not equal {expected}"
            ),
            Self::InvalidMagic => formatter.write_str("aggregate Verus evidence magic mismatch"),
            Self::UnsupportedVersion => {
                formatter.write_str("unsupported aggregate Verus evidence version")
            }
            Self::UnsupportedFlags => {
                formatter.write_str("unsupported aggregate Verus evidence flags")
            }
            Self::DeclaredLengthMismatch => {
                formatter.write_str("aggregate Verus evidence declared length mismatch")
            }
            Self::NonzeroReserved => {
                formatter.write_str("aggregate Verus evidence reserved field is nonzero")
            }
            Self::Truncated => formatter.write_str("aggregate Verus evidence is truncated"),
            Self::InvalidReferenceKind(value) => {
                write!(
                    formatter,
                    "aggregate Verus evidence reference kind {value} is invalid"
                )
            }
            Self::InvalidAssurance => {
                formatter.write_str("aggregate Verus evidence assurance tuple is invalid")
            }
            Self::ZeroIdentity(field) => {
                write!(formatter, "aggregate Verus {field} identity is zero")
            }
            Self::ZeroRetainedStaging => {
                formatter.write_str("aggregate Verus evidence retains no policy-checked staging")
            }
            Self::IdentityMismatch => {
                formatter.write_str("aggregate Verus evidence terminal identity mismatch")
            }
            Self::ReportMismatch(field) => {
                write!(
                    formatter,
                    "aggregate Verus report differs from the signed {field}"
                )
            }
            Self::ReceiptImport(error) => write!(
                formatter,
                "aggregate Verus signed receipt import failed: {error}"
            ),
            Self::NonCanonical => {
                formatter.write_str("aggregate Verus evidence encoding is noncanonical")
            }
        }
    }
}

impl Error for ProductionMirPlironVerusExecutionEvidenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReceiptImport(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};
    use fe2o3_functional_proof::{
        FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
        FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
        UnsignedFunctionalRefinementReceiptV2,
    };

    use super::*;

    fn digest(seed: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([seed; 32])
    }

    fn fixture() -> CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
        let binding = FunctionalRefinementBindingV2::new(
            SafeReferenceKindV2::SourceAndMir,
            digest(10),
            digest(11),
            digest(12),
            digest(13),
            digest(14),
            digest(15),
        )
        .unwrap();
        let toolchain = VerusToolchainIdentityV2::new(
            digest(20),
            digest(21),
            digest(22),
            digest(23),
            digest(24),
        )
        .unwrap();
        let signing = SigningKey::from_bytes(&[42; 32]);
        let verifying_key = signing.verifying_key().to_bytes();
        let policy = FunctionalRefinementImportPolicyV2::new(
            verifying_key,
            toolchain,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
        )
        .unwrap();
        let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
            policy.signer_identity(),
            binding,
            toolchain,
            digest(30),
            FunctionalRefinementResultV2::Proved,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
        )
        .unwrap();
        let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
        let wire = unsigned.attach_signature(signature);
        let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
        let imported = importer
            .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
            .unwrap();
        let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            binding,
            imported.signer_identity(),
            toolchain,
            imported.execution_identity(),
            imported.receipt_identity().digest(),
            7,
        )
        .unwrap();
        CanonicalProductionMirPlironVerusExecutionEvidenceV1::new(claims, verifying_key, wire)
            .unwrap()
    }

    fn rehash(bytes: &mut [u8]) {
        let terminal = bytes.len() - IDENTITY_BYTES_V1;
        let identity = terminal_identity(&bytes[..terminal]);
        bytes[terminal..].copy_from_slice(&identity);
    }

    #[test]
    fn signed_execution_evidence_roundtrips_and_retains_no_later_authority() {
        let evidence = fixture();
        assert_eq!(
            evidence.canonical_bytes().len(),
            PRODUCTION_MIR_PLIRON_VERUS_EXECUTION_EVIDENCE_BYTES_V1
        );
        let decoded = CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(
            evidence.canonical_bytes(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_bytes(), evidence.canonical_bytes());
        assert_eq!(decoded.claims(), evidence.claims());
        assert_eq!(decoded.signed_receipt(), evidence.signed_receipt());
        assert_eq!(decoded.verifying_key(), evidence.verifying_key());
        assert!(decoded.authenticates_signed_receipt_under_embedded_key());
        assert!(!decoded.authenticates_compiler_origin());
        assert!(!decoded.grants_llvm_or_later_authority());
        assert!(!decoded.grants_runtime_authority());
    }

    #[test]
    fn every_prefix_trailing_byte_and_unrehashed_mutation_fail_closed() {
        let evidence = fixture();
        let bytes = evidence.canonical_bytes();
        for prefix in 0..bytes.len() {
            assert!(
                CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&bytes[..prefix])
                    .is_err(),
                "accepted prefix {prefix}"
            );
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&trailing).is_err());
        for offset in 0..bytes.len() - IDENTITY_BYTES_V1 {
            let mut mutated = bytes.to_vec();
            mutated[offset] ^= 0x80;
            assert!(
                CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&mutated).is_err(),
                "accepted mutation at {offset}"
            );
        }
    }

    #[test]
    fn rehashed_key_and_signed_wire_substitutions_still_fail_receipt_import() {
        let evidence = fixture();
        let key_offset = HEADER_BYTES_V1
            + META_BYTES_V1
            + RETAINED_COUNT_BYTES_V1
            + DIGEST_COUNT_V1 * DIGEST_BYTES_V1;
        let wire_offset = key_offset + VERIFYING_KEY_BYTES_V1;

        let mut key_substitution = evidence.canonical_bytes().to_vec();
        key_substitution[key_offset] ^= 0x80;
        rehash(&mut key_substitution);
        assert!(matches!(
            CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&key_substitution),
            Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport(_))
                | Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReportMismatch(_))
        ));

        let mut wire_substitution = evidence.canonical_bytes().to_vec();
        wire_substitution[wire_offset + 100] ^= 0x80;
        rehash(&mut wire_substitution);
        assert!(matches!(
            CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&wire_substitution),
            Err(ProductionMirPlironVerusExecutionEvidenceErrorV1::ReceiptImport(_))
        ));
    }

    #[test]
    fn zero_report_identity_and_zero_retained_staging_are_rejected() {
        let evidence = fixture();
        let claims = evidence.claims();
        assert!(
            ProductionMirPlironVerusExecutionClaimsV1::new(
                DigestV1::ZERO,
                claims.parallel_contract_identity(),
                claims.pliron_evidence_identity(),
                claims.composition_template_identity(),
                claims.generated_source_identity(),
                claims.binding(),
                claims.signer_identity(),
                claims.toolchain(),
                claims.execution_identity(),
                claims.receipt_identity(),
                claims.retained_policy_checked_staging(),
            )
            .is_err()
        );
        assert!(
            ProductionMirPlironVerusExecutionClaimsV1::new(
                claims.contract_identity(),
                claims.parallel_contract_identity(),
                claims.pliron_evidence_identity(),
                claims.composition_template_identity(),
                claims.generated_source_identity(),
                claims.binding(),
                claims.signer_identity(),
                claims.toolchain(),
                claims.execution_identity(),
                claims.receipt_identity(),
                0,
            )
            .is_err()
        );
    }
}
