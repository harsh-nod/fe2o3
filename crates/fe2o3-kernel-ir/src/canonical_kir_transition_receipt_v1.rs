//! Inert canonical occurrence rows. Decoding is not transition admission.

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirBlockSegmentV1 as Segment,
    CanonicalKirBlockTransitionV1 as BlockRow, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind,
    CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirDefinitionTransitionV1 as DefinitionRow,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirEdgeArgumentTransitionV1 as EdgeArgumentRow, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirEdgeTransitionV1 as EdgeRow, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirFunctionTransitionV1 as FunctionRow,
    CanonicalKirOperationCoordinateV1 as Operation, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirOperationTransitionV1 as OperationRow,
    CanonicalKirTransitionCandidateV1 as Candidate, CanonicalKirTransitionRangeV1 as Range,
    CanonicalKirUseCoordinateV1 as Use, CanonicalKirUseTransitionV1 as UseRow, MAX_MODULE_BYTES_V1,
    VerifiedCanonicalKernelIrIdentityV12,
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "canonical_kir_transition_receipt_v1_rows.rs"]
mod rows;

#[path = "canonical_kir_occurrence_row_bytes_v1.rs"]
mod row_bytes;
pub use row_bytes::*;

/// Fixed named scalar/CFG transition checker policy, independent of wire schema.
pub const CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1: u16 = 1;
/// Canonical occurrence receipt framing, separate from graph and legacy map frames.
pub const CANONICAL_KIR_TRANSITION_RECEIPT_MAGIC_V1: [u8; 8] = *b"F2NTR1\0\0";
/// Standalone cap; enclosing receipts must additionally bound their aggregate.
/// Equals the existing 4 MiB lineage preimage cap without depending on lineage.
pub const MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1: usize = 4 * 1024 * 1024;
/// Domain for inert row-receipt identities, never verified graph identities.
pub const CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CANONICAL-KIR-TRANSITION-ROWS/V1\0";

const HEADER: usize = 132;
const WIDTHS: [usize; 9] = [8, 16, 24, 36, 28, 24, 40, 24, 32];
const SIZES: [usize; 9] = [
    size_of::<FunctionRow>(),
    size_of::<BlockRow>(),
    size_of::<Segment>(),
    size_of::<OperationRow>(),
    size_of::<DefinitionRow>(),
    size_of::<Descendant>(),
    size_of::<UseRow>(),
    size_of::<EdgeRow>(),
    size_of::<EdgeArgumentRow>(),
];
type Result<T> = std::result::Result<T, CanonicalKirTransitionReceiptErrorV1>;

/// An inert endpoint locator. A decoder cannot mint a verified graph identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertCanonicalKirTransitionGraphIdentityV1 {
    digest: [u8; 32],
    canonical_length: u64,
}
impl InertCanonicalKirTransitionGraphIdentityV1 {
    /// Copies coordinates from an admitted identity without retaining its graph.
    pub const fn from_verified(identity: &VerifiedCanonicalKernelIrIdentityV12) -> Self {
        Self {
            digest: *identity.digest(),
            canonical_length: identity.canonical_length(),
        }
    }
    /// Inert claimed endpoint digest, not graph admission.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Inert claimed endpoint canonical byte length.
    pub const fn canonical_length(self) -> u64 {
        self.canonical_length
    }
    /// Exact coordinate equality only; this does not establish owner custody.
    pub fn matches_verified(self, identity: &VerifiedCanonicalKernelIrIdentityV12) -> bool {
        self.digest == *identity.digest() && self.canonical_length == identity.canonical_length()
    }
}

