use std::{error::Error, fmt, ops::Range};

use fe2o3_proof_contracts::{
    CapabilityCodecErrorV1, CapabilityCompositionErrorV1, CapabilitySubjectV1, DigestV1,
    ExecutableKirIdentityV1, InertCapabilityObligationSetIdentityV1,
    InertCapabilityObligationSetV1, InertCapabilityResultSetIdentityV1, InertCapabilityResultSetV1,
    KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1, TargetModelIdentityV1,
    validate_capability_composition_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    InertLineageContentIdentityV3, InertMultiRootProofLineageIdentityV3,
    InertMultiRootProofLineageV3, MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3, MultiRootProofRosterKindV3,
    TargetMachineRefinementReceiptV1,
};

/// Magic for the side-by-side static capability-evidence association.
pub const INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1: [u8; 8] = *b"F2CAPEV1";
/// Exact wire version for the side-by-side static capability-evidence association.
pub const INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1: u16 = 1;
/// Maximum canonical association bytes, including both nested capability sets.
pub const MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum canonical bytes in one bounded multi-root capability-evidence roster.
pub const MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1: usize =
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
        * MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1
        + 4096;
/// Maximum bytes retained for one refinement receipt.
pub const MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1: usize = 4 * 1024 * 1024;

const RECORD_KIND_V1: u16 = 1;
const ASSOCIATION_ONLY_POLICY_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 24;
const FIELD_HEADER_BYTES_V1: usize = 8;
const FIELD_COUNT_V1: usize = 16;
const LINEAGE_IDENTITY_BYTES_V1: usize = 40;
const SUBJECT_BYTES_V1: usize = 32 * 5 + 8;
const SET_IDENTITY_BYTES_V1: usize = 32;
const TERMINAL_IDENTITY_BYTES_V1: usize = 32;
const DOMAIN_V1: &[u8] = b"FE2O3/CAPABILITY/STATIC-EVIDENCE/IDENTITY/V1\0";
const CLAIM_V1: &[u8] =
    b"inert exact association/no producer-publication-currentness-load-launch authority";
const SOURCE_REFINEMENT_DOMAIN_V1: &[u8] =
    b"FE2O3/CAPABILITY/SOURCE-MIR-TO-KIR-REFINEMENT-RECEIPT/V1\0";
const MACHINE_REFINEMENT_DOMAIN_V1: &[u8] = b"FE2O3/CAPABILITY/MACHINE-REFINEMENT-RECEIPT/V1\0";
const MULTI_ROOT_ASSOCIATION_MAGIC_V1: [u8; 8] = *b"F2CAPMR1";
const MULTI_ROOT_ASSOCIATION_VERSION_V1: u16 = 1;
const MULTI_ROOT_ASSOCIATION_POLICY_V1: u16 = 1;
const MULTI_ROOT_ASSOCIATION_HEADER_BYTES_V1: usize = 24;
const MULTI_ROOT_ASSOCIATION_TERMINAL_BYTES_V1: usize = 32;
const MULTI_ROOT_ASSOCIATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CAPABILITY/MULTI-ROOT-STATIC-EVIDENCE-IDENTITY/V1\0";
const SOURCE_REFINEMENT_JOIN_MAGIC_V1: [u8; 8] = *b"F2CSRCV1";
const SOURCE_REFINEMENT_JOIN_VERSION_V1: u16 = 1;
const SOURCE_REFINEMENT_JOIN_POLICY_V1: u16 = 1;
const SOURCE_REFINEMENT_JOIN_HEADER_BYTES_V1: usize = 148;
const SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1: usize = 32;
const SOURCE_REFINEMENT_JOIN_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CAPABILITY/CHECKED-SOURCE-REFINEMENT-TO-FINAL-LINEAGE/V1\0";
const SOURCE_EVIDENCE_MAGIC_V1: [u8; 8] = *b"F2SREFV1";
const SOURCE_EVIDENCE_VERSION_V1: u16 = 1;
const SOURCE_EVIDENCE_POLICY_V1: u16 = 1;
const SOURCE_EVIDENCE_HEADER_BYTES_V1: usize = 136;
const SOURCE_EVIDENCE_TERMINAL_BYTES_V1: usize = 32;
const SOURCE_EVIDENCE_MAX_NAME_BYTES_V1: usize = 4096;
const SOURCE_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CHECKED-SOURCE-MIR-TO-KIR-REFINEMENT-EVIDENCE/V1\0";
const CORRESPONDENCE_V5_MAGIC: [u8; 8] = *b"F2M2K5\0\0";
const CORRESPONDENCE_V5_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/EXACT-FUNCTION-MIR-TO-KIR-CORRESPONDENCE-EVIDENCE/V5\0";

mod expanded_source_evidence_v2;
pub use expanded_source_evidence_v2::{
    EXPANDED_SOURCE_EVIDENCE_MAGIC_V2, ExpandedSourceRootV2, InertExpandedSourceEvidenceV2,
};

/// The two refinement boundaries whose exact receipt bytes may be associated by W6.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InertCapabilityRefinementReceiptKindV1 {
    /// Source/semantic-MIR to executable-KIR refinement.
    SourceMirToKir,
    /// LLVM/ISA to final-machine refinement.
    Machine,
}

impl InertCapabilityRefinementReceiptKindV1 {
    const fn domain(self) -> &'static [u8] {
        match self {
            Self::SourceMirToKir => SOURCE_REFINEMENT_DOMAIN_V1,
            Self::Machine => MACHINE_REFINEMENT_DOMAIN_V1,
        }
    }
}

/// Domain-separated coordinates of one refinement receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCapabilityRefinementReceiptIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertCapabilityRefinementReceiptIdentityV1 {
    /// Returns the exact domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact opaque receipt length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Reconstructs an exact inert identity while decoding a containing canonical record.
    pub fn from_exact_identity_v1(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Self::new(sha256, byte_len)
    }

    fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidIdentity);
        }
        Ok(Self { sha256, byte_len })
    }
}

/// Exact refinement-receipt bytes retained without authenticating their issuer.
///
/// Source receipts are validated by their source-specific join. Machine receipts are strictly
/// decoded as [`TargetMachineRefinementReceiptV1`]. Construction here establishes schema and
/// bounded content identity, not target-coordinate equality or issuer authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCapabilityRefinementReceiptV1 {
    kind: InertCapabilityRefinementReceiptKindV1,
    canonical_preimage: Box<[u8]>,
    identity: InertCapabilityRefinementReceiptIdentityV1,
    typed_machine_refinement: Option<TargetMachineRefinementReceiptV1>,
}

