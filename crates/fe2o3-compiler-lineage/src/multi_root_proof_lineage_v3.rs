use std::{error::Error, fmt};

use sha2::{Digest as _, Sha256};

use crate::{
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3, MultiRootNeutralKirIdentityV3,
    MultiRootProofRosterErrorV3, MultiRootProofRosterKindV3, MultiRootProofRosterTranscriptV3,
};

/// Exact wire version of the native KIR V13 multi-root proof lineage.
pub const MULTI_ROOT_PROOF_LINEAGE_VERSION_V3: u16 = 3;
/// Maximum bytes retained by one aggregate V13 multi-root lineage record.
pub const MAX_MULTI_ROOT_PROOF_LINEAGE_BYTES_V3: usize =
    4 * MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 128;

const MAGIC_V3: [u8; 8] = *b"F2MRLNV3";
const POLICY_V3: u16 = 1;
const HEADER_BYTES_V3: usize = 20;
const FIELD_HEADER_BYTES_V3: usize = 8;
const FIELD_COUNT_V3: usize = 4;
const TERMINAL_BYTES_V3: usize = 32;
const IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/MULTI-ROOT-PROOF-LINEAGE/EXACT-KIR-V13/V3\0";

/// Domain-separated identity of one complete native V13 multi-root lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertMultiRootProofLineageIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertMultiRootProofLineageIdentityV3 {
    pub(crate) fn from_exact_identity_v3(sha256: [u8; 32], byte_len: u64) -> Option<Self> {
        if sha256 == [0; 32] || byte_len == 0 {
            None
        } else {
            Some(Self { sha256, byte_len })
        }
    }

    /// Returns the exact lineage digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the complete canonical lineage length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only aggregate of the four exact V13 proof-roster transcripts.
#[derive(Debug, Eq, PartialEq)]
pub struct InertMultiRootProofLineageV3 {
    canonical_bytes: Box<[u8]>,
    identity: InertMultiRootProofLineageIdentityV3,
    middle_end: MultiRootProofRosterTranscriptV3,
    correspondence: MultiRootProofRosterTranscriptV3,
    formal_memory: MultiRootProofRosterTranscriptV3,
    verus_execution: MultiRootProofRosterTranscriptV3,
}

impl InertMultiRootProofLineageV3 {
    /// Joins four exact, mutually consistent native V13 proof rosters.
    pub fn new(
        middle_end: MultiRootProofRosterTranscriptV3,
        correspondence: MultiRootProofRosterTranscriptV3,
        formal_memory: MultiRootProofRosterTranscriptV3,
        verus_execution: MultiRootProofRosterTranscriptV3,
    ) -> Result<Self, MultiRootProofLineageErrorV3> {
        let rosters = [
            &middle_end,
            &correspondence,
            &formal_memory,
            &verus_execution,
        ];
        validate_rosters(&rosters)?;
        let canonical_bytes = encode_rosters(&rosters)?;
        let identity = identity(&canonical_bytes)?;
        Ok(Self {
            canonical_bytes,
            identity,
            middle_end,
            correspondence,
            formal_memory,
            verus_execution,
        })
    }