/// Canonical framing/row validation failure, not a semantic checker result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirTransitionReceiptErrorV1 {
    /// Shared logical work/storage admission or allocation failed.
    Resource(Resource),
    /// Checked arithmetic, count conversion or byte bound failed.
    Limit,
    /// A canonical tag, padding, length or field is invalid.
    Malformed(&'static str),
    /// Parent ranges do not partition their corresponding flat slice exactly.
    RangePartition,
    /// All codec-owned values dropped after an internal unwind.
    Panicked,
}
impl fmt::Display for CanonicalKirTransitionReceiptErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "transition receipt resources: {e}"),
            Self::Limit => f.write_str("transition receipt limit exceeded"),
            Self::Malformed(field) => write!(f, "noncanonical transition receipt: {field}"),
            Self::RangePartition => f.write_str("transition receipt range partition differs"),
            Self::Panicked => f.write_str("transition receipt codec panicked"),
        }
    }
}
impl Error for CanonicalKirTransitionReceiptErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}
impl From<Resource> for CanonicalKirTransitionReceiptErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

#[derive(Debug)]
struct Rows {
    functions: Vec<FunctionRow>,
    blocks: Vec<BlockRow>,
    segments: Vec<Segment>,
    operations: Vec<OperationRow>,
    definitions: Vec<DefinitionRow>,
    definition_outputs: Vec<Descendant>,
    uses: Vec<UseRow>,
    edges: Vec<EdgeRow>,
    edge_arguments: Vec<EdgeArgumentRow>,
}
impl Rows {
    fn candidate(&self) -> Candidate<'_> {
        Candidate {
            functions: &self.functions,
            blocks: &self.blocks,
            segments: &self.segments,
            operations: &self.operations,
            definitions: &self.definitions,
            definition_outputs: &self.definition_outputs,
            uses: &self.uses,
            edges: &self.edges,
            edge_arguments: &self.edge_arguments,
        }
    }
}