impl InertCapabilityRefinementReceiptV1 {
    /// Retains one nonempty bounded receipt and derives its domain-specific identity.
    pub fn from_canonical_preimage(
        kind: InertCapabilityRefinementReceiptKindV1,
        canonical_preimage: impl Into<Vec<u8>>,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        let canonical_preimage = canonical_preimage.into();
        if canonical_preimage.is_empty() {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::EmptyRefinementReceipt);
        }
        if canonical_preimage.len() > MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementReceiptTooLarge);
        }
        if kind == InertCapabilityRefinementReceiptKindV1::SourceMirToKir {
            if canonical_preimage.starts_with(&expanded_source_evidence_v2::JOIN_MAGIC) {
                expanded_source_evidence_v2::decode_join(&canonical_preimage)?;
            } else {
                ParsedSourceRefinementJoinV1::decode(&canonical_preimage)?;
            }
        }
        let typed_machine_refinement =
            if kind == InertCapabilityRefinementReceiptKindV1::Machine {
                Some(TargetMachineRefinementReceiptV1::decode(&canonical_preimage).map_err(|_| {
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidMachineRefinementReceipt
            })?)
            } else {
                None
            };
        let byte_len = u64::try_from(canonical_preimage.len())
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        let identity = InertCapabilityRefinementReceiptIdentityV1::new(
            derive_identity(kind.domain(), &canonical_preimage),
            byte_len,
        )?;
        Ok(Self {
            kind,
            canonical_preimage: canonical_preimage.into_boxed_slice(),
            identity,
            typed_machine_refinement,
        })
    }

    /// Composes live-owner-issued source evidence with the exact final V13 proof lineage.
    ///
    /// The source evidence must be the complete canonical output of
    /// `ProductionSourceRefinementEvidenceV1::from_live_owner`; a digest, label, or arbitrary
    /// transcript is rejected. The resulting receipt binds that source endpoint to the exact
    /// final graph, epoch, and canonical root roster without granting adjacent authority.
    pub fn from_checked_source_evidence_v1(
        source_evidence: &[u8],
        final_lineage: &InertMultiRootProofLineageV3,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        let parsed = ParsedSourceEvidenceV1::decode(source_evidence)?;
        validate_source_evidence_against_lineage_v1(&parsed, final_lineage)?;
        let lineage = final_lineage.identity();
        let neutral = final_lineage.neutral_kir();
        let source_length = u32::try_from(source_evidence.len())
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        let total = SOURCE_REFINEMENT_JOIN_HEADER_BYTES_V1
            .checked_add(source_evidence.len())
            .and_then(|value| value.checked_add(SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1))
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if total > MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementReceiptTooLarge);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
        bytes.extend_from_slice(&SOURCE_REFINEMENT_JOIN_MAGIC_V1);
        bytes.extend_from_slice(&SOURCE_REFINEMENT_JOIN_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&SOURCE_REFINEMENT_JOIN_POLICY_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(total)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&source_length.to_le_bytes());
        bytes.extend_from_slice(&lineage.sha256());
        bytes.extend_from_slice(&lineage.byte_len().to_le_bytes());
        bytes.extend_from_slice(&parsed.semantic_mir_sha256);
        bytes.extend_from_slice(&neutral.digest());
        bytes.extend_from_slice(&neutral.canonical_length().to_le_bytes());
        bytes.extend_from_slice(&neutral.graph_epoch().to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(parsed.roots.len())
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(source_evidence);
        let terminal = derive_identity(SOURCE_REFINEMENT_JOIN_IDENTITY_DOMAIN_V1, &bytes);
        bytes.extend_from_slice(&terminal);
        debug_assert_eq!(bytes.len(), total);
        Self::from_canonical_preimage(
            InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            bytes,
        )
    }

    /// Returns the refinement boundary named by this opaque receipt.
    pub const fn kind(&self) -> InertCapabilityRefinementReceiptKindV1 {
        self.kind
    }

    /// Returns the exact receipt bytes.
    pub fn canonical_preimage(&self) -> &[u8] {
        &self.canonical_preimage
    }

    /// Returns the domain-separated exact content identity.
    pub const fn identity(&self) -> InertCapabilityRefinementReceiptIdentityV1 {
        self.identity
    }

    /// Returns the strictly decoded target-specific machine receipt, when present.
    pub const fn typed_machine_refinement(&self) -> Option<&TargetMachineRefinementReceiptV1> {
        self.typed_machine_refinement.as_ref()
    }

    /// Revalidates a version-tagged source receipt against final lineage and ordered subjects.
    /// V1 and expanded V2 inner schemas remain distinct; neither decoder grants authority.
    pub fn validate_source_subject_roster_v1(
        &self,
        final_lineage: &InertMultiRootProofLineageV3,
        subjects: &[CapabilitySubjectV1],
    ) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
        if self.kind != InertCapabilityRefinementReceiptKindV1::SourceMirToKir {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongRefinementKind);
        }
        if self
            .canonical_preimage
            .starts_with(&expanded_source_evidence_v2::JOIN_MAGIC)
        {
            return expanded_source_evidence_v2::validate_receipt(
                &self.canonical_preimage,
                final_lineage,
                subjects,
            );
        }
        let join = ParsedSourceRefinementJoinV1::decode(&self.canonical_preimage)?;
        let source = ParsedSourceEvidenceV1::decode(join.source_evidence)?;
        validate_source_evidence_against_lineage_v1(&source, final_lineage)?;
        join.validate_subjects(
            final_lineage,
            subjects,
            source.semantic_mir_sha256,
            source.roots.len(),
        )
    }
}

