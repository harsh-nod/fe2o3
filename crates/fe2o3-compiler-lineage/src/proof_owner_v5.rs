use std::{error::Error, fmt};

use fe2o3_proof_contracts::{
    CapabilityCodecErrorV1, CapabilitySubjectV1, DigestV1, ExecutableKirIdentityV1,
    KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1, TargetModelIdentityV1,
};
use sha2::{Digest, Sha256};

use crate::{
    InertCapabilityRefinementReceiptIdentityV1, InertLineageContentIdentityV3,
    InertMultiRootProofLineageIdentityV3, InertMultiRootProofLineageV3,
    InertStaticCapabilityEvidenceAssociationIdentityV1, MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3,
    MultiRootProofRosterKindV3,
};

/// Magic for the native canonical-KIR-V13 compiler proof owner.
pub const INERT_COMPILER_PROOF_OWNER_MAGIC_V5: [u8; 8] = *b"F2O3POV5";
/// Exact wire version of the native canonical-KIR-V13 compiler proof owner.
pub const INERT_COMPILER_PROOF_OWNER_VERSION_V5: u16 = 5;
/// Maximum canonical size accepted for one V5 owner association.
pub const MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5: usize = 64 * 1024;
/// Maximum exact canonical KIR V13 bytes retained by one side-by-side V5 receipt.
pub const MAX_INERT_CANONICAL_KERNEL_IR_V13_RECEIPT_BYTES_V5: usize = 4 * 1024 * 1024;

const RECORD_KIND_V5: u16 = 9;
const EXACT_V13_OWNER_POLICY_V5: u16 = 1;
const HEADER_BYTES_V5: usize = 24;
const FIELD_HEADER_BYTES_V5: usize = 8;
const FIELD_COUNT_V5: usize = 18;
const DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-PROOF-OWNER/CANONICAL-KIR-V13/V5\0";
const CLAIM_V5: &[u8] =
    b"exact-v13-graph-and-epoch/source-machine-receipts/no-v8-projection/no-runtime-authority";
const IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-PROOF-OWNER-IDENTITY/V5\0";
const KIR_RECEIPT_IDENTITY_DOMAIN_V5: &[u8] =
    b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-KERNEL-IR-V13/V5\0";
const SUBJECT_ROSTER_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-PROOF-OWNER/SUBJECT-ROSTER/V5\0";

/// Domain-separated identity of exact canonical KIR V13 bytes carried beside V3/V4 lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCanonicalKernelIrV13ReceiptIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertCanonicalKernelIrV13ReceiptIdentityV5 {
    /// Returns the domain-separated receipt digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact canonical KIR V13 byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn from_exact_identity(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(InertCompilerProofOwnerErrorV5::InvalidIdentity);
        }
        Ok(Self { sha256, byte_len })
    }
}

/// Side-by-side inert receipt whose preimage must decode as exact canonical KIR V13.
///
/// Construction derives content identity only. The verifier owns canonical V13 decoding and this
/// value grants no compiler, proof, publication, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalKernelIrV13ReceiptV5 {
    canonical_preimage: Box<[u8]>,
    identity: InertCanonicalKernelIrV13ReceiptIdentityV5,
}

impl InertCanonicalKernelIrV13ReceiptV5 {
    /// Retains nonempty bounded candidate canonical KIR V13 bytes.
    pub fn from_canonical_preimage(
        canonical_preimage: impl Into<Vec<u8>>,
    ) -> Result<Self, InertCanonicalKernelIrV13ReceiptErrorV5> {
        let canonical_preimage = canonical_preimage.into();
        if canonical_preimage.is_empty() {
            return Err(InertCanonicalKernelIrV13ReceiptErrorV5::EmptyPreimage);
        }
        if canonical_preimage.len() > MAX_INERT_CANONICAL_KERNEL_IR_V13_RECEIPT_BYTES_V5 {
            return Err(InertCanonicalKernelIrV13ReceiptErrorV5::TooLarge);
        }
        let byte_len = u64::try_from(canonical_preimage.len())
            .map_err(|_| InertCanonicalKernelIrV13ReceiptErrorV5::LengthOverflow)?;
        let mut digest = Sha256::new();
        digest.update(KIR_RECEIPT_IDENTITY_DOMAIN_V5);
        digest.update(byte_len.to_le_bytes());
        digest.update(&canonical_preimage);
        let identity = InertCanonicalKernelIrV13ReceiptIdentityV5 {
            sha256: digest.finalize().into(),
            byte_len,
        };
        Ok(Self {
            canonical_preimage: canonical_preimage.into_boxed_slice(),
            identity,
        })
    }