/// Move-only canonical candidate rows with inert endpoint coordinates.
/// Neither construction nor decoding checks graph coverage or rewrite semantics.
/// Only the independent analysis checker can admit the two actual inventories.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<InertCanonicalKirTransitionReceiptV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{InertCanonicalKirTransitionReceiptV1,
///     VerifiedCanonicalKernelIrIdentityV12};
/// fn not_verified(receipt: &InertCanonicalKirTransitionReceiptV1)
///     -> VerifiedCanonicalKernelIrIdentityV12 { receipt.input_identity() }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{InertCanonicalKirTransitionReceiptV1,
///     CanonicalKirTransitionCandidateV1};
/// fn cannot_escape(receipt: InertCanonicalKirTransitionReceiptV1)
///     -> CanonicalKirTransitionCandidateV1<'static> { receipt.candidate() }
/// ```
#[derive(Debug)]
pub struct InertCanonicalKirTransitionReceiptV1 {
    input: InertCanonicalKirTransitionGraphIdentityV1,
    output: InertCanonicalKirTransitionGraphIdentityV1,
    rows: Rows,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

/// Exact logical retained receipt header, typed-row capacities and byte capacity.
/// Reserve before subsequent controlled allocation; this is not allocator/RSS size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirTransitionReceiptStorageV1(usize);
impl CanonicalKirTransitionReceiptStorageV1 {
    /// Reserve exactly once while the returned receipt owner is retained.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl InertCanonicalKirTransitionReceiptV1 {
    /// Serializes borrowed candidate rows. Endpoint identities alone do not make
    /// those rows checked. Input owners/rows must already remain reserved.
    /// Every exit restores the incoming floor after temporary owners drop;
    /// success transfers one exact receipt. Work, peak and failures accumulate.
    pub fn from_candidate_with_budget(
        input: &VerifiedCanonicalKernelIrIdentityV12,
        output: &VerifiedCanonicalKernelIrIdentityV12,
        candidate: Candidate<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirTransitionReceiptStorageV1)> {
        with_floor(budget, |budget| {
            budget.charge_work(1)?;
            let counts = counts(candidate);
            let (length, count, storage) = extent(counts)?;
            validate_ranges(candidate, budget)?;
            budget.reserve_storage(storage)?;
            budget.charge_work(
                length
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(count))
                    .and_then(|n| {
                        n.checked_add(CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1.len() + 8)
                    })
                    .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?,
            )?;
            let rows = rows::copy(candidate)?;
            let input = InertCanonicalKirTransitionGraphIdentityV1::from_verified(input);
            let output = InertCanonicalKirTransitionGraphIdentityV1::from_verified(output);
            let mut writer = Writer {
                bytes: allocate(length)?,
                length,
            };
            writer.put(CANONICAL_KIR_TRANSITION_RECEIPT_MAGIC_V1)?;
            writer.put(1_u16.to_le_bytes())?;
            writer.put(CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1.to_le_bytes())?;
            writer.u32(
                u32::try_from(length).map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?,
            )?;
            writer.identity(input)?;
            writer.identity(output)?;
            for count in counts {
                writer.u32(
                    u32::try_from(count)
                        .map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?,
                )?;
            }
            rows::encode(&mut writer, rows.candidate())?;
            if writer.bytes.len() != length {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "encoded length",
                ));
            }
            let bytes = writer.bytes;
            let digest = digest(&bytes);
            Ok((
                Self {
                    input,
                    output,
                    rows,
                    bytes,
                    digest,
                },
                CanonicalKirTransitionReceiptStorageV1(storage),
            ))
        })
    }

    /// Strictly decodes fixed-width candidate rows without admitting any graph.
    /// Caller-owned input bytes remain reserved separately. All owned headers,
    /// typed rows and canonical bytes are precharged while they coexist.
    /// Floor restoration and successful transfer match the encoder.
    pub fn decode_with_budget(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirTransitionReceiptStorageV1)> {
        with_floor(budget, |budget| {
            budget.charge_work(1)?;
            if !(HEADER..=MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1).contains(&bytes.len()) {
                return Err(CanonicalKirTransitionReceiptErrorV1::Limit);
            }
            budget.charge_work(HEADER)?;
            let mut reader = Reader { bytes, offset: 0 };
            if reader.take::<8>()? != CANONICAL_KIR_TRANSITION_RECEIPT_MAGIC_V1
                || u16::from_le_bytes(reader.take()?) != 1
                || u16::from_le_bytes(reader.take()?) != CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1
            {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "header policy",
                ));
            }
            if usize::try_from(reader.u32()?).ok() != Some(bytes.len()) {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "frame length",
                ));
            }
            let input = reader.identity()?;
            let output = reader.identity()?;
            let mut counts = [0; 9];
            for count in &mut counts {
                *count = usize::try_from(reader.u32()?)
                    .map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?;
            }
            let (length, count, storage) = extent(counts)?;
            if length != bytes.len() {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "row extent",
                ));
            }
            budget.reserve_storage(storage)?;
            budget.charge_work(
                (length - HEADER)
                    .checked_add(count)
                    .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?,
            )?;
            let rows = rows::decode(&mut reader, counts)?;
            if reader.offset != length {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "trailing data",
                ));
            }
            validate_ranges(rows.candidate(), budget)?;
            budget.charge_work(
                length
                    .checked_mul(2)
                    .and_then(|n| {
                        n.checked_add(CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1.len() + 8)
                    })
                    .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?,
            )?;
            let mut owned = allocate(length)?;
            owned.extend_from_slice(bytes);
            let digest = digest(&owned);
            Ok((
                Self {
                    input,
                    output,
                    rows,
                    bytes: owned,
                    digest,
                },
                CanonicalKirTransitionReceiptStorageV1(storage),
            ))
        })
    }

    /// Immutable canonical framing; it has no graph or executable authority.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Inert row-receipt digest, distinct from both endpoint graph identities.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Claimed original endpoint, requiring independent actual-owner comparison.
    pub const fn input_identity(&self) -> InertCanonicalKirTransitionGraphIdentityV1 {
        self.input
    }
    /// Claimed final endpoint, requiring independent actual-owner comparison.
    pub const fn output_identity(&self) -> InertCanonicalKirTransitionGraphIdentityV1 {
        self.output
    }
    /// Fixed closed checker policy to be interpreted only by its actual checker.
    pub const fn checker_policy(&self) -> u16 {
        CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1
    }
    /// Borrowed untrusted rows; their lifetime cannot outlive this inert owner.
    pub fn candidate(&self) -> Candidate<'_> {
        self.rows.candidate()
    }
    /// Canonical framing and hashes grant no proof or execution authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn counts(candidate: Candidate<'_>) -> [usize; 9] {
    [
        candidate.functions.len(),
        candidate.blocks.len(),
        candidate.segments.len(),
        candidate.operations.len(),
        candidate.definitions.len(),
        candidate.definition_outputs.len(),
        candidate.uses.len(),
        candidate.edges.len(),
        candidate.edge_arguments.len(),
    ]
}
fn extent(counts: [usize; 9]) -> Result<(usize, usize, usize)> {
    let mut length = HEADER;
    let mut rows = 0_usize;
    let mut storage = size_of::<InertCanonicalKirTransitionReceiptV1>();
    for ((count, width), size) in counts.into_iter().zip(WIDTHS).zip(SIZES) {
        u32::try_from(count).map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?;
        length = count
            .checked_mul(width)
            .and_then(|n| length.checked_add(n))
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
        rows = rows
            .checked_add(count)
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
        storage = count
            .checked_mul(size)
            .and_then(|n| storage.checked_add(n))
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
    }
    if length > MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1 {
        return Err(CanonicalKirTransitionReceiptErrorV1::Limit);
    }
    storage = storage
        .checked_add(length)
        .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
    Ok((length, rows, storage))
}
fn validate_ranges(candidate: Candidate<'_>, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(
        candidate
            .blocks
            .len()
            .checked_add(candidate.definitions.len())
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?,
    )?;
    let mut end = 0_usize;
    for block in candidate.blocks {
        if block.segments.len == 0 {
            return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
        }
        end = range_end(block.segments, end)?;
    }
    if end != candidate.segments.len() {
        return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
    }
    end = 0;
    for definition in candidate.definitions {
        end = range_end(definition.outputs, end)?;
    }
    if end != candidate.definition_outputs.len() {
        return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
    }
    Ok(())
}
fn range_end(range: Range, expected: usize) -> Result<usize> {
    if usize::try_from(range.start).ok() != Some(expected) {
        return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
    }
    let end = range
        .start
        .checked_add(range.len)
        .ok_or(CanonicalKirTransitionReceiptErrorV1::RangePartition)?;
    usize::try_from(end).map_err(|_| CanonicalKirTransitionReceiptErrorV1::RangePartition)
}
fn allocate<T>(count: usize) -> Result<Vec<T>> {
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if vector.capacity() != count {
        return Err(Resource::Allocation.into());
    }
    Ok(vector)
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn with_floor<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(CanonicalKirTransitionReceiptErrorV1::Panicked)
        }
    };
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