impl ParsedSourceRefinementJoinV1<'_> {
    fn validate_subjects(
        &self,
        final_lineage: &InertMultiRootProofLineageV3,
        subjects: &[CapabilitySubjectV1],
        semantic_mir_sha256: [u8; 32],
        root_count: usize,
    ) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
        let lineage = final_lineage.identity();
        let neutral = final_lineage.neutral_kir();
        if self.lineage != lineage
            || self.semantic_mir_sha256 != semantic_mir_sha256
            || self.final_kir_sha256 != neutral.digest()
            || self.final_kir_bytes != neutral.canonical_length()
            || self.final_epoch != neutral.graph_epoch()
            || self.root_count != root_count
            || subjects.len() != root_count
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch);
        }
        let roster = final_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
        for (subject_index, root_index) in roster.canonical_kernel_order().iter().enumerate() {
            let root_index = usize::try_from(*root_index)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            let root = roster.root(root_index).ok_or(
                InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch,
            )?;
            let subject = subjects.get(subject_index).ok_or(
                InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch,
            )?;
            if subject.kernel().digest().as_bytes() != &root.kernel_binding()
                || subject.root().digest().as_bytes() != &root.semantic_root_identity()
                || subject.executable_kir().digest().as_bytes() != &neutral.digest()
                || subject.executable_kir_epoch() != neutral.graph_epoch()
            {
                return Err(
                    InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch,
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ParsedSourceRefinementJoinV1<'a> {
    lineage: InertMultiRootProofLineageIdentityV3,
    semantic_mir_sha256: [u8; 32],
    final_kir_sha256: [u8; 32],
    final_kir_bytes: u64,
    final_epoch: u64,
    root_count: usize,
    source_evidence: &'a [u8],
}

impl<'a> ParsedSourceRefinementJoinV1<'a> {
    fn decode(bytes: &'a [u8]) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Self::decode_frame(
            bytes,
            SOURCE_REFINEMENT_JOIN_MAGIC_V1,
            SOURCE_REFINEMENT_JOIN_VERSION_V1,
            SOURCE_REFINEMENT_JOIN_IDENTITY_DOMAIN_V1,
        )
    }

    // Only framing is shared. Callers select an exact version and validate its own source schema.
    fn decode_frame(
        bytes: &'a [u8],
        magic: [u8; 8],
        version: u16,
        domain: &[u8],
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if bytes.len() > MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1
            || bytes.len()
                < SOURCE_REFINEMENT_JOIN_HEADER_BYTES_V1 + SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let mut reader = SourceEvidenceReaderV1::new(bytes);
        if reader.fixed::<8>()? != magic
            || reader.u16()? != version
            || reader.u16()? != SOURCE_REFINEMENT_JOIN_POLICY_V1
            || reader.u32()? != 0
            || reader.usize_u32()? != bytes.len()
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let source_len = reader.usize_u32()?;
        let lineage_sha256 = reader.fixed::<32>()?;
        let lineage_len = reader.u64()?;
        let lineage = InertMultiRootProofLineageIdentityV3::from_exact_identity_v3(
            lineage_sha256,
            lineage_len,
        )
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidIdentity)?;
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        let final_kir_sha256 = reader.fixed::<32>()?;
        let final_kir_bytes = reader.u64()?;
        let final_epoch = reader.u64()?;
        let root_count = reader.usize_u32()?;
        if semantic_mir_sha256 == [0; 32]
            || final_kir_sha256 == [0; 32]
            || final_kir_bytes == 0
            || final_epoch == 0
            || !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&root_count)
            || source_len == 0
            || reader.remaining()
                != source_len
                    .checked_add(SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1)
                    .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let source_evidence = reader.take(source_len)?;
        let terminal = reader.fixed::<SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1>()?;
        reader.finish()?;
        let preimage_len = bytes.len() - SOURCE_REFINEMENT_JOIN_TERMINAL_BYTES_V1;
        if terminal != derive_identity(domain, &bytes[..preimage_len]) {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        Ok(Self {
            lineage,
            semantic_mir_sha256,
            final_kir_sha256,
            final_kir_bytes,
            final_epoch,
            root_count,
            source_evidence,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedSourceEvidenceRootV1 {
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    kernel_entry_ordinal: u32,
    kernel_function_ordinal: u32,
    export_symbol: Box<str>,
    kernel_id: Box<str>,
    kernel_function: Box<str>,
}

#[derive(Debug)]
struct ParsedSourceEvidenceV1 {
    semantic_mir_sha256: [u8; 32],
    roots: Box<[ParsedSourceEvidenceRootV1]>,
}

impl ParsedSourceEvidenceV1 {
    fn decode(bytes: &[u8]) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if bytes.len() < SOURCE_EVIDENCE_HEADER_BYTES_V1 + SOURCE_EVIDENCE_TERMINAL_BYTES_V1
            || bytes.len() > MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let mut reader = SourceEvidenceReaderV1::new(bytes);
        if reader.fixed::<8>()? != SOURCE_EVIDENCE_MAGIC_V1
            || reader.u16()? != SOURCE_EVIDENCE_VERSION_V1
            || reader.u16()? != SOURCE_EVIDENCE_POLICY_V1
            || reader.u32()? != 0
            || reader.usize_u32()? != bytes.len()
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        let source_kir_version = reader.u16()?;
        if !matches!(source_kir_version, 8 | 9 | 11 | 12 | 13) || reader.u16()? != 0 {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let source_kir_bytes = reader.u64()?;
        let source_kir_sha256 = reader.fixed::<32>()?;
        let correspondence_identity = reader.fixed::<32>()?;
        let root_count = reader.usize_u32()?;
        let correspondence_len = reader.usize_u32()?;
        if semantic_mir_sha256 == [0; 32]
            || source_kir_sha256 == [0; 32]
            || source_kir_bytes == 0
            || correspondence_identity == [0; 32]
            || !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&root_count)
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(root_count)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
        let mut previous_root = None;
        let mut root_identities = std::collections::BTreeSet::new();
        let mut bindings = std::collections::BTreeSet::new();
        let mut function_ordinals = std::collections::BTreeSet::new();
        for expected_kernel_ordinal in 0..root_count {
            let semantic_root = reader.u32()?;
            let semantic_root_identity = reader.fixed::<32>()?;
            let kernel_binding = reader.fixed::<32>()?;
            let kernel_entry_ordinal = reader.u32()?;
            let kernel_function_ordinal = reader.u32()?;
            if previous_root.is_some_and(|previous| semantic_root <= previous)
                || semantic_root_identity == [0; 32]
                || kernel_binding == [0; 32]
                || kernel_entry_ordinal
                    != u32::try_from(expected_kernel_ordinal).map_err(|_| {
                        InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow
                    })?
                || !root_identities.insert(semantic_root_identity)
                || !bindings.insert(kernel_binding)
                || !function_ordinals.insert(kernel_function_ordinal)
            {
                return Err(
                    InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
                );
            }
            previous_root = Some(semantic_root);
            let export_symbol = reader.text(SOURCE_EVIDENCE_MAX_NAME_BYTES_V1)?;
            let kernel_id = reader.text(SOURCE_EVIDENCE_MAX_NAME_BYTES_V1)?;
            let kernel_function = reader.text(SOURCE_EVIDENCE_MAX_NAME_BYTES_V1)?;
            if export_symbol != kernel_id || kernel_id != kernel_function {
                return Err(
                    InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
                );
            }
            roots.push(ParsedSourceEvidenceRootV1 {
                semantic_root,
                semantic_root_identity,
                kernel_binding,
                kernel_entry_ordinal,
                kernel_function_ordinal,
                export_symbol,
                kernel_id,
                kernel_function,
            });
        }
        if correspondence_len == 0
            || reader.remaining()
                != correspondence_len
                    .checked_add(SOURCE_EVIDENCE_TERMINAL_BYTES_V1)
                    .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        let correspondence = reader.take(correspondence_len)?;
        validate_correspondence_v5(
            correspondence,
            correspondence_identity,
            semantic_mir_sha256,
            source_kir_sha256,
            source_kir_bytes,
            source_kir_version,
            &roots,
        )?;
        let terminal = reader.fixed::<SOURCE_EVIDENCE_TERMINAL_BYTES_V1>()?;
        reader.finish()?;
        let preimage_len = bytes.len() - SOURCE_EVIDENCE_TERMINAL_BYTES_V1;
        if terminal != derive_identity(SOURCE_EVIDENCE_IDENTITY_DOMAIN_V1, &bytes[..preimage_len]) {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        Ok(Self {
            semantic_mir_sha256,
            roots: roots.into_boxed_slice(),
        })
    }
}

fn validate_source_evidence_against_lineage_v1(
    source: &ParsedSourceEvidenceV1,
    lineage: &InertMultiRootProofLineageV3,
) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
    let roster = lineage.roster(MultiRootProofRosterKindV3::Correspondence);
    if source.semantic_mir_sha256 != roster.semantic_mir_sha256()
        || source.roots.len() != roster.root_count()
    {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch);
    }
    for (index, source_root) in source.roots.iter().enumerate() {
        let root = roster
            .root(index)
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch)?;
        if source_root.semantic_root != root.semantic_root()
            || source_root.semantic_root_identity != root.semantic_root_identity()
            || source_root.kernel_binding != root.kernel_binding()
            || source_root.export_symbol.as_ref() != root.export_symbol()
            || source_root.kernel_id.as_ref() != root.kernel_id()
            || source_root.kernel_function.as_ref() != root.kernel_id()
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RefinementSubjectMismatch);
        }
    }
    Ok(())
}