    /// Returns the exact retained candidate canonical KIR V13 bytes.
    pub fn canonical_preimage(&self) -> &[u8] {
        &self.canonical_preimage
    }

    /// Returns the domain-separated exact V5 receipt identity.
    pub const fn identity(&self) -> InertCanonicalKernelIrV13ReceiptIdentityV5 {
        self.identity
    }
}

/// Representation error for the side-by-side canonical KIR V13 receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertCanonicalKernelIrV13ReceiptErrorV5 {
    /// The receipt preimage is empty.
    EmptyPreimage,
    /// The receipt exceeds its hard maximum.
    TooLarge,
    /// The receipt length cannot be represented.
    LengthOverflow,
}

impl fmt::Display for InertCanonicalKernelIrV13ReceiptErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid canonical KIR V13 V5 receipt: {self:?}")
    }
}

impl Error for InertCanonicalKernelIrV13ReceiptErrorV5 {}

/// Exact source, graph, execution-subject, policy, and refinement coordinates of a V5 owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertCompilerProofOwnerInputsV5 {
    legacy_proof_binding: InertLineageContentIdentityV3,
    semantic_mir_receipt: InertLineageContentIdentityV3,
    semantic_mir_identity: [u8; 32],
    executable_kir_receipt: InertCanonicalKernelIrV13ReceiptIdentityV5,
    executable_kir_bytes: u64,
    subject: CapabilitySubjectV1,
    compiler_policy: [u8; 32],
    source_refinement: InertCapabilityRefinementReceiptIdentityV1,
    machine_refinement: InertCapabilityRefinementReceiptIdentityV1,
    capability_association: InertStaticCapabilityEvidenceAssociationIdentityV1,
}

impl InertCompilerProofOwnerInputsV5 {
    /// Constructs exact, non-authoritative owner coordinates.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        legacy_proof_binding: InertLineageContentIdentityV3,
        semantic_mir_receipt: InertLineageContentIdentityV3,
        semantic_mir_identity: [u8; 32],
        executable_kir_receipt: InertCanonicalKernelIrV13ReceiptIdentityV5,
        executable_kir_bytes: u64,
        subject: CapabilitySubjectV1,
        compiler_policy: [u8; 32],
        source_refinement: InertCapabilityRefinementReceiptIdentityV1,
        machine_refinement: InertCapabilityRefinementReceiptIdentityV1,
        capability_association: InertStaticCapabilityEvidenceAssociationIdentityV1,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        if semantic_mir_identity == [0; 32]
            || compiler_policy == [0; 32]
            || executable_kir_bytes == 0
        {
            return Err(InertCompilerProofOwnerErrorV5::InvalidIdentity);
        }
        if executable_kir_bytes != executable_kir_receipt.byte_len() {
            return Err(InertCompilerProofOwnerErrorV5::ExecutableKirReceiptLengthMismatch);
        }
        Ok(Self {
            legacy_proof_binding,
            semantic_mir_receipt,
            semantic_mir_identity,
            executable_kir_receipt,
            executable_kir_bytes,
            subject,
            compiler_policy,
            source_refinement,
            machine_refinement,
            capability_association,
        })
    }

    /// Returns the frozen V4 proof-binding receipt identity retained as source lineage.
    pub const fn legacy_proof_binding(self) -> InertLineageContentIdentityV3 {
        self.legacy_proof_binding
    }

    /// Returns the exact semantic-MIR receipt identity.
    pub const fn semantic_mir_receipt(self) -> InertLineageContentIdentityV3 {
        self.semantic_mir_receipt
    }

    /// Returns the exact decoded semantic-MIR identity.
    pub const fn semantic_mir_identity(self) -> [u8; 32] {
        self.semantic_mir_identity
    }

    /// Returns the receipt identity whose preimage is exact canonical KIR V13.
    pub const fn executable_kir_receipt(self) -> InertCanonicalKernelIrV13ReceiptIdentityV5 {
        self.executable_kir_receipt
    }

    /// Returns the exact canonical KIR V13 byte length.
    pub const fn executable_kir_bytes(self) -> u64 {
        self.executable_kir_bytes
    }

    /// Returns the complete exact V13 graph subject, including optimization epoch.
    pub const fn subject(self) -> CapabilitySubjectV1 {
        self.subject
    }

    /// Returns the exact protected compiler-policy identity.
    pub const fn compiler_policy(self) -> [u8; 32] {
        self.compiler_policy
    }

    /// Returns the exact source/MIR-to-KIR refinement receipt identity.
    pub const fn source_refinement(self) -> InertCapabilityRefinementReceiptIdentityV1 {
        self.source_refinement
    }

    /// Returns the exact machine-refinement receipt identity.
    pub const fn machine_refinement(self) -> InertCapabilityRefinementReceiptIdentityV1 {
        self.machine_refinement
    }

    /// Returns the exact capability association composed into this owner.
    pub const fn capability_association(
        self,
    ) -> InertStaticCapabilityEvidenceAssociationIdentityV1 {
        self.capability_association
    }
}