struct Writer {
    bytes: Vec<u8>,
    length: usize,
}
impl Writer {
    fn put<const N: usize>(&mut self, bytes: [u8; N]) -> Result<()> {
        if self
            .bytes
            .len()
            .checked_add(N)
            .is_none_or(|end| end > self.length)
        {
            return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "writer extent",
            ));
        }
        self.bytes.extend_from_slice(&bytes);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<()> {
        self.put(value.to_le_bytes())
    }
    fn identity(&mut self, identity: InertCanonicalKirTransitionGraphIdentityV1) -> Result<()> {
        self.put(identity.digest)?;
        self.put(identity.canonical_length.to_le_bytes())
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
        let value = self.bytes.get(self.offset..end).ok_or(
            CanonicalKirTransitionReceiptErrorV1::Malformed("truncated field"),
        )?;
        self.offset = end;
        value
            .try_into()
            .map_err(|_| CanonicalKirTransitionReceiptErrorV1::Malformed("field width"))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn identity(&mut self) -> Result<InertCanonicalKirTransitionGraphIdentityV1> {
        let digest = self.take()?;
        let canonical_length = u64::from_le_bytes(self.take()?);
        if canonical_length == 0
            || usize::try_from(canonical_length)
                .ok()
                .is_none_or(|n| n > MAX_MODULE_BYTES_V1)
        {
            return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "endpoint length",
            ));
        }
        Ok(InertCanonicalKirTransitionGraphIdentityV1 {
            digest,
            canonical_length,
        })
    }
}

#[cfg(test)]
#[path = "canonical_kir_transition_receipt_v1_tests.rs"]
mod tests;