fn validate_correspondence_v5(
    bytes: &[u8],
    expected_identity: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    source_kir_sha256: [u8; 32],
    source_kir_bytes: u64,
    source_kir_version: u16,
    roots: &[ParsedSourceEvidenceRootV1],
) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
    let mut reader = SourceEvidenceReaderV1::new(bytes);
    if reader.fixed::<8>()? != CORRESPONDENCE_V5_MAGIC
        || reader.u16()? != 5
        || reader.u16()? != 1
        || reader.u32()? != 0
        || reader.usize_u32()? != bytes.len()
    {
        return Err(
            InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
        );
    }
    let nested_len = reader.usize_u32()?;
    let function_count = reader.usize_u32()?;
    if nested_len < 124 || function_count == 0 || nested_len > reader.remaining() {
        return Err(
            InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
        );
    }
    let nested = reader.take(nested_len)?;
    if nested.get(..8) != Some(b"F2M2K4\0\0")
        || read_u16(nested, 8)? != 4
        || read_u16(nested, 10)? != 1
        || read_u32(nested, 12)? != 0
        || usize::try_from(read_u32(nested, 16)?)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
            != nested.len()
        || nested.get(20..52) != Some(&semantic_mir_sha256)
        || read_u16(nested, 52)? != source_kir_version
        || read_u16(nested, 54)? != 0
        || read_u64_at(nested, 56)? != source_kir_bytes
        || nested.get(64..96) != Some(&source_kir_sha256)
    {
        return Err(
            InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
        );
    }
    let mut previous = None;
    let mut ordinals = std::collections::BTreeSet::new();
    let mut entry_records = Vec::new();
    entry_records
        .try_reserve_exact(roots.len())
        .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
    for _ in 0..function_count {
        let owner = reader.u32()?;
        let semantic_function = reader.u32()?;
        let ordinal = reader.u32()?;
        let role = reader.u8()?;
        if reader.fixed::<3>()? != [0; 3]
            || previous.is_some_and(|prior| (owner, semantic_function) <= prior)
            || !ordinals.insert(ordinal)
            || !(1..=2).contains(&role)
        {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        previous = Some((owner, semantic_function));
        let function = reader.text(SOURCE_EVIDENCE_MAX_NAME_BYTES_V1)?;
        if role == 1 {
            entry_records.push((owner, ordinal, function));
        }
    }
    reader.finish()?;
    if entry_records.len() != roots.len()
        || roots.iter().any(|root| {
            entry_records
                .iter()
                .filter(|(owner, ordinal, function)| {
                    *owner == root.semantic_root
                        && *ordinal == root.kernel_function_ordinal
                        && function.as_ref() == root.kernel_function.as_ref()
                })
                .count()
                != 1
        })
    {
        return Err(
            InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
        );
    }
    let actual_identity = derive_identity(CORRESPONDENCE_V5_IDENTITY_DOMAIN, bytes);
    if actual_identity != expected_identity {
        return Err(
            InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
        );
    }
    Ok(())
}

struct SourceEvidenceReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SourceEvidenceReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], InertStaticCapabilityEvidenceAssociationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], InertStaticCapabilityEvidenceAssociationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, InertStaticCapabilityEvidenceAssociationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn usize_u32(&mut self) -> Result<usize, InertStaticCapabilityEvidenceAssociationErrorV1> {
        usize::try_from(self.u32()?)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)
    }

    fn text(
        &mut self,
        max: usize,
    ) -> Result<Box<str>, InertStaticCapabilityEvidenceAssociationErrorV1> {
        let length = self.usize_u32()?;
        if length == 0 || length > max {
            return Err(
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt,
            );
        }
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map(String::into_boxed_str)
            .map_err(|_| {
                InertStaticCapabilityEvidenceAssociationErrorV1::InvalidSourceRefinementReceipt
            })
    }

    fn finish(self) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(InertStaticCapabilityEvidenceAssociationErrorV1::TrailingBytes)
        }
    }
}

fn read_u64_at(
    bytes: &[u8],
    offset: usize,
) -> Result<u64, InertStaticCapabilityEvidenceAssociationErrorV1> {
    bytes
        .get(offset..offset.saturating_add(8))
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)
}

/// Exact frozen-capsule and stage coordinates associated with capability evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertStaticCapabilityEvidenceAssociationInputsV1 {
    capsule: InertLineageContentIdentityV3,
    kernel_ir: InertLineageContentIdentityV3,
    proof_binding: InertLineageContentIdentityV3,
    target_binding: InertLineageContentIdentityV3,
    target_lowering: InertLineageContentIdentityV3,
    semantic_to_llvm: InertLineageContentIdentityV3,
    final_compiler_module: InertLineageContentIdentityV3,
    source_refinement: Option<InertCapabilityRefinementReceiptIdentityV1>,
    machine_refinement: Option<InertCapabilityRefinementReceiptIdentityV1>,
}

impl InertStaticCapabilityEvidenceAssociationInputsV1 {
    /// Constructs inert exact coordinates for the frozen V3 capsule and its applicable receipts.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        capsule: InertLineageContentIdentityV3,
        kernel_ir: InertLineageContentIdentityV3,
        proof_binding: InertLineageContentIdentityV3,
        target_binding: InertLineageContentIdentityV3,
        target_lowering: InertLineageContentIdentityV3,
        semantic_to_llvm: InertLineageContentIdentityV3,
        final_compiler_module: InertLineageContentIdentityV3,
        source_refinement: Option<InertCapabilityRefinementReceiptIdentityV1>,
        machine_refinement: Option<InertCapabilityRefinementReceiptIdentityV1>,
    ) -> Self {
        Self {
            capsule,
            kernel_ir,
            proof_binding,
            target_binding,
            target_lowering,
            semantic_to_llvm,
            final_compiler_module,
            source_refinement,
            machine_refinement,
        }
    }

    /// Returns the exact frozen V3 capsule coordinates.
    pub const fn capsule(self) -> InertLineageContentIdentityV3 {
        self.capsule
    }
    /// Returns the exact optimized executable-KIR lineage receipt coordinates.
    pub const fn kernel_ir(self) -> InertLineageContentIdentityV3 {
        self.kernel_ir
    }
    /// Returns the exact V4 proof-binding receipt coordinates.
    pub const fn proof_binding(self) -> InertLineageContentIdentityV3 {
        self.proof_binding
    }
    /// Returns the exact target-binding receipt coordinates.
    pub const fn target_binding(self) -> InertLineageContentIdentityV3 {
        self.target_binding
    }
    /// Returns the exact target-lowering receipt coordinates.
    pub const fn target_lowering(self) -> InertLineageContentIdentityV3 {
        self.target_lowering
    }
    /// Returns the exact semantic-to-LLVM receipt coordinates.
    pub const fn semantic_to_llvm(self) -> InertLineageContentIdentityV3 {
        self.semantic_to_llvm
    }
    /// Returns the exact final compiler-module receipt coordinates.
    pub const fn final_compiler_module(self) -> InertLineageContentIdentityV3 {
        self.final_compiler_module
    }
    /// Returns the optional exact source-refinement receipt coordinates.
    pub const fn source_refinement(self) -> Option<InertCapabilityRefinementReceiptIdentityV1> {
        self.source_refinement
    }
    /// Returns the optional exact machine-refinement receipt coordinates.
    pub const fn machine_refinement(self) -> Option<InertCapabilityRefinementReceiptIdentityV1> {
        self.machine_refinement
    }

    const fn ordered_lineage(self) -> [InertLineageContentIdentityV3; 7] {
        [
            self.capsule,
            self.kernel_ir,
            self.proof_binding,
            self.target_binding,
            self.target_lowering,
            self.semantic_to_llvm,
            self.final_compiler_module,
        ]
    }
}