/// Domain-separated identity of one exact canonical V5 owner association.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCompilerProofOwnerIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertCompilerProofOwnerIdentityV5 {
    /// Returns the exact domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact canonical byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Canonical inert association for a native V13 compiler proof owner.
///
/// This record is descriptive and grants no authority. A sealed protected verifier must validate
/// its exact inputs and authenticate producer policy before host admission can retain it.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCompilerProofOwnerV5 {
    canonical_bytes: Box<[u8]>,
    inputs: InertCompilerProofOwnerInputsV5,
    proof_lineage: Option<InertMultiRootProofLineageIdentityV3>,
    subjects: Box<[CapabilitySubjectV1]>,
    selected_subject_ordinal: u32,
    selected_capability_association: InertStaticCapabilityEvidenceAssociationIdentityV1,
    identity: InertCompilerProofOwnerIdentityV5,
}

impl InertCompilerProofOwnerV5 {
    /// Canonically associates one exact V13 proof-owner subject.
    pub fn new(
        inputs: InertCompilerProofOwnerInputsV5,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        Self::encode(
            inputs,
            None,
            vec![inputs.subject],
            0,
            inputs.capability_association,
        )
    }

    /// Binds the complete canonical subject roster and selects the exact matching subject.
    ///
    /// Selection is rederived from the complete canonical roster. Use
    /// [`Self::new_multi_root_at_ordinal`] when the caller already owns the expected ordinal and
    /// needs an explicit ordinal check at construction.
    pub fn new_multi_root(
        inputs: InertCompilerProofOwnerInputsV5,
        proof_lineage: &InertMultiRootProofLineageV3,
        subjects: Vec<CapabilitySubjectV1>,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        let selected_subject_ordinal = subjects
            .iter()
            .position(|subject| *subject == inputs.subject)
            .ok_or(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch)?;
        let selected_subject_ordinal = u32::try_from(selected_subject_ordinal)
            .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?;
        Self::new_multi_root_at_ordinal(inputs, proof_lineage, subjects, selected_subject_ordinal)
    }

    /// Binds a complete proof roster and one exact descriptor-order subject ordinal.
    pub fn new_multi_root_at_ordinal(
        inputs: InertCompilerProofOwnerInputsV5,
        proof_lineage: &InertMultiRootProofLineageV3,
        subjects: Vec<CapabilitySubjectV1>,
        selected_subject_ordinal: u32,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        Self::new_multi_root_for_association_at_ordinal(
            inputs,
            proof_lineage,
            subjects,
            selected_subject_ordinal,
            inputs.capability_association,
        )
    }

    /// Binds complete proof and capability-association rosters to one exact selected entry.
    ///
    /// For a multi-root owner, `inputs.capability_association()` identifies the complete canonical
    /// capability-association roster. `selected_capability_association` identifies exactly the
    /// entry at `selected_subject_ordinal`; a verifier must rederive both from retained bytes.
    pub fn new_multi_root_for_association_at_ordinal(
        inputs: InertCompilerProofOwnerInputsV5,
        proof_lineage: &InertMultiRootProofLineageV3,
        subjects: Vec<CapabilitySubjectV1>,
        selected_subject_ordinal: u32,
        selected_capability_association: InertStaticCapabilityEvidenceAssociationIdentityV1,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        validate_subjects_against_lineage_v5(inputs, proof_lineage, &subjects)?;
        Self::encode(
            inputs,
            Some(proof_lineage.identity()),
            subjects,
            selected_subject_ordinal,
            selected_capability_association,
        )
    }