    /// Strictly decodes V3 bytes with no V2 fallback or projection.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiRootProofLineageErrorV3> {
        if bytes.len() > MAX_MULTI_ROOT_PROOF_LINEAGE_BYTES_V3 {
            return Err(MultiRootProofLineageErrorV3::TooLarge);
        }
        if bytes.len()
            < HEADER_BYTES_V3 + FIELD_COUNT_V3 * FIELD_HEADER_BYTES_V3 + TERMINAL_BYTES_V3
        {
            return Err(MultiRootProofLineageErrorV3::Truncated);
        }
        if bytes[..8] != MAGIC_V3 {
            return Err(MultiRootProofLineageErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != MULTI_ROOT_PROOF_LINEAGE_VERSION_V3 {
            return Err(MultiRootProofLineageErrorV3::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != POLICY_V3 {
            return Err(MultiRootProofLineageErrorV3::WrongPolicy);
        }
        if usize::try_from(read_u32(bytes, 12)?).ok() != Some(bytes.len()) {
            return Err(MultiRootProofLineageErrorV3::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 16)? != 0 {
            return Err(MultiRootProofLineageErrorV3::NonzeroReserved);
        }

        let mut offset = HEADER_BYTES_V3;
        let mut rosters = Vec::new();
        rosters
            .try_reserve_exact(FIELD_COUNT_V3)
            .map_err(|_| MultiRootProofLineageErrorV3::AllocationFailed)?;
        for index in 0..FIELD_COUNT_V3 {
            if read_u16(bytes, offset)? != u16::try_from(index + 1).unwrap() {
                return Err(MultiRootProofLineageErrorV3::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(MultiRootProofLineageErrorV3::NonzeroFieldFlags);
            }
            let length = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| MultiRootProofLineageErrorV3::LengthOverflow)?;
            if length == 0 || length > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
                return Err(MultiRootProofLineageErrorV3::InvalidRosterLength);
            }
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V3)
                .ok_or(MultiRootProofLineageErrorV3::LengthOverflow)?;
            let end = start
                .checked_add(length)
                .ok_or(MultiRootProofLineageErrorV3::LengthOverflow)?;
            let roster = MultiRootProofRosterTranscriptV3::decode(
                bytes
                    .get(start..end)
                    .ok_or(MultiRootProofLineageErrorV3::Truncated)?,
            )
            .map_err(MultiRootProofLineageErrorV3::Roster)?;
            rosters.push(roster);
            offset = end;
        }
        let terminal_end = offset
            .checked_add(TERMINAL_BYTES_V3)
            .ok_or(MultiRootProofLineageErrorV3::LengthOverflow)?;
        if terminal_end != bytes.len() {
            return Err(MultiRootProofLineageErrorV3::TrailingBytes);
        }
        let expected = derive_identity(&bytes[..offset]);
        if bytes[offset..terminal_end] != expected {
            return Err(MultiRootProofLineageErrorV3::IdentityMismatch);
        }
        let mut iter = rosters.into_iter();
        let decoded = Self::new(
            iter.next().expect("fixed roster count"),
            iter.next().expect("fixed roster count"),
            iter.next().expect("fixed roster count"),
            iter.next().expect("fixed roster count"),
        )?;
        if decoded.canonical_bytes() != bytes {
            return Err(MultiRootProofLineageErrorV3::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the exact canonical aggregate bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the native aggregate identity.
    pub const fn identity(&self) -> InertMultiRootProofLineageIdentityV3 {
        self.identity
    }

    /// Returns the exact V13 graph and epoch shared by all roots and rosters.
    pub const fn neutral_kir(&self) -> MultiRootNeutralKirIdentityV3 {
        self.middle_end.neutral_kir()
    }

    /// Returns one exact roster by its required evidence kind.
    pub const fn roster(
        &self,
        kind: MultiRootProofRosterKindV3,
    ) -> &MultiRootProofRosterTranscriptV3 {
        match kind {
            MultiRootProofRosterKindV3::MiddleEnd => &self.middle_end,
            MultiRootProofRosterKindV3::Correspondence => &self.correspondence,
            MultiRootProofRosterKindV3::FormalMemory => &self.formal_memory,
            MultiRootProofRosterKindV3::VerusExecution => &self.verus_execution,
        }
    }
}

fn validate_rosters(
    rosters: &[&MultiRootProofRosterTranscriptV3; FIELD_COUNT_V3],
) -> Result<(), MultiRootProofLineageErrorV3> {
    let kinds = [
        MultiRootProofRosterKindV3::MiddleEnd,
        MultiRootProofRosterKindV3::Correspondence,
        MultiRootProofRosterKindV3::FormalMemory,
        MultiRootProofRosterKindV3::VerusExecution,
    ];
    let first = rosters[0];
    for (roster, expected_kind) in rosters.iter().zip(kinds) {
        if roster.kind() != expected_kind {
            return Err(MultiRootProofLineageErrorV3::WrongRosterKind);
        }
        if roster.semantic_mir_sha256() != first.semantic_mir_sha256()
            || roster.neutral_kir() != first.neutral_kir()
            || roster.roster_identity() != first.roster_identity()
            || roster.canonical_kernel_order() != first.canonical_kernel_order()
            || roster.root_count() != first.root_count()
        {
            return Err(MultiRootProofLineageErrorV3::RosterSubjectMismatch);
        }
        for index in 0..first.root_count() {
            let left = first.root(index).expect("validated root count");
            let right = roster.root(index).expect("validated root count");
            if left.semantic_root() != right.semantic_root()
                || left.semantic_root_identity() != right.semantic_root_identity()
                || left.kernel_binding() != right.kernel_binding()
                || left.source_rank() != right.source_rank()
                || left.workgroup() != right.workgroup()
                || left.logical_name() != right.logical_name()
                || left.export_symbol() != right.export_symbol()
                || left.kernel_id() != right.kernel_id()
            {
                return Err(MultiRootProofLineageErrorV3::RosterRootMismatch { index });
            }
        }
    }
    Ok(())
}

fn encode_rosters(
    rosters: &[&MultiRootProofRosterTranscriptV3; FIELD_COUNT_V3],
) -> Result<Box<[u8]>, MultiRootProofLineageErrorV3> {
    let payload = rosters
        .iter()
        .try_fold(0_usize, |total, roster| {
            total.checked_add(roster.canonical_bytes().len())
        })
        .ok_or(MultiRootProofLineageErrorV3::LengthOverflow)?;
    let total = HEADER_BYTES_V3
        .checked_add(FIELD_COUNT_V3 * FIELD_HEADER_BYTES_V3)
        .and_then(|value| value.checked_add(payload))
        .and_then(|value| value.checked_add(TERMINAL_BYTES_V3))
        .ok_or(MultiRootProofLineageErrorV3::LengthOverflow)?;
    if total > MAX_MULTI_ROOT_PROOF_LINEAGE_BYTES_V3 {
        return Err(MultiRootProofLineageErrorV3::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| MultiRootProofLineageErrorV3::AllocationFailed)?;
    bytes.extend_from_slice(&MAGIC_V3);
    bytes.extend_from_slice(&MULTI_ROOT_PROOF_LINEAGE_VERSION_V3.to_le_bytes());
    bytes.extend_from_slice(&POLICY_V3.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(total)
            .map_err(|_| MultiRootProofLineageErrorV3::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for (index, roster) in rosters.iter().enumerate() {
        bytes.extend_from_slice(&u16::try_from(index + 1).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(roster.canonical_bytes().len())
                .map_err(|_| MultiRootProofLineageErrorV3::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(roster.canonical_bytes());
    }
    let terminal = derive_identity(&bytes);
    bytes.extend_from_slice(&terminal);
    Ok(bytes.into_boxed_slice())
}

fn identity(
    bytes: &[u8],
) -> Result<InertMultiRootProofLineageIdentityV3, MultiRootProofLineageErrorV3> {
    let terminal = bytes
        .get(bytes.len().saturating_sub(TERMINAL_BYTES_V3)..)
        .ok_or(MultiRootProofLineageErrorV3::Truncated)?;
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(terminal);
    Ok(InertMultiRootProofLineageIdentityV3 {
        sha256,
        byte_len: u64::try_from(bytes.len())
            .map_err(|_| MultiRootProofLineageErrorV3::LengthOverflow)?,
    })
}

fn derive_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V3);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, MultiRootProofLineageErrorV3> {
    bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(MultiRootProofLineageErrorV3::Truncated)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, MultiRootProofLineageErrorV3> {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(MultiRootProofLineageErrorV3::Truncated)
}

/// Failure to compose or decode native V13 multi-root lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MultiRootProofLineageErrorV3 {
    /// A bounded allocation failed.
    AllocationFailed,
    /// The aggregate exceeds its fixed byte bound.
    TooLarge,
    /// Canonical length arithmetic overflowed.
    LengthOverflow,
    /// The aggregate is truncated.
    Truncated,
    /// The aggregate does not use the V3 magic.
    InvalidMagic,
    /// The aggregate does not use exact wire version 3.
    UnsupportedVersion,
    /// The aggregate policy is unsupported.
    WrongPolicy,
    /// The declared and physical lengths differ.
    DeclaredLengthMismatch,
    /// A reserved header field is nonzero.
    NonzeroReserved,
    /// A field tag is absent or reordered.
    WrongFieldTag,
    /// A field has unsupported flags.
    NonzeroFieldFlags,
    /// A nested roster length is outside its bound.
    InvalidRosterLength,
    /// Bytes remain after the terminal identity.
    TrailingBytes,
    /// The terminal identity does not match the exact preimage.
    IdentityMismatch,
    /// Reconstruction did not reproduce the exact input.
    NonCanonical,
    /// A nested V13 roster is invalid.
    Roster(MultiRootProofRosterErrorV3),
    /// A nested roster occupies the wrong evidence slot.
    WrongRosterKind,
    /// The four rosters name different graph or source subjects.
    RosterSubjectMismatch,
    /// One root coordinate differs across the four rosters.
    RosterRootMismatch {
        /// Canonical root index that differs.
        index: usize,
    },
}

impl fmt::Display for MultiRootProofLineageErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native V13 multi-root proof lineage: {self:?}"
        )
    }
}

impl Error for MultiRootProofLineageErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Roster(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MultiRootCanonicalKirVersionV3, MultiRootProofRosterInputsV3,
        MultiRootProofRosterRootInputV3,
    };

    fn roster(
        kind: MultiRootProofRosterKindV3,
        epoch: u64,
        second_binding: u8,
    ) -> MultiRootProofRosterTranscriptV3 {
        let roots = [
            MultiRootProofRosterRootInputV3 {
                semantic_root: 1,
                semantic_root_identity: [11; 32],
                kernel_binding: [22; 32],
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "a",
                export_symbol: "a",
                kernel_id: "a",
                payload: b"proof-a",
            },
            MultiRootProofRosterRootInputV3 {
                semantic_root: 2,
                semantic_root_identity: [12; 32],
                kernel_binding: [second_binding; 32],
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "b",
                export_symbol: "b",
                kernel_id: "b",
                payload: b"proof-b",
            },
        ];
        let mut order = [0_u32, 1];
        order.sort_unstable_by_key(|index| {
            fe2o3_kernel_descriptor::KernelId::from_bytes(
                roots[usize::try_from(*index).unwrap()].kernel_binding,
            )
        });
        MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: [33; 32],
            neutral_kir: MultiRootNeutralKirIdentityV3::new(
                MultiRootCanonicalKirVersionV3::V13,
                4096,
                [44; 32],
                epoch,
            )
            .unwrap(),
            roster_identity: [55; 32],
            canonical_kernel_order: &order,
            roots: &roots,
        })
        .unwrap()
    }

    fn lineage(epoch: u64) -> InertMultiRootProofLineageV3 {
        InertMultiRootProofLineageV3::new(
            roster(MultiRootProofRosterKindV3::MiddleEnd, epoch, 21),
            roster(MultiRootProofRosterKindV3::Correspondence, epoch, 21),
            roster(MultiRootProofRosterKindV3::FormalMemory, epoch, 21),
            roster(MultiRootProofRosterKindV3::VerusExecution, epoch, 21),
        )
        .unwrap()
    }

    #[test]
    fn exact_v13_lineage_round_trips() {
        let lineage = lineage(7);
        let decoded = InertMultiRootProofLineageV3::decode(lineage.canonical_bytes()).unwrap();
        assert_eq!(decoded, lineage);
        assert_eq!(decoded.neutral_kir().graph_epoch(), 7);
        assert_eq!(decoded.neutral_kir().version().wire_version(), 13);
        assert_eq!(
            decoded.identity().byte_len(),
            u64::try_from(decoded.canonical_bytes().len()).unwrap()
        );
    }

    #[test]
    fn downgrade_omission_and_stale_epoch_fail_closed() {
        let lineage = lineage(7);
        let mut downgrade = lineage.canonical_bytes().to_vec();
        downgrade[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            InertMultiRootProofLineageV3::decode(&downgrade).unwrap_err(),
            MultiRootProofLineageErrorV3::UnsupportedVersion
        );
        assert!(
            InertMultiRootProofLineageV3::decode(
                &lineage.canonical_bytes()[..lineage.canonical_bytes().len() - 1]
            )
            .is_err()
        );
        assert_eq!(
            InertMultiRootProofLineageV3::new(
                roster(MultiRootProofRosterKindV3::MiddleEnd, 7, 21),
                roster(MultiRootProofRosterKindV3::Correspondence, 8, 21),
                roster(MultiRootProofRosterKindV3::FormalMemory, 7, 21),
                roster(MultiRootProofRosterKindV3::VerusExecution, 7, 21),
            )
            .unwrap_err(),
            MultiRootProofLineageErrorV3::RosterSubjectMismatch
        );
    }

    #[test]
    fn cross_kernel_and_roster_substitution_fail_closed() {
        assert_eq!(
            InertMultiRootProofLineageV3::new(
                roster(MultiRootProofRosterKindV3::MiddleEnd, 7, 21),
                roster(MultiRootProofRosterKindV3::Correspondence, 7, 20),
                roster(MultiRootProofRosterKindV3::FormalMemory, 7, 21),
                roster(MultiRootProofRosterKindV3::VerusExecution, 7, 21),
            )
            .unwrap_err(),
            MultiRootProofLineageErrorV3::RosterRootMismatch { index: 1 }
        );
        assert_eq!(
            InertMultiRootProofLineageV3::new(
                roster(MultiRootProofRosterKindV3::Correspondence, 7, 21),
                roster(MultiRootProofRosterKindV3::MiddleEnd, 7, 21),
                roster(MultiRootProofRosterKindV3::FormalMemory, 7, 21),
                roster(MultiRootProofRosterKindV3::VerusExecution, 7, 21),
            )
            .unwrap_err(),
            MultiRootProofLineageErrorV3::WrongRosterKind
        );
    }
}
