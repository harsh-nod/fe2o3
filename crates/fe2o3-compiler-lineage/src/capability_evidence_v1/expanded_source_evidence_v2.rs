//! Versioned inert source envelope; semantic admission still requires live V6 replay.

use super::*;
use crate::{MultiRootCorrespondencePayloadV3, MultiRootInductionKindV3};

/// Source envelope carrying shared V6 correspondence, never frozen V5 bytes.
pub const EXPANDED_SOURCE_EVIDENCE_MAGIC_V2: [u8; 8] = *b"F2SREFV2";
const DOMAIN: &[u8] = b"FE2O3/EXPANDED-SOURCE-MIR-TO-KIR-EVIDENCE/V2\0";
const CORRESPONDENCE_DOMAIN: &[u8] = b"FE2O3/COMPOSED-EXECUTION-MIR-TO-KIR-CORRESPONDENCE/V6\0";
const EXPANSION_DOMAIN: &[u8] = b"FE2O3/INERT-CALL-EXPANSION-EVIDENCE/V1\0";
const HEADER: usize = 168;
const ROOT_PREFIX: usize = crate::MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3 + 36;
const MAX_BYTES: usize = 4 * 1024 * 1024;
pub(super) const JOIN_MAGIC: [u8; 8] = *b"F2CSRCV2";
const JOIN_DOMAIN: &[u8] = b"FE2O3/CAPABILITY/CHECKED-SOURCE-REFINEMENT-TO-FINAL-LINEAGE/V2\0";
type Result<T> = std::result::Result<T, InertStaticCapabilityEvidenceAssociationErrorV1>;
use InertStaticCapabilityEvidenceAssociationErrorV1 as E;

fn invalid() -> E {
    E::InvalidSourceRefinementReceipt
}