    fn encode(
        inputs: InertCompilerProofOwnerInputsV5,
        proof_lineage_identity: Option<InertMultiRootProofLineageIdentityV3>,
        subjects: Vec<CapabilitySubjectV1>,
        selected_subject_ordinal: u32,
        selected_capability_association: InertStaticCapabilityEvidenceAssociationIdentityV1,
    ) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        validate_subject_roster_v5(inputs, &subjects, selected_subject_ordinal)?;
        let legacy = encode_lineage(inputs.legacy_proof_binding);
        let semantic_receipt = encode_lineage(inputs.semantic_mir_receipt);
        let kir_receipt = encode_kir_receipt(inputs.executable_kir_receipt);
        let kir_identity = encode_kir_identity(inputs.subject, inputs.executable_kir_bytes);
        let epoch = inputs.subject.executable_kir_epoch().to_le_bytes();
        let proof_lineage = proof_lineage_identity
            .map(|identity| encode_proof_lineage(identity).to_vec())
            .unwrap_or_default();
        let subject_roster = encode_subject_roster_v5(&subjects)?;
        let subject_roster_identity = derive_subject_roster_identity_v5(&subject_roster);
        let source = encode_refinement(inputs.source_refinement);
        let machine = encode_refinement(inputs.machine_refinement);
        let capability = encode_capability_association(inputs.capability_association);
        let target = inputs.subject.target_model().digest();
        let selected_subject_ordinal_bytes = selected_subject_ordinal.to_le_bytes();
        let selected_capability = encode_capability_association(selected_capability_association);
        let fields: [&[u8]; FIELD_COUNT_V5] = [
            DOMAIN_V5,
            CLAIM_V5,
            &legacy,
            &semantic_receipt,
            &inputs.semantic_mir_identity,
            &kir_receipt,
            &kir_identity,
            &epoch,
            &proof_lineage,
            &subject_roster,
            target.as_bytes(),
            &subject_roster_identity,
            &inputs.compiler_policy,
            &source,
            &machine,
            &capability,
            &selected_subject_ordinal_bytes,
            &selected_capability,
        ];
        let total_len = encoded_len(&fields)?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(total_len)
            .map_err(|_| InertCompilerProofOwnerErrorV5::AllocationFailed)?;
        canonical.extend_from_slice(&INERT_COMPILER_PROOF_OWNER_MAGIC_V5);
        canonical.extend_from_slice(&INERT_COMPILER_PROOF_OWNER_VERSION_V5.to_le_bytes());
        canonical.extend_from_slice(&RECORD_KIND_V5.to_le_bytes());
        canonical.extend_from_slice(&EXACT_V13_OWNER_POLICY_V5.to_le_bytes());
        canonical.extend_from_slice(&(FIELD_COUNT_V5 as u16).to_le_bytes());
        canonical.extend_from_slice(
            &u32::try_from(total_len)
                .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        for (index, field) in fields.iter().enumerate() {
            canonical.extend_from_slice(
                &u16::try_from(index + 1)
                    .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
                    .to_le_bytes(),
            );
            canonical.extend_from_slice(&0_u16.to_le_bytes());
            canonical.extend_from_slice(
                &u32::try_from(field.len())
                    .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
                    .to_le_bytes(),
            );
            canonical.extend_from_slice(field);
        }
        let canonical_bytes = canonical.into_boxed_slice();
        let identity = owner_identity(&canonical_bytes)?;
        Ok(Self {
            canonical_bytes,
            inputs,
            proof_lineage: proof_lineage_identity,
            subjects: subjects.into_boxed_slice(),
            selected_subject_ordinal,
            selected_capability_association,
            identity,
        })
    }