/// Identity of one complete side-by-side static capability-evidence association.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertStaticCapabilityEvidenceAssociationIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertStaticCapabilityEvidenceAssociationIdentityV1 {
    /// Reconstructs an exact inert identity while decoding a containing canonical record.
    pub fn from_exact_identity_v1(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidIdentity);
        }
        Ok(Self { sha256, byte_len })
    }
}

impl InertStaticCapabilityEvidenceAssociationIdentityV1 {
    /// Returns the association digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Returns the complete canonical wire length, including the terminal digest.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Side-by-side association of exact capability evidence with the frozen #209 lineage.
///
/// This record does not alter the V3 capsule or V4 proof-binding wire. It preserves exact bytes
/// and coordinates only; a sealed verifier must independently decode and authenticate every
/// referenced receipt before this evidence can participate in a later authority join.
#[derive(Debug, Eq, PartialEq)]
pub struct InertStaticCapabilityEvidenceAssociationV1 {
    canonical_bytes: Box<[u8]>,
    identity: InertStaticCapabilityEvidenceAssociationIdentityV1,
    inputs: InertStaticCapabilityEvidenceAssociationInputsV1,
    subject: CapabilitySubjectV1,
    obligation_identity: InertCapabilityObligationSetIdentityV1,
    result_identity: InertCapabilityResultSetIdentityV1,
    obligation_range: Range<usize>,
    result_range: Range<usize>,
}

impl InertStaticCapabilityEvidenceAssociationV1 {
    /// Canonically associates one complete obligation/result pair with exact lineage coordinates.
    pub fn new(
        inputs: InertStaticCapabilityEvidenceAssociationInputsV1,
        obligations: &InertCapabilityObligationSetV1,
        results: &InertCapabilityResultSetV1,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        let subject = obligations.subject();
        validate_capability_composition_v1(subject, obligations, results)
            .map_err(InertStaticCapabilityEvidenceAssociationErrorV1::Composition)?;
        Self::encode(
            inputs,
            subject,
            obligations.identity(),
            results.identity(),
            obligations.canonical_bytes(),
            results.canonical_bytes(),
        )
    }