impl InertCapabilityRefinementReceiptV1 {
    /// Associates complete expanded-source evidence with exact final V13 lineage.
    /// This checks inert schema and custody, not the issuer: production must first replay V6
    /// against its live compiler owner. Final graph equivalence is a separate mandatory proof.
    pub fn from_checked_source_evidence_v2(
        source_evidence: &[u8],
        final_lineage: &InertMultiRootProofLineageV3,
    ) -> Result<Self> {
        let source = InertExpandedSourceEvidenceV2::decode(source_evidence)?;
        source.validate_against_lineage(final_lineage)?;
        let total = SOURCE_REFINEMENT_JOIN_HEADER_BYTES_V1
            .checked_add(source_evidence.len())
            .and_then(|n| n.checked_add(32))
            .ok_or(E::LengthOverflow)?;
        if total > MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1 {
            return Err(E::RefinementReceiptTooLarge);
        }
        let lineage = final_lineage.identity();
        let final_kir = final_lineage.neutral_kir();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| E::AllocationFailed)?;
        bytes.extend_from_slice(&JOIN_MAGIC);
        bytes.extend_from_slice(&[2, 0, 1, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&(source_evidence.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&lineage.sha256());
        bytes.extend_from_slice(&lineage.byte_len().to_le_bytes());
        bytes.extend_from_slice(&source.roots[0].coordinates.inputs().semantic_mir_sha256);
        bytes.extend_from_slice(&final_kir.digest());
        bytes.extend_from_slice(&final_kir.canonical_length().to_le_bytes());
        bytes.extend_from_slice(&final_kir.graph_epoch().to_le_bytes());
        bytes.extend_from_slice(&(source.roots.len() as u32).to_le_bytes());
        bytes.extend_from_slice(source_evidence);
        let terminal = derive_identity(JOIN_DOMAIN, &bytes);
        bytes.extend_from_slice(&terminal);
        Self::from_canonical_preimage(
            InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            bytes,
        )
    }
}

pub(super) fn decode_join(
    bytes: &[u8],
) -> Result<(
    ParsedSourceRefinementJoinV1<'_>,
    InertExpandedSourceEvidenceV2,
)> {
    let join = ParsedSourceRefinementJoinV1::decode_frame(bytes, JOIN_MAGIC, 2, JOIN_DOMAIN)?;
    let source = InertExpandedSourceEvidenceV2::decode(join.source_evidence)?;
    if join.semantic_mir_sha256 != source.roots[0].coordinates.inputs().semantic_mir_sha256
        || join.root_count != source.roots.len()
    {
        return Err(invalid());
    }
    Ok((join, source))
}

pub(super) fn validate_receipt(
    bytes: &[u8],
    lineage: &InertMultiRootProofLineageV3,
    subjects: &[CapabilitySubjectV1],
) -> Result<()> {
    let (join, source) = decode_join(bytes)?;
    source.validate_against_lineage(lineage)?;
    join.validate_subjects(
        lineage,
        subjects,
        source.roots[0].coordinates.inputs().semantic_mir_sha256,
        source.roots.len(),
    )
}

/// Exact root reference plus its physical binding and entry symbol. Still inert.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSourceRootV2 {
    coordinates: MultiRootCorrespondencePayloadV3,
    kernel_binding: [u8; 32],
    export_symbol: Box<str>,
}
impl ExpandedSourceRootV2 {
    /// Retains inert metadata; only the compiler's live replay may establish its truth.
    pub fn new(
        coordinates: MultiRootCorrespondencePayloadV3,
        kernel_binding: [u8; 32],
        export_symbol: &str,
    ) -> Result<Self> {
        if kernel_binding == [0; 32]
            || export_symbol.is_empty()
            || export_symbol.len() > 4096
            || export_symbol.contains('\0')
        {
            return Err(invalid());
        }
        Ok(Self {
            coordinates,
            kernel_binding,
            export_symbol: export_symbol.into(),
        })
    }
    /// Exact original and execution coordinates for this root.
    pub const fn coordinates(&self) -> &MultiRootCorrespondencePayloadV3 {
        &self.coordinates
    }
    /// Original compiler-issued physical kernel binding.
    pub const fn kernel_binding(&self) -> &[u8; 32] {
        &self.kernel_binding
    }
    /// Physical export and exact KIR entry symbol.
    pub fn export_symbol(&self) -> &str {
        &self.export_symbol
    }
}