    /// Strictly decodes one complete V5 association without legacy fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertCompilerProofOwnerErrorV5> {
        if bytes.len() > MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5 {
            return Err(InertCompilerProofOwnerErrorV5::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V5 {
            return Err(InertCompilerProofOwnerErrorV5::Truncated);
        }
        if bytes[..8] != INERT_COMPILER_PROOF_OWNER_MAGIC_V5 {
            return Err(InertCompilerProofOwnerErrorV5::InvalidMagic);
        }
        if read_u16(bytes, 8)? != INERT_COMPILER_PROOF_OWNER_VERSION_V5 {
            return Err(InertCompilerProofOwnerErrorV5::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != RECORD_KIND_V5 {
            return Err(InertCompilerProofOwnerErrorV5::WrongRecordKind);
        }
        if read_u16(bytes, 12)? != EXACT_V13_OWNER_POLICY_V5 {
            return Err(InertCompilerProofOwnerErrorV5::WrongPolicy);
        }
        if usize::from(read_u16(bytes, 14)?) != FIELD_COUNT_V5 {
            return Err(InertCompilerProofOwnerErrorV5::WrongFieldCount);
        }
        if usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
            != bytes.len()
        {
            return Err(InertCompilerProofOwnerErrorV5::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(InertCompilerProofOwnerErrorV5::NonzeroReserved);
        }
        let expected_lengths = [
            DOMAIN_V5.len(),
            CLAIM_V5.len(),
            40,
            40,
            32,
            40,
            40,
            8,
            usize::MAX,
            usize::MAX,
            32,
            32,
            32,
            40,
            40,
            40,
            4,
            40,
        ];
        let mut fields: [&[u8]; FIELD_COUNT_V5] = [&[]; FIELD_COUNT_V5];
        let mut offset = HEADER_BYTES_V5;
        for index in 0..FIELD_COUNT_V5 {
            if read_u16(bytes, offset)?
                != u16::try_from(index + 1)
                    .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
            {
                return Err(InertCompilerProofOwnerErrorV5::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(InertCompilerProofOwnerErrorV5::NonzeroFieldFlags);
            }
            let len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?;
            if expected_lengths[index] != usize::MAX && len != expected_lengths[index] {
                return Err(InertCompilerProofOwnerErrorV5::InvalidFieldLength);
            }
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V5)
                .ok_or(InertCompilerProofOwnerErrorV5::LengthOverflow)?;
            let end = start
                .checked_add(len)
                .ok_or(InertCompilerProofOwnerErrorV5::LengthOverflow)?;
            fields[index] = bytes
                .get(start..end)
                .ok_or(InertCompilerProofOwnerErrorV5::Truncated)?;
            offset = end;
        }
        if offset != bytes.len() {
            return Err(InertCompilerProofOwnerErrorV5::TrailingBytes);
        }
        if fields[0] != DOMAIN_V5 {
            return Err(InertCompilerProofOwnerErrorV5::WrongDomain);
        }
        if fields[1] != CLAIM_V5 {
            return Err(InertCompilerProofOwnerErrorV5::WrongClaim);
        }
        if !matches!(fields[8].len(), 0 | 40) {
            return Err(InertCompilerProofOwnerErrorV5::InvalidFieldLength);
        }
        let executable_kir_bytes = read_exact_u64(fields[6], 32)?;
        let epoch = read_exact_u64(fields[7], 0)?;
        let subjects = decode_subject_roster_v5(fields[9])?;
        let selected_subject_ordinal = read_exact_u32(fields[16], 0)?;
        let selected_capability_association = decode_capability_association(fields[17])?;
        let subject = *subjects
            .get(
                usize::try_from(selected_subject_ordinal)
                    .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?,
            )
            .ok_or(InertCompilerProofOwnerErrorV5::SelectedSubjectOrdinalOutOfRange)?;
        if subject.executable_kir().digest() != decode_digest(&fields[6][..32])?
            || subject.executable_kir_epoch() != epoch
            || subject.target_model().digest() != decode_digest(fields[10])?
            || derive_subject_roster_identity_v5(fields[9]) != copy_32(fields[11])?
        {
            return Err(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch);
        }
        let inputs = InertCompilerProofOwnerInputsV5::new(
            decode_lineage(fields[2])?,
            decode_lineage(fields[3])?,
            copy_32(fields[4])?,
            decode_kir_receipt(fields[5])?,
            executable_kir_bytes,
            subject,
            copy_32(fields[12])?,
            decode_refinement(fields[13])?,
            decode_refinement(fields[14])?,
            decode_capability_association(fields[15])?,
        )?;
        let proof_lineage = if fields[8].is_empty() {
            None
        } else {
            Some(decode_proof_lineage(fields[8])?)
        };
        let decoded = Self::encode(
            inputs,
            proof_lineage,
            subjects,
            selected_subject_ordinal,
            selected_capability_association,
        )?;
        if decoded.canonical_bytes() != bytes {
            return Err(InertCompilerProofOwnerErrorV5::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns exact canonical V5 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns all exact coordinates.
    pub const fn inputs(&self) -> InertCompilerProofOwnerInputsV5 {
        self.inputs
    }

    /// Returns the exact native proof-lineage identity, absent only for legacy singleton callers.
    pub const fn proof_lineage(&self) -> Option<InertMultiRootProofLineageIdentityV3> {
        self.proof_lineage
    }

    /// Returns every exact execution subject in canonical descriptor-kernel order.
    pub fn subjects(&self) -> &[CapabilitySubjectV1] {
        &self.subjects
    }

    /// Returns the selected subject's descriptor-order ordinal in the retained roster.
    pub const fn selected_subject_ordinal(&self) -> u32 {
        self.selected_subject_ordinal
    }

    /// Returns the exact selected subject.
    pub fn selected_subject(&self) -> CapabilitySubjectV1 {
        self.subjects[self.selected_subject_ordinal as usize]
    }

    /// Returns the exact capability-association entry selected for this subject.
    pub const fn selected_capability_association(
        &self,
    ) -> InertStaticCapabilityEvidenceAssociationIdentityV1 {
        self.selected_capability_association
    }

    /// Returns the domain-separated exact record identity.
    pub const fn identity(&self) -> InertCompilerProofOwnerIdentityV5 {
        self.identity
    }
}

fn encode_lineage(identity: InertLineageContentIdentityV3) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn encode_proof_lineage(identity: InertMultiRootProofLineageIdentityV3) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn decode_proof_lineage(
    bytes: &[u8],
) -> Result<InertMultiRootProofLineageIdentityV3, InertCompilerProofOwnerErrorV5> {
    InertMultiRootProofLineageIdentityV3::from_exact_identity_v3(
        copy_32(&bytes[..32])?,
        read_exact_u64(bytes, 32)?,
    )
    .ok_or(InertCompilerProofOwnerErrorV5::InvalidIdentity)
}

fn encode_subject_roster_v5(
    subjects: &[CapabilitySubjectV1],
) -> Result<Vec<u8>, InertCompilerProofOwnerErrorV5> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&subjects.len()) {
        return Err(InertCompilerProofOwnerErrorV5::InvalidRosterCount);
    }
    let mut bytes = Vec::with_capacity(4 + subjects.len() * 168);
    bytes.extend_from_slice(&(subjects.len() as u32).to_le_bytes());
    for subject in subjects {
        bytes.extend_from_slice(subject.kernel().digest().as_bytes());
        bytes.extend_from_slice(subject.root().digest().as_bytes());
        bytes.extend_from_slice(subject.executable_kir().digest().as_bytes());
        bytes.extend_from_slice(&subject.executable_kir_epoch().to_le_bytes());
        bytes.extend_from_slice(subject.target_model().digest().as_bytes());
        bytes.extend_from_slice(subject.launch_contract().digest().as_bytes());
    }
    Ok(bytes)
}

fn decode_subject_roster_v5(
    bytes: &[u8],
) -> Result<Vec<CapabilitySubjectV1>, InertCompilerProofOwnerErrorV5> {
    let count = usize::try_from(read_u32(bytes, 0)?)
        .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?;
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count)
        || bytes.len() != 4 + count * 168
    {
        return Err(InertCompilerProofOwnerErrorV5::InvalidRosterCount);
    }
    let mut subjects = Vec::with_capacity(count);
    for index in 0..count {
        let value = &bytes[4 + index * 168..4 + (index + 1) * 168];
        subjects.push(
            CapabilitySubjectV1::new(
                KernelIdentityV1::from_untrusted_digest(decode_digest(&value[..32])?),
                KernelRootIdentityV1::from_untrusted_digest(decode_digest(&value[32..64])?),
                ExecutableKirIdentityV1::from_untrusted_digest(decode_digest(&value[64..96])?),
                read_exact_u64(value, 96)?,
                TargetModelIdentityV1::from_untrusted_digest(decode_digest(&value[104..136])?),
                LaunchContractIdentityV1::from_untrusted_digest(decode_digest(&value[136..])?),
            )
            .map_err(InertCompilerProofOwnerErrorV5::Capability)?,
        );
    }
    Ok(subjects)
}

fn derive_subject_roster_identity_v5(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SUBJECT_ROSTER_IDENTITY_DOMAIN_V5);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn validate_subject_roster_v5(
    inputs: InertCompilerProofOwnerInputsV5,
    subjects: &[CapabilitySubjectV1],
    selected_subject_ordinal: u32,
) -> Result<(), InertCompilerProofOwnerErrorV5> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&subjects.len()) {
        return Err(InertCompilerProofOwnerErrorV5::InvalidRosterCount);
    }
    let selected = *subjects
        .get(
            usize::try_from(selected_subject_ordinal)
                .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?,
        )
        .ok_or(InertCompilerProofOwnerErrorV5::SelectedSubjectOrdinalOutOfRange)?;
    if selected != inputs.subject {
        return Err(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch);
    }
    let mut previous_kernel = None;
    let mut roots = std::collections::BTreeSet::new();
    for subject in subjects {
        let kernel = *subject.kernel().digest().as_bytes();
        if previous_kernel.is_some_and(|previous| kernel <= previous)
            || !roots.insert(*subject.root().digest().as_bytes())
            || subject.executable_kir() != selected.executable_kir()
            || subject.executable_kir_epoch() != selected.executable_kir_epoch()
            || subject.target_model() != selected.target_model()
        {
            return Err(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch);
        }
        previous_kernel = Some(kernel);
    }
    Ok(())
}

fn validate_subjects_against_lineage_v5(
    inputs: InertCompilerProofOwnerInputsV5,
    lineage: &InertMultiRootProofLineageV3,
    subjects: &[CapabilitySubjectV1],
) -> Result<(), InertCompilerProofOwnerErrorV5> {
    let roster = lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    if subjects.len() != roster.root_count()
        || inputs.semantic_mir_identity() != roster.semantic_mir_sha256()
        || inputs.executable_kir_bytes() != roster.neutral_kir().canonical_length()
    {
        return Err(InertCompilerProofOwnerErrorV5::InvalidRosterCount);
    }
    for (subject_index, root_index) in roster.canonical_kernel_order().iter().enumerate() {
        let root = roster
            .root(*root_index as usize)
            .ok_or(InertCompilerProofOwnerErrorV5::InvalidRosterCount)?;
        let subject = subjects[subject_index];
        if subject.kernel().digest().as_bytes() != &root.kernel_binding()
            || subject.root().digest().as_bytes() != &root.semantic_root_identity()
            || subject.executable_kir().digest().as_bytes() != &roster.neutral_kir().digest()
            || subject.executable_kir_epoch() != roster.neutral_kir().graph_epoch()
        {
            return Err(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch);
        }
    }
    Ok(())
}

fn decode_lineage(
    bytes: &[u8],
) -> Result<InertLineageContentIdentityV3, InertCompilerProofOwnerErrorV5> {
    InertLineageContentIdentityV3::new(copy_32(&bytes[..32])?, read_exact_u64(bytes, 32)?)
        .map_err(|_| InertCompilerProofOwnerErrorV5::InvalidIdentity)
}

fn encode_kir_identity(subject: CapabilitySubjectV1, byte_len: u64) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(subject.executable_kir().digest().as_bytes());
    bytes[32..].copy_from_slice(&byte_len.to_le_bytes());
    bytes
}

fn encode_kir_receipt(identity: InertCanonicalKernelIrV13ReceiptIdentityV5) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn encode_refinement(identity: InertCapabilityRefinementReceiptIdentityV1) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn decode_refinement(
    bytes: &[u8],
) -> Result<InertCapabilityRefinementReceiptIdentityV1, InertCompilerProofOwnerErrorV5> {
    InertCapabilityRefinementReceiptIdentityV1::from_exact_identity_v1(
        copy_32(&bytes[..32])?,
        read_exact_u64(bytes, 32)?,
    )
    .map_err(|_| InertCompilerProofOwnerErrorV5::InvalidIdentity)
}

fn decode_kir_receipt(
    bytes: &[u8],
) -> Result<InertCanonicalKernelIrV13ReceiptIdentityV5, InertCompilerProofOwnerErrorV5> {
    InertCanonicalKernelIrV13ReceiptIdentityV5::from_exact_identity(
        copy_32(&bytes[..32])?,
        read_exact_u64(bytes, 32)?,
    )
}

fn encode_capability_association(
    identity: InertStaticCapabilityEvidenceAssociationIdentityV1,
) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn decode_capability_association(
    bytes: &[u8],
) -> Result<InertStaticCapabilityEvidenceAssociationIdentityV1, InertCompilerProofOwnerErrorV5> {
    InertStaticCapabilityEvidenceAssociationIdentityV1::from_exact_identity_v1(
        copy_32(&bytes[..32])?,
        read_exact_u64(bytes, 32)?,
    )
    .map_err(|_| InertCompilerProofOwnerErrorV5::InvalidIdentity)
}

fn decode_digest(bytes: &[u8]) -> Result<DigestV1, InertCompilerProofOwnerErrorV5> {
    let digest = DigestV1::from_untrusted_bytes(copy_32(bytes)?);
    if digest.is_zero() {
        return Err(InertCompilerProofOwnerErrorV5::InvalidIdentity);
    }
    Ok(digest)
}

fn copy_32(bytes: &[u8]) -> Result<[u8; 32], InertCompilerProofOwnerErrorV5> {
    bytes
        .try_into()
        .map_err(|_| InertCompilerProofOwnerErrorV5::InvalidFieldLength)
}

fn read_exact_u64(bytes: &[u8], offset: usize) -> Result<u64, InertCompilerProofOwnerErrorV5> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(InertCompilerProofOwnerErrorV5::Truncated)?
        .try_into()
        .map_err(|_| InertCompilerProofOwnerErrorV5::Truncated)?;
    Ok(u64::from_le_bytes(value))
}