    /// Strictly decodes exact V1 association bytes without fallback or projection.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if bytes.len() > MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V1 + TERMINAL_IDENTITY_BYTES_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated);
        }
        if bytes[..8] != INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidMagic);
        }
        if read_u16(bytes, 8)? != INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != RECORD_KIND_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongRecordKind);
        }
        if read_u16(bytes, 12)? != ASSOCIATION_ONLY_POLICY_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongPolicy);
        }
        if usize::from(read_u16(bytes, 14)?) != FIELD_COUNT_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongFieldCount);
        }
        let declared = usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if declared != bytes.len() {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::NonzeroReserved);
        }

        let mut fields: [&[u8]; FIELD_COUNT_V1] = [&[]; FIELD_COUNT_V1];
        let mut ranges: [Range<usize>; FIELD_COUNT_V1] = std::array::from_fn(|_| 0..0);
        let mut offset = HEADER_BYTES_V1;
        for index in 0..FIELD_COUNT_V1 {
            if read_u16(bytes, offset)? != u16::try_from(index + 1).unwrap() {
                return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(InertStaticCapabilityEvidenceAssociationErrorV1::NonzeroFieldFlags);
            }
            let len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V1)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            let end = start
                .checked_add(len)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            fields[index] = bytes
                .get(start..end)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)?;
            ranges[index] = start..end;
            offset = end;
        }
        let terminal_end = offset
            .checked_add(TERMINAL_IDENTITY_BYTES_V1)
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if terminal_end != bytes.len() {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TrailingBytes);
        }
        if fields[0] != DOMAIN_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongDomain);
        }
        if fields[1] != CLAIM_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::WrongClaim);
        }

        let inputs = InertStaticCapabilityEvidenceAssociationInputsV1::new(
            decode_lineage_identity(fields[2])?,
            decode_lineage_identity(fields[3])?,
            decode_lineage_identity(fields[4])?,
            decode_lineage_identity(fields[5])?,
            decode_lineage_identity(fields[6])?,
            decode_lineage_identity(fields[7])?,
            decode_lineage_identity(fields[8])?,
            decode_optional_refinement_identity(fields[12])?,
            decode_optional_refinement_identity(fields[13])?,
        );
        let subject = decode_subject(fields[9])?;
        let obligation_identity = decode_obligation_identity(fields[10])?;
        let result_identity = decode_result_identity(fields[11])?;
        let obligations = InertCapabilityObligationSetV1::decode_canonical(fields[14])
            .map_err(InertStaticCapabilityEvidenceAssociationErrorV1::CapabilityCodec)?;
        let results = InertCapabilityResultSetV1::decode_canonical(fields[15])
            .map_err(InertStaticCapabilityEvidenceAssociationErrorV1::CapabilityCodec)?;
        if obligations.identity() != obligation_identity || results.identity() != result_identity {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::SetIdentityMismatch);
        }
        validate_capability_composition_v1(subject, &obligations, &results)
            .map_err(InertStaticCapabilityEvidenceAssociationErrorV1::Composition)?;

        let mut terminal = [0_u8; 32];
        terminal.copy_from_slice(&bytes[offset..terminal_end]);
        let expected = derive_identity(DOMAIN_V1, &bytes[..offset]);
        if terminal == [0; 32] || terminal != expected {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::IdentityMismatch);
        }
        let decoded = Self::encode(
            inputs,
            subject,
            obligation_identity,
            result_identity,
            fields[14],
            fields[15],
        )?;
        if decoded.canonical_bytes() != bytes {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::NonCanonical);
        }
        Ok(Self {
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: InertStaticCapabilityEvidenceAssociationIdentityV1 {
                sha256: terminal,
                byte_len: u64::try_from(bytes.len())
                    .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?,
            },
            inputs,
            subject,
            obligation_identity,
            result_identity,
            obligation_range: ranges[14].clone(),
            result_range: ranges[15].clone(),
        })
    }

    /// Returns exact canonical association bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Returns the exact association identity.
    pub const fn identity(&self) -> InertStaticCapabilityEvidenceAssociationIdentityV1 {
        self.identity
    }
    /// Returns the frozen capsule and lineage receipt coordinates.
    pub const fn inputs(&self) -> InertStaticCapabilityEvidenceAssociationInputsV1 {
        self.inputs
    }
    /// Returns the exact kernel/root/KIR-epoch/target/launch subject.
    pub const fn subject(&self) -> CapabilitySubjectV1 {
        self.subject
    }
    /// Returns the complete obligation-set identity.
    pub const fn obligation_set_identity(&self) -> InertCapabilityObligationSetIdentityV1 {
        self.obligation_identity
    }
    /// Returns the complete result-set identity.
    pub const fn result_set_identity(&self) -> InertCapabilityResultSetIdentityV1 {
        self.result_identity
    }
    /// Returns exact nested canonical obligation-set bytes.
    pub fn obligation_set_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.obligation_range.clone()]
    }
    /// Returns exact nested canonical result-set bytes.
    pub fn result_set_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.result_range.clone()]
    }
    #[allow(clippy::too_many_arguments)]
    fn encode(
        inputs: InertStaticCapabilityEvidenceAssociationInputsV1,
        subject: CapabilitySubjectV1,
        obligation_identity: InertCapabilityObligationSetIdentityV1,
        result_identity: InertCapabilityResultSetIdentityV1,
        obligation_bytes: &[u8],
        result_bytes: &[u8],
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        let lineage = inputs.ordered_lineage().map(encode_lineage_identity);
        let subject_bytes = encode_subject(subject);
        let source = encode_optional_refinement_identity(inputs.source_refinement);
        let machine = encode_optional_refinement_identity(inputs.machine_refinement);
        let obligation_digest = obligation_identity.digest();
        let result_digest = result_identity.digest();
        let fields: [&[u8]; FIELD_COUNT_V1] = [
            DOMAIN_V1,
            CLAIM_V1,
            &lineage[0],
            &lineage[1],
            &lineage[2],
            &lineage[3],
            &lineage[4],
            &lineage[5],
            &lineage[6],
            &subject_bytes,
            obligation_digest.as_bytes(),
            result_digest.as_bytes(),
            &source,
            &machine,
            obligation_bytes,
            result_bytes,
        ];
        let total = encoded_len(&fields)?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(total)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
        canonical.extend_from_slice(&INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1);
        canonical.extend_from_slice(
            &INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1.to_le_bytes(),
        );
        canonical.extend_from_slice(&RECORD_KIND_V1.to_le_bytes());
        canonical.extend_from_slice(&ASSOCIATION_ONLY_POLICY_V1.to_le_bytes());
        canonical.extend_from_slice(&(FIELD_COUNT_V1 as u16).to_le_bytes());
        canonical.extend_from_slice(
            &u32::try_from(total)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                .to_le_bytes(),
        );
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        let mut ranges: [Range<usize>; FIELD_COUNT_V1] = std::array::from_fn(|_| 0..0);
        for (index, field) in fields.iter().enumerate() {
            canonical.extend_from_slice(&u16::try_from(index + 1).unwrap().to_le_bytes());
            canonical.extend_from_slice(&0_u16.to_le_bytes());
            canonical.extend_from_slice(
                &u32::try_from(field.len())
                    .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                    .to_le_bytes(),
            );
            let start = canonical.len();
            canonical.extend_from_slice(field);
            ranges[index] = start..canonical.len();
        }
        let sha256 = derive_identity(DOMAIN_V1, &canonical);
        canonical.extend_from_slice(&sha256);
        debug_assert_eq!(canonical.len(), total);
        Ok(Self {
            identity: InertStaticCapabilityEvidenceAssociationIdentityV1 {
                sha256,
                byte_len: u64::try_from(total)
                    .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?,
            },
            canonical_bytes: canonical.into_boxed_slice(),
            inputs,
            subject,
            obligation_identity,
            result_identity,
            obligation_range: ranges[14].clone(),
            result_range: ranges[15].clone(),
        })
    }
}

/// Move-only, bounded, canonical roster of one exact capability association per kernel root.
#[derive(Debug, Eq, PartialEq)]
pub struct InertMultiRootStaticCapabilityEvidenceAssociationV1 {
    canonical_bytes: Box<[u8]>,
    identity: InertStaticCapabilityEvidenceAssociationIdentityV1,
    entries: Box<[InertStaticCapabilityEvidenceAssociationV1]>,
}

impl InertMultiRootStaticCapabilityEvidenceAssociationV1 {
    /// Joins a complete descriptor-order roster of independently composed root associations.
    pub fn new(
        entries: Vec<InertStaticCapabilityEvidenceAssociationV1>,
    ) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        validate_multi_root_associations_v1(&entries)?;
        let payload = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(4)
                .and_then(|value| value.checked_add(entry.canonical_bytes().len()))
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)
        })?;
        let total = MULTI_ROOT_ASSOCIATION_HEADER_BYTES_V1
            .checked_add(payload)
            .and_then(|value| value.checked_add(MULTI_ROOT_ASSOCIATION_TERMINAL_BYTES_V1))
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if total > MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1 {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TooLarge);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
        bytes.extend_from_slice(&MULTI_ROOT_ASSOCIATION_MAGIC_V1);
        bytes.extend_from_slice(&MULTI_ROOT_ASSOCIATION_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&MULTI_ROOT_ASSOCIATION_POLICY_V1.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(entries.len())
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u32::try_from(total)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for entry in &entries {
            bytes.extend_from_slice(
                &u32::try_from(entry.canonical_bytes().len())
                    .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(entry.canonical_bytes());
        }
        let terminal = derive_identity(MULTI_ROOT_ASSOCIATION_IDENTITY_DOMAIN_V1, &bytes);
        bytes.extend_from_slice(&terminal);
        debug_assert_eq!(bytes.len(), total);
        Ok(Self {
            identity: InertStaticCapabilityEvidenceAssociationIdentityV1 {
                sha256: terminal,
                byte_len: u64::try_from(total)
                    .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?,
            },
            canonical_bytes: bytes.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        })
    }

    /// Strictly decodes one complete multi-root roster without singleton projection.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertStaticCapabilityEvidenceAssociationErrorV1> {
        if bytes.len() > MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1
            || bytes.len()
                < MULTI_ROOT_ASSOCIATION_HEADER_BYTES_V1 + MULTI_ROOT_ASSOCIATION_TERMINAL_BYTES_V1
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TooLarge);
        }
        if bytes[..8] != MULTI_ROOT_ASSOCIATION_MAGIC_V1
            || read_u16(bytes, 8)? != MULTI_ROOT_ASSOCIATION_VERSION_V1
            || read_u16(bytes, 10)? != MULTI_ROOT_ASSOCIATION_POLICY_V1
            || read_u32(bytes, 20)? != 0
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidMagic);
        }
        let count = usize::try_from(read_u32(bytes, 12)?)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count) {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidRosterCount);
        }
        if usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?
            != bytes.len()
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::DeclaredLengthMismatch);
        }
        let mut offset = MULTI_ROOT_ASSOCIATION_HEADER_BYTES_V1;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::AllocationFailed)?;
        for _ in 0..count {
            let length = usize::try_from(read_u32(bytes, offset)?)
                .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            if length == 0 || length > MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1 {
                return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidFieldLength);
            }
            let start = offset
                .checked_add(4)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            let end = start
                .checked_add(length)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
            entries.push(InertStaticCapabilityEvidenceAssociationV1::decode(
                bytes
                    .get(start..end)
                    .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)?,
            )?);
            offset = end;
        }
        let terminal_end = offset
            .checked_add(MULTI_ROOT_ASSOCIATION_TERMINAL_BYTES_V1)
            .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
        if terminal_end != bytes.len() {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TrailingBytes);
        }
        let terminal: [u8; 32] = bytes[offset..terminal_end]
            .try_into()
            .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)?;
        if terminal != derive_identity(MULTI_ROOT_ASSOCIATION_IDENTITY_DOMAIN_V1, &bytes[..offset])
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::IdentityMismatch);
        }
        let decoded = Self::new(entries)?;
        if decoded.canonical_bytes() != bytes {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the exact canonical roster bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated exact roster identity.
    pub const fn identity(&self) -> InertStaticCapabilityEvidenceAssociationIdentityV1 {
        self.identity
    }

    /// Returns every root association in canonical descriptor order.
    pub fn entries(&self) -> &[InertStaticCapabilityEvidenceAssociationV1] {
        &self.entries
    }

    /// Returns the complete ordered subject roster.
    pub fn subjects(&self) -> impl ExactSizeIterator<Item = CapabilitySubjectV1> + '_ {
        self.entries.iter().map(|entry| entry.subject())
    }
}