/// Canonical bytes only. Neither construction nor decoding authenticates an IR producer.
/// Full V6 source/expansion/KIR replay is mandatory before production acceptance.
#[derive(Debug, Eq, PartialEq)]
pub struct InertExpandedSourceEvidenceV2 {
    bytes: Box<[u8]>,
    identity: [u8; 32],
    source_kir_sha256: [u8; 32],
    source_kir_bytes: u64,
    roots: Box<[ExpandedSourceRootV2]>,
    correspondence_offset: usize,
}
impl InertExpandedSourceEvidenceV2 {
    /// Encodes associations to complete V6 bytes, without issuing a live proof result.
    pub fn new(
        source_kir_sha256: [u8; 32],
        source_kir_bytes: u64,
        roots: Vec<ExpandedSourceRootV2>,
        correspondence: &[u8],
    ) -> Result<Self> {
        if source_kir_sha256 == [0; 32]
            || source_kir_bytes == 0
            || roots.is_empty()
            || roots.len() > MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
            || correspondence.len() > MAX_BYTES
        {
            return Err(invalid());
        }
        let first = *roots[0].coordinates.inputs();
        let mut prior = None;
        let mut bindings = std::collections::BTreeSet::new();
        let mut functions = std::collections::BTreeSet::new();
        let mut identities = std::collections::BTreeSet::new();
        let mut symbols = std::collections::BTreeSet::new();
        let mut total = HEADER
            .checked_add(correspondence.len())
            .and_then(|n| n.checked_add(32))
            .ok_or(E::LengthOverflow)?;
        for (ordinal, root) in roots.iter().enumerate() {
            let i = root.coordinates.inputs();
            if i.root_ordinal as usize != ordinal
                || prior.is_some_and(|p| p >= i.semantic_root)
                || i.semantic_mir_sha256 != first.semantic_mir_sha256
                || i.correspondence_identity != first.correspondence_identity
                || i.expansion_evidence_identity != first.expansion_evidence_identity
                || !bindings.insert(root.kernel_binding)
                || !functions.insert(i.kernel_function_ordinal)
                || !identities.insert(i.semantic_root_identity)
                || !symbols.insert(root.export_symbol())
            {
                return Err(invalid());
            }
            prior = Some(i.semantic_root);
            total = total
                .checked_add(ROOT_PREFIX)
                .and_then(|n| n.checked_add(root.export_symbol.len()))
                .ok_or(E::LengthOverflow)?;
        }
        if total > MAX_BYTES {
            return Err(E::RefinementReceiptTooLarge);
        }
        validate_v6_bindings(correspondence, source_kir_sha256, source_kir_bytes, &roots)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| E::AllocationFailed)?;
        bytes.extend_from_slice(&EXPANDED_SOURCE_EVIDENCE_MAGIC_V2);
        bytes.extend_from_slice(&[2, 0, 1, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&[13, 0, 0, 0]);
        bytes.extend_from_slice(&source_kir_bytes.to_le_bytes());
        bytes.extend_from_slice(&source_kir_sha256);
        bytes.extend_from_slice(&first.semantic_mir_sha256);
        bytes.extend_from_slice(&first.correspondence_identity);
        bytes.extend_from_slice(&first.expansion_evidence_identity);
        bytes.extend_from_slice(&(roots.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(correspondence.len() as u32).to_le_bytes());
        for root in &roots {
            bytes.extend_from_slice(root.coordinates.canonical_bytes());
            bytes.extend_from_slice(&root.kernel_binding);
            bytes.extend_from_slice(&(root.export_symbol.len() as u32).to_le_bytes());
            bytes.extend_from_slice(root.export_symbol.as_bytes());
        }
        let correspondence_offset = bytes.len();
        bytes.extend_from_slice(correspondence);
        let identity = derive_identity(DOMAIN, &bytes);
        bytes.extend_from_slice(&identity);
        Ok(Self {
            bytes: bytes.into_boxed_slice(),
            identity,
            source_kir_sha256,
            source_kir_bytes,
            roots: roots.into_boxed_slice(),
            correspondence_offset,
        })
    }

    /// Checks canonical encoding and nested identity bindings, not semantic equivalence.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER + ROOT_PREFIX + 32 || bytes.len() > MAX_BYTES {
            return Err(invalid());
        }
        let mut r = SourceEvidenceReaderV1::new(bytes);
        if r.fixed::<8>()? != EXPANDED_SOURCE_EVIDENCE_MAGIC_V2
            || r.u16()? != 2
            || r.u16()? != 1
            || r.u32()? != 0
            || r.usize_u32()? != bytes.len()
            || r.u16()? != 13
            || r.u16()? != 0
        {
            return Err(invalid());
        }
        let kir_bytes = r.u64()?;
        let kir_sha = r.fixed()?;
        let semantic = r.fixed::<32>()?;
        let correspondence_identity = r.fixed::<32>()?;
        let expansion_identity = r.fixed::<32>()?;
        let count = r.usize_u32()?;
        let length = r.usize_u32()?;
        if count == 0
            || count > MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
            || count
                .checked_mul(ROOT_PREFIX + 1)
                .and_then(|n| n.checked_add(length))
                .and_then(|n| n.checked_add(32))
                .is_none_or(|n| n > r.remaining())
        {
            return Err(invalid());
        }
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(count)
            .map_err(|_| E::AllocationFailed)?;
        for _ in 0..count {
            let coordinates = MultiRootCorrespondencePayloadV3::decode(
                r.take(crate::MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3)?,
            )
            .map_err(|_| invalid())?;
            let i = coordinates.inputs();
            if i.semantic_mir_sha256 != semantic
                || i.correspondence_identity != correspondence_identity
                || i.expansion_evidence_identity != expansion_identity
            {
                return Err(invalid());
            }
            let binding = r.fixed()?;
            let name = r.text(4096)?;
            roots.push(ExpandedSourceRootV2::new(coordinates, binding, &name)?);
        }
        let correspondence = r.take(length)?;
        let terminal = r.fixed::<32>()?;
        r.finish()?;
        let value = Self::new(kir_sha, kir_bytes, roots, correspondence)?;
        if terminal != value.identity || value.canonical_bytes() != bytes {
            return Err(invalid());
        }
        Ok(value)
    }

    /// Complete preimage, including the shared V6 relation exactly once.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Domain-separated source-envelope identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Exact canonical source KIR digest, distinct from optimized final KIR.
    pub const fn source_kir_sha256(&self) -> &[u8; 32] {
        &self.source_kir_sha256
    }
    /// Exact canonical source KIR byte length.
    pub const fn source_kir_bytes(&self) -> u64 {
        self.source_kir_bytes
    }
    /// Complete source-ordered root references.
    pub fn roots(&self) -> &[ExpandedSourceRootV2] {
        &self.roots
    }
    /// The full V6 preimage requiring production live replay.
    pub fn correspondence(&self) -> &[u8] {
        &self.bytes[self.correspondence_offset..self.bytes.len() - 32]
    }
    /// This codec never grants compiler-transform, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Joins every root field and exact per-root payload to the final V13 roster.
    pub fn validate_against_lineage(&self, lineage: &InertMultiRootProofLineageV3) -> Result<()> {
        let roster = lineage.roster(MultiRootProofRosterKindV3::Correspondence);
        if roster.semantic_mir_sha256() != self.roots[0].coordinates.inputs().semantic_mir_sha256
            || roster.root_count() != self.roots.len()
        {
            return Err(E::RefinementSubjectMismatch);
        }
        for (ordinal, expected) in self.roots.iter().enumerate() {
            let actual = roster.root(ordinal).ok_or(E::RefinementSubjectMismatch)?;
            let i = expected.coordinates.inputs();
            if actual.semantic_root() != i.semantic_root
                || actual.semantic_root_identity() != i.semantic_root_identity
                || actual.kernel_binding() != expected.kernel_binding
                || actual.export_symbol() != expected.export_symbol()
                || actual.kernel_id() != expected.export_symbol()
                || actual.payload() != expected.coordinates.canonical_bytes()
            {
                return Err(E::RefinementSubjectMismatch);
            }
        }
        Ok(())
    }
}