fn read_exact_u32(bytes: &[u8], offset: usize) -> Result<u32, InertCompilerProofOwnerErrorV5> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(InertCompilerProofOwnerErrorV5::Truncated)?
        .try_into()
        .map_err(|_| InertCompilerProofOwnerErrorV5::Truncated)?;
    Ok(u32::from_le_bytes(value))
}

fn encoded_len(fields: &[&[u8]]) -> Result<usize, InertCompilerProofOwnerErrorV5> {
    let headers = fields
        .len()
        .checked_mul(FIELD_HEADER_BYTES_V5)
        .ok_or(InertCompilerProofOwnerErrorV5::LengthOverflow)?;
    let payload = fields.iter().try_fold(0_usize, |sum, field| {
        sum.checked_add(field.len())
            .ok_or(InertCompilerProofOwnerErrorV5::LengthOverflow)
    })?;
    let total = HEADER_BYTES_V5
        .checked_add(headers)
        .and_then(|value| value.checked_add(payload))
        .ok_or(InertCompilerProofOwnerErrorV5::LengthOverflow)?;
    if total > MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5 {
        return Err(InertCompilerProofOwnerErrorV5::TooLarge);
    }
    Ok(total)
}

fn owner_identity(
    bytes: &[u8],
) -> Result<InertCompilerProofOwnerIdentityV5, InertCompilerProofOwnerErrorV5> {
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?;
    let mut digest = Sha256::new();
    digest.update(
        u32::try_from(IDENTITY_DOMAIN_V5.len())
            .map_err(|_| InertCompilerProofOwnerErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    digest.update(IDENTITY_DOMAIN_V5);
    digest.update(INERT_COMPILER_PROOF_OWNER_VERSION_V5.to_le_bytes());
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    Ok(InertCompilerProofOwnerIdentityV5 {
        sha256: digest.finalize().into(),
        byte_len,
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InertCompilerProofOwnerErrorV5> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(InertCompilerProofOwnerErrorV5::Truncated)?
        .try_into()
        .map_err(|_| InertCompilerProofOwnerErrorV5::Truncated)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InertCompilerProofOwnerErrorV5> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(InertCompilerProofOwnerErrorV5::Truncated)?
        .try_into()
        .map_err(|_| InertCompilerProofOwnerErrorV5::Truncated)?;
    Ok(u32::from_le_bytes(value))
}

/// Strict representation error for a V5 proof-owner association.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertCompilerProofOwnerErrorV5 {
    /// Record exceeds its hard maximum.
    TooLarge,
    /// Allocation failed.
    AllocationFailed,
    /// Arithmetic or host conversion overflowed.
    LengthOverflow,
    /// Record ended early.
    Truncated,
    /// Magic is not V5 owner magic.
    InvalidMagic,
    /// Version is not exactly V5.
    UnsupportedVersion,
    /// Record kind differs.
    WrongRecordKind,
    /// Policy differs.
    WrongPolicy,
    /// Field count differs.
    WrongFieldCount,
    /// Declared and actual lengths differ.
    DeclaredLengthMismatch,
    /// Reserved header bits are nonzero.
    NonzeroReserved,
    /// A field tag differs.
    WrongFieldTag,
    /// Field flags are nonzero.
    NonzeroFieldFlags,
    /// A fixed-width field has the wrong length.
    InvalidFieldLength,
    /// Domain differs.
    WrongDomain,
    /// Claim differs.
    WrongClaim,
    /// Bytes remain after the final field.
    TrailingBytes,
    /// An identity or required epoch is zero/invalid.
    InvalidIdentity,
    /// The declared KIR byte count differs from the exact V5 receipt length.
    ExecutableKirReceiptLengthMismatch,
    /// The subject roster is empty, oversized, or incomplete.
    InvalidRosterCount,
    /// A subject is duplicated, noncanonical, or names different graph coordinates.
    RosterSubjectMismatch,
    /// The selected descriptor-order subject ordinal does not name a retained roster entry.
    SelectedSubjectOrdinalOutOfRange,
    /// The execution subject is invalid.
    Capability(CapabilityCodecErrorV1),
    /// Re-encoding did not reproduce the input.
    NonCanonical,
}

impl fmt::Display for InertCompilerProofOwnerErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid canonical V5 compiler proof owner: {self:?}"
        )
    }
}

impl Error for InertCompilerProofOwnerErrorV5 {}