fn validate_multi_root_associations_v1(
    entries: &[InertStaticCapabilityEvidenceAssociationV1],
) -> Result<(), InertStaticCapabilityEvidenceAssociationErrorV1> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&entries.len()) {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidRosterCount);
    }
    let first = &entries[0];
    let first_subject = first.subject();
    let mut previous_kernel = None;
    let mut roots = std::collections::BTreeSet::new();
    for entry in entries {
        let subject = entry.subject();
        let kernel = *subject.kernel().digest().as_bytes();
        if previous_kernel.is_some_and(|previous| kernel <= previous) {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::NonCanonicalRosterOrder);
        }
        previous_kernel = Some(kernel);
        if !roots.insert(*subject.root().digest().as_bytes())
            || entry.inputs() != first.inputs()
            || subject.executable_kir() != first_subject.executable_kir()
            || subject.executable_kir_epoch() != first_subject.executable_kir_epoch()
            || subject.target_model() != first_subject.target_model()
        {
            return Err(InertStaticCapabilityEvidenceAssociationErrorV1::RosterSubjectMismatch);
        }
    }
    Ok(())
}

fn encode_lineage_identity(
    identity: InertLineageContentIdentityV3,
) -> [u8; LINEAGE_IDENTITY_BYTES_V1] {
    let mut bytes = [0; LINEAGE_IDENTITY_BYTES_V1];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn decode_lineage_identity(
    bytes: &[u8],
) -> Result<InertLineageContentIdentityV3, InertStaticCapabilityEvidenceAssociationErrorV1> {
    if bytes.len() != LINEAGE_IDENTITY_BYTES_V1 {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidFieldLength);
    }
    let mut digest = [0; 32];
    digest.copy_from_slice(&bytes[..32]);
    let mut length = [0; 8];
    length.copy_from_slice(&bytes[32..]);
    InertLineageContentIdentityV3::new(digest, u64::from_le_bytes(length))
        .map_err(|_| InertStaticCapabilityEvidenceAssociationErrorV1::InvalidIdentity)
}

fn encode_optional_refinement_identity(
    identity: Option<InertCapabilityRefinementReceiptIdentityV1>,
) -> Vec<u8> {
    identity.map_or_else(Vec::new, |identity| {
        let mut bytes = Vec::with_capacity(LINEAGE_IDENTITY_BYTES_V1);
        bytes.extend_from_slice(&identity.sha256);
        bytes.extend_from_slice(&identity.byte_len.to_le_bytes());
        bytes
    })
}

fn decode_optional_refinement_identity(
    bytes: &[u8],
) -> Result<
    Option<InertCapabilityRefinementReceiptIdentityV1>,
    InertStaticCapabilityEvidenceAssociationErrorV1,
> {
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.len() != LINEAGE_IDENTITY_BYTES_V1 {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidFieldLength);
    }
    let mut digest = [0; 32];
    digest.copy_from_slice(&bytes[..32]);
    let mut length = [0; 8];
    length.copy_from_slice(&bytes[32..]);
    InertCapabilityRefinementReceiptIdentityV1::new(digest, u64::from_le_bytes(length)).map(Some)
}

fn encode_subject(subject: CapabilitySubjectV1) -> [u8; SUBJECT_BYTES_V1] {
    let mut bytes = [0; SUBJECT_BYTES_V1];
    let fields = [
        subject.kernel().digest(),
        subject.root().digest(),
        subject.executable_kir().digest(),
        subject.target_model().digest(),
        subject.launch_contract().digest(),
    ];
    for (index, field) in fields.iter().enumerate() {
        bytes[index * 32..(index + 1) * 32].copy_from_slice(field.as_bytes());
    }
    bytes[160..].copy_from_slice(&subject.executable_kir_epoch().to_le_bytes());
    bytes
}

fn decode_subject(
    bytes: &[u8],
) -> Result<CapabilitySubjectV1, InertStaticCapabilityEvidenceAssociationErrorV1> {
    if bytes.len() != SUBJECT_BYTES_V1 {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidFieldLength);
    }
    let digest = |offset: usize| {
        let mut value = [0; 32];
        value.copy_from_slice(&bytes[offset..offset + 32]);
        DigestV1::from_untrusted_bytes(value)
    };
    let mut epoch = [0; 8];
    epoch.copy_from_slice(&bytes[160..168]);
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(digest(0)),
        KernelRootIdentityV1::from_untrusted_digest(digest(32)),
        ExecutableKirIdentityV1::from_untrusted_digest(digest(64)),
        u64::from_le_bytes(epoch),
        TargetModelIdentityV1::from_untrusted_digest(digest(96)),
        LaunchContractIdentityV1::from_untrusted_digest(digest(128)),
    )
    .map_err(InertStaticCapabilityEvidenceAssociationErrorV1::CapabilityCodec)
}

fn decode_obligation_identity(
    bytes: &[u8],
) -> Result<InertCapabilityObligationSetIdentityV1, InertStaticCapabilityEvidenceAssociationErrorV1>
{
    decode_set_digest(bytes).map(InertCapabilityObligationSetIdentityV1::from_untrusted_digest)
}

fn decode_result_identity(
    bytes: &[u8],
) -> Result<InertCapabilityResultSetIdentityV1, InertStaticCapabilityEvidenceAssociationErrorV1> {
    decode_set_digest(bytes).map(InertCapabilityResultSetIdentityV1::from_untrusted_digest)
}