// Check bindings using the closed V6 frame layout, not an alternative lowering
// or expression evaluator. The owning lowerer performs the full logical replay.
fn validate_v6_bindings(
    bytes: &[u8],
    kir_sha: [u8; 32],
    kir_bytes: u64,
    roots: &[ExpandedSourceRootV2],
) -> Result<()> {
    let first = roots[0].coordinates.inputs();
    if derive_identity(CORRESPONDENCE_DOMAIN, bytes) != first.correspondence_identity {
        return Err(invalid());
    }
    let mut r = SourceEvidenceReaderV1::new(bytes);
    if r.fixed::<8>()? != *b"F2M2K6\0\0"
        || r.u16()? != 6
        || r.u16()? != 1
        || r.u32()? != 0
        || r.usize_u32()? != bytes.len()
        || r.fixed::<32>()? == [0; 32]
        || r.u32()? != 13
        || r.u64()? != kir_bytes
        || r.fixed::<32>()? != kir_sha
    {
        return Err(invalid());
    }
    r.take(32)?; // Lowering limits remain part of the exact V6 preimage.
    let expansion = blob(&mut r)?;
    let mut hash = Sha256::new();
    hash.update(EXPANSION_DOMAIN);
    hash.update(expansion);
    if <[u8; 32]>::from(hash.finalize()) != first.expansion_evidence_identity
        || expansion.len() < 152
        || expansion.get(..8) != Some(b"F2SCEX1\0")
        || expansion.get(8..16) != Some([1, 0, 1, 0, 0, 0, 0, 0].as_slice())
        || read_u32(expansion, 16)? as usize != expansion.len()
        || expansion.get(20..52) != Some(first.semantic_mir_sha256.as_slice())
    {
        return Err(invalid());
    }
    if r.usize_u32()? != roots.len() {
        return Err(invalid());
    }
    for expected in roots {
        let i = expected.coordinates.inputs();
        if r.u32()? != i.semantic_root
            || r.u32()? != i.selected_body
            || r.fixed::<32>()? != i.execution_view_identity
            || r.u32()? != i.induction_kind as u32
        {
            return Err(invalid());
        }
        let induction = blob(&mut r)?;
        let mut reader = SourceEvidenceReaderV1::new(induction);
        let (version, domain) = match i.induction_kind {
            MultiRootInductionKindV3::OriginalV1 => {
                (1, b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V1\0".as_slice())
            }
            MultiRootInductionKindV3::ExpandedV2 => {
                (2, b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V2\0".as_slice())
            }
        };
        if derive_identity(domain, induction) != i.induction_identity
            || reader.fixed::<8>()? != *b"F2U32I\0\0"
            || reader.u16()? != version
            || reader.u16()? != 1
            || reader.u32()? != 0
            || reader.usize_u32()? != induction.len()
            || reader.fixed::<32>()? != i.semantic_mir_sha256
        {
            return Err(invalid());
        }
        if i.induction_kind == MultiRootInductionKindV3::ExpandedV2
            && (reader.fixed::<32>()? != i.expansion_evidence_identity
                || reader.fixed::<32>()?.as_slice() != &expansion[52..84]
                || reader.u32()? != i.semantic_root
                || reader.fixed::<32>()? != i.execution_view_identity)
        {
            return Err(invalid());
        }
        if reader.u32()? != i.selected_body
            || reader.fixed::<32>()? != i.execution_function_identity
        {
            return Err(invalid());
        }
    }
    if r.usize_u32()? != roots.len() {
        return Err(invalid());
    }
    for expected in roots {
        let i = expected.coordinates.inputs();
        if r.u32()? != i.semantic_root
            || r.u32()? != i.selected_body
            || r.u32()? != i.kernel_function_ordinal
            || r.u32()? != 1
            || blob(&mut r)? != expected.export_symbol.as_bytes()
        {
            return Err(invalid());
        }
    }
    if r.u32()? == 0 {
        return Err(invalid());
    }
    let mut rows = 0usize;
    for width in [20, 28, 24, 24, 24, 16] {
        skip_rows(&mut r, width, &mut rows)?;
    }
    let components = row_count(&mut r, 24, &mut rows)?;
    for _ in 0..components {
        r.take(20)?;
        skip_rows(&mut r, 8, &mut rows)?;
    }
    skip_rows(&mut r, 16, &mut rows)?;
    r.finish()
}
fn blob<'a>(r: &mut SourceEvidenceReaderV1<'a>) -> Result<&'a [u8]> {
    let length = r.usize_u32()?;
    if length == 0 {
        return Err(invalid());
    }
    r.take(length)
}
fn row_count(r: &mut SourceEvidenceReaderV1<'_>, width: usize, rows: &mut usize) -> Result<usize> {
    let count = r.usize_u32()?;
    *rows = rows
        .checked_add(count)
        .filter(|n| *n <= 262_144)
        .ok_or(E::RefinementReceiptTooLarge)?;
    if count.checked_mul(width).is_none_or(|n| n > r.remaining()) {
        return Err(invalid());
    }
    Ok(count)
}
fn skip_rows(r: &mut SourceEvidenceReaderV1<'_>, width: usize, rows: &mut usize) -> Result<()> {
    let count = row_count(r, width, rows)?;
    r.take(count * width)?;
    Ok(())
}