fn decode_set_digest(
    bytes: &[u8],
) -> Result<DigestV1, InertStaticCapabilityEvidenceAssociationErrorV1> {
    if bytes.len() != SET_IDENTITY_BYTES_V1 {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidFieldLength);
    }
    let mut digest = [0; 32];
    digest.copy_from_slice(bytes);
    if digest == [0; 32] {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::InvalidIdentity);
    }
    Ok(DigestV1::from_untrusted_bytes(digest))
}

fn encoded_len(fields: &[&[u8]]) -> Result<usize, InertStaticCapabilityEvidenceAssociationErrorV1> {
    let payload = fields
        .iter()
        .try_fold(0_usize, |total, field| total.checked_add(field.len()))
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
    let total = HEADER_BYTES_V1
        .checked_add(
            fields
                .len()
                .checked_mul(FIELD_HEADER_BYTES_V1)
                .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?,
        )
        .and_then(|value| value.checked_add(payload))
        .and_then(|value| value.checked_add(TERMINAL_IDENTITY_BYTES_V1))
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::LengthOverflow)?;
    if total > MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1 {
        return Err(InertStaticCapabilityEvidenceAssociationErrorV1::TooLarge);
    }
    Ok(total)
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn read_u16(
    bytes: &[u8],
    offset: usize,
) -> Result<u16, InertStaticCapabilityEvidenceAssociationErrorV1> {
    bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
) -> Result<u32, InertStaticCapabilityEvidenceAssociationErrorV1> {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(InertStaticCapabilityEvidenceAssociationErrorV1::Truncated)
}

/// Strict construction and decoding failures for the V1 static association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InertStaticCapabilityEvidenceAssociationErrorV1 {
    /// The association exceeds its hard bound.
    TooLarge,
    /// Exact allocation failed.
    AllocationFailed,
    /// A size computation overflowed.
    LengthOverflow,
    /// Required bytes are absent.
    Truncated,
    /// The magic is not exact V1.
    InvalidMagic,
    /// The version is not exact V1.
    UnsupportedVersion,
    /// The record kind differs.
    WrongRecordKind,
    /// The association-only policy differs.
    WrongPolicy,
    /// The ordered field count differs.
    WrongFieldCount,
    /// The declared length differs from the input length.
    DeclaredLengthMismatch,
    /// A reserved header value is nonzero.
    NonzeroReserved,
    /// An ordered field tag differs.
    WrongFieldTag,
    /// Field flags are nonzero.
    NonzeroFieldFlags,
    /// A fixed-width or optional field has the wrong length.
    InvalidFieldLength,
    /// A required identity is zero or has zero length.
    InvalidIdentity,
    /// The stable identity domain differs.
    WrongDomain,
    /// The exact inert claim differs.
    WrongClaim,
    /// Bytes remain outside the canonical record.
    TrailingBytes,
    /// The terminal association identity differs.
    IdentityMismatch,
    /// Nested set identities differ from their explicit coordinates.
    SetIdentityMismatch,
    /// Re-encoding did not reproduce the exact bytes.
    NonCanonical,
    /// A nested capability set is malformed or noncanonical.
    CapabilityCodec(CapabilityCodecErrorV1),
    /// The obligation and result sets do not compose one-to-one.
    Composition(CapabilityCompositionErrorV1),
    /// An opaque refinement receipt is empty.
    EmptyRefinementReceipt,
    /// An opaque refinement receipt exceeds its independent hard bound.
    RefinementReceiptTooLarge,
    /// Source-refinement bytes are not the exact checked relation and final-lineage join.
    InvalidSourceRefinementReceipt,
    /// Machine-refinement bytes are not the exact canonical target-specific #214 schema.
    InvalidMachineRefinementReceipt,
    /// The checked source relation names a different source, graph, epoch, or root roster.
    RefinementSubjectMismatch,
    /// A source-only validation API received a machine-refinement receipt or vice versa.
    WrongRefinementKind,
    /// A multi-root evidence roster is empty or exceeds the fixed root bound.
    InvalidRosterCount,
    /// Multi-root evidence is not in unique increasing kernel-identity order.
    NonCanonicalRosterOrder,
    /// Multi-root entries differ in lineage, graph, epoch, target, or root identity.
    RosterSubjectMismatch,
}

impl fmt::Display for InertStaticCapabilityEvidenceAssociationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityCodec(error) => {
                write!(formatter, "invalid nested capability set: {error:?}")
            }
            Self::Composition(error) => {
                write!(formatter, "capability sets do not compose: {error:?}")
            }
            other => formatter.write_str(match other {
                Self::TooLarge => "static capability association exceeds its hard limit",
                Self::AllocationFailed => "static capability association allocation failed",
                Self::LengthOverflow => "static capability association length overflowed",
                Self::Truncated => "static capability association is truncated",
                Self::InvalidMagic => "static capability association magic mismatch",
                Self::UnsupportedVersion => "unsupported static capability association version",
                Self::WrongRecordKind => "static capability association record kind mismatch",
                Self::WrongPolicy => "static capability association policy mismatch",
                Self::WrongFieldCount => "static capability association field count mismatch",
                Self::DeclaredLengthMismatch => {
                    "static capability association declared length mismatch"
                }
                Self::NonzeroReserved => "static capability association reserved field is nonzero",
                Self::WrongFieldTag => "static capability association field tag mismatch",
                Self::NonzeroFieldFlags => "static capability association field flags are nonzero",
                Self::InvalidFieldLength => "static capability association field length is invalid",
                Self::InvalidIdentity => "static capability association identity is invalid",
                Self::WrongDomain => "static capability association domain mismatch",
                Self::WrongClaim => "static capability association claim mismatch",
                Self::TrailingBytes => "static capability association has trailing bytes",
                Self::IdentityMismatch => "static capability association identity mismatch",
                Self::SetIdentityMismatch => "static capability set identity mismatch",
                Self::NonCanonical => "static capability association encoding is noncanonical",
                Self::EmptyRefinementReceipt => "capability refinement receipt is empty",
                Self::RefinementReceiptTooLarge => {
                    "capability refinement receipt exceeds its hard limit"
                }
                Self::InvalidSourceRefinementReceipt => {
                    "source refinement receipt is not a checked canonical relation"
                }
                Self::InvalidMachineRefinementReceipt => {
                    "machine refinement receipt is not canonical typed target evidence"
                }
                Self::RefinementSubjectMismatch => {
                    "source refinement receipt subject roster mismatch"
                }
                Self::WrongRefinementKind => "capability refinement receipt kind mismatch",
                Self::InvalidRosterCount => "capability evidence roster count is invalid",
                Self::NonCanonicalRosterOrder => "capability evidence roster order is noncanonical",
                Self::RosterSubjectMismatch => "capability evidence roster subjects differ",
                Self::CapabilityCodec(_) | Self::Composition(_) => unreachable!(),
            }),
        }
    }
}

impl Error for InertStaticCapabilityEvidenceAssociationErrorV1 {}
