//! Mixed stage content inside the existing inert V3 middle-end receipt.
//! Eight fixed pair nodes share the established strict framing engine. Payload
//! bytes are never decoded into source custody or executed-proof authority here.

use crate::{OrderedInertSemanticLineageReceiptsV3, bounded_pair as pair};
use sha2::Sha256;
use std::{convert::Infallible, mem::size_of, ops::Range};

/// Distinct mixed stage content, not a new outer capsule or a policy alias.
pub const MIXED_MIDDLE_END_MAGIC_V50: [u8; 8] = *b"F2MC50R\0";
/// Unchanged existing middle-end receipt ceiling.
pub const MAX_MIXED_MIDDLE_END_BYTES_V50: usize = crate::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;
/// Inert witness ceiling; each nominal producer independently enforces its
/// actual schedule and round limits. The aggregate stage cap is unchanged.
pub const MAX_MIXED_MIDDLE_END_WITNESS_BYTES_V50: usize = MAX_MIXED_MIDDLE_END_BYTES_V50;
const STORAGE_MAX: usize = crate::MAX_NATIVE_CONDITIONAL_STORAGE_V1;
const MAX: usize = MAX_MIXED_MIDDLE_END_BYTES_V50;
const MAXIMA: [usize; 9] = [
    MAX,
    32,
    MAX,
    MAX,
    MAX,
    MAX,
    MAX_MIXED_MIDDLE_END_WITNESS_BYTES_V50,
    MAX,
    MAX,
];
const fn policy(magic: [u8; 8], domain: &'static [u8]) -> pair::Policy {
    pair::Policy {
        magic,
        version: 50,
        domain,
        first_max: MAX,
        second_max: MAX,
        total_max: MAX,
        storage_max: STORAGE_MAX,
    }
}
// Internal node indices precede the nine leaves in canonical field order.
const CHILDREN: [[usize; 2]; 8] = [
    [1, 2],
    [3, 4],
    [5, 6],
    [8, 9],
    [10, 11],
    [12, 13],
    [14, 7],
    [15, 16],
];
const POLICIES: [pair::Policy; 8] = [
    policy(
        MIXED_MIDDLE_END_MAGIC_V50,
        b"FE2O3/MIXED-MIDDLE-END/ROOT/V50\0",
    ),
    policy(*b"F2MC50I\0", b"FE2O3/MIXED-MIDDLE-END/INPUT/V50\0"),
    policy(*b"F2MC50T\0", b"FE2O3/MIXED-MIDDLE-END/TAIL/V50\0"),
    policy(*b"F2MC50S\0", b"FE2O3/MIXED-MIDDLE-END/SOURCE/V50\0"),
    policy(*b"F2MC50G\0", b"FE2O3/MIXED-MIDDLE-END/GRAPHS/V50\0"),
    policy(*b"F2MC50O\0", b"FE2O3/MIXED-MIDDLE-END/OUTPUT/V50\0"),
    policy(*b"F2MC50P\0", b"FE2O3/MIXED-MIDDLE-END/PROOF/V50\0"),
    policy(*b"F2MC50E\0", b"FE2O3/MIXED-MIDDLE-END/EXECUTION/V50\0"),
];

/// All mandatory exact preimages; supplying them does not authenticate them.
#[derive(Clone, Copy)]
pub struct MixedMiddleEndInputV50<'a> {
    /// Original admitted canonical semantic MIR.
    pub semantic_mir: &'a [u8],
    /// Exact retained SSA-plan identity, not an independently accepted proof.
    pub source_ssa_identity: &'a [u8],
    /// Canonical original V18 input.
    pub original: &'a [u8],
    /// Canonical output of the exact nominal prefix policy.
    pub prefix: &'a [u8],
    /// Canonical output of the exact LICM relocation.
    pub licm: &'a [u8],
    /// Canonical final output after checked memory forwarding.
    pub forwarded: &'a [u8],
    /// Full actual variable-round execution witness.
    pub prefix_witness: &'a [u8],
    /// Complete generated typed source/prefix/LICM/forwarding proof source.
    pub generated_source: &'a [u8],
    /// Exact typed execution wire, still inert at this framing layer.
    pub execution_receipt: &'a [u8],
}
impl<'a> MixedMiddleEndInputV50<'a> {
    fn fields(self) -> [&'a [u8]; 9] {
        [
            self.semantic_mir,
            self.source_ssa_identity,
            self.original,
            self.prefix,
            self.licm,
            self.forwarded,
            self.prefix_witness,
            self.generated_source,
            self.execution_receipt,
        ]
    }
    fn from_fields(f: [&'a [u8]; 9]) -> Self {
        Self {
            semantic_mir: f[0],
            source_ssa_identity: f[1],
            original: f[2],
            prefix: f[3],
            licm: f[4],
            forwarded: f[5],
            prefix_witness: f[6],
            generated_source: f[7],
            execution_receipt: f[8],
        }
    }
}

/// Strict framing or original prepaid-resource refusal.
#[derive(Debug, Eq, PartialEq)]
pub enum MixedMiddleEndErrorV50<E = Infallible> {
    /// Caller work refusal.
    Charge(E),
    /// Mandatory field violates its own bound.
    FieldLength,
    /// Aggregate, declared, or exact destination length differs.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Fixed magic, policy, version, or header differs.
    Header,
    /// Reserved bytes are nonzero.
    Reserved,
    /// A domain-separated node digest differs.
    Identity,
    /// Caller has not prepaid fixed scratch, or exceeds the common ceiling.
    Storage,
    /// A mandatory exact capsule/source field differs.
    Binding,
}
impl<E: std::fmt::Display> std::fmt::Display for MixedMiddleEndErrorV50<E> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Charge(error) => write!(out, "mixed middle-end work refused: {error}"),
            Self::FieldLength => out.write_str("mixed middle-end mandatory field length differs"),
            Self::Length => out.write_str("mixed middle-end complete wire extent differs"),
            Self::Arithmetic => out.write_str("mixed middle-end resource arithmetic overflow"),
            Self::Header => out.write_str("mixed middle-end canonical header differs"),
            Self::Reserved => out.write_str("mixed middle-end reserved bytes are nonzero"),
            Self::Identity => out.write_str("mixed middle-end content identity differs"),
            Self::Storage => out.write_str("mixed middle-end prepaid scratch extent differs"),
            Self::Binding => out.write_str("mixed middle-end exact capsule inputs differ"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for MixedMiddleEndErrorV50<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Charge(error) => Some(error),
            _ => None,
        }
    }
}
type Error<E> = MixedMiddleEndErrorV50<E>;
impl<E> From<pair::Error<E>> for Error<E> {
    fn from(e: pair::Error<E>) -> Self {
        match e {
            pair::Error::Charge(e) => Self::Charge(e),
            pair::Error::FirstLength | pair::Error::SecondLength => Self::FieldLength,
            pair::Error::Length => Self::Length,
            pair::Error::Arithmetic => Self::Arithmetic,
            pair::Error::Header => Self::Header,
            pair::Error::Reserved => Self::Reserved,
            pair::Error::Identity => Self::Identity,
            pair::Error::StorageLimit => Self::Storage,
        }
    }
}
fn lengths<E>(f: [usize; 9]) -> Result<(), Error<E>> {
    for (n, max) in f.into_iter().zip(MAXIMA) {
        if n == 0 || n > max {
            return Err(Error::FieldLength);
        }
    }
    if f[1] != 32 {
        return Err(Error::FieldLength);
    }
    Ok(())
}

/// Fixed eight-node framing plan; does not traverse any payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedMiddleEndLayoutV50 {
    nodes: [pair::Layout; 8],
}
impl MixedMiddleEndLayoutV50 {
    /// Checks all independent extents and the unchanged aggregate stage bound.
    pub fn new<E>(input: MixedMiddleEndInputV50<'_>) -> Result<Self, Error<E>> {
        Self::from_lengths(input.fields().map(<[u8]>::len))
    }
    fn from_lengths<E>(fields: [usize; 9]) -> Result<Self, Error<E>> {
        lengths(fields)?;
        let placeholder = pair::Layout::new(&POLICIES[0], 1, 1)?;
        let mut nodes = [placeholder; 8];
        let mut n = [0; 17];
        n[8..].copy_from_slice(&fields);
        for index in (0..8).rev() {
            let [first, second] = CHILDREN[index];
            nodes[index] = pair::Layout::new(&POLICIES[index], n[first], n[second])?;
            n[index] = nodes[index].encoded_len();
        }
        Ok(Self { nodes })
    }
    /// Exact complete middle-end preimage length.
    pub const fn encoded_len(self) -> usize {
        self.nodes[0].encoded_len()
    }
    fn ranges(self) -> [Range<usize>; 17] {
        let mut ranges = std::array::from_fn(|_| 0..0);
        ranges[0] = 0..self.encoded_len();
        for i in 0..8 {
            let base = ranges[i].start;
            let a = self.nodes[i].first_range();
            let b = self.nodes[i].second_range();
            let [first, second] = CHILDREN[i];
            ranges[first] = base + a.start..base + a.end;
            ranges[second] = base + b.start..base + b.end;
        }
        ranges
    }
}

/// Exact stage identity only; no signer or producer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedMiddleEndIdentityV50(pair::Identity);
impl MixedMiddleEndIdentityV50 {
    /// Root domain-separated digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.0.sha256
    }
    /// Complete canonical extent.
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
}

/// Borrowed strict content view, retaining the original immutable input and four distinct graph stages.
pub struct MixedMiddleEndRefV50<'a> {
    bytes: &'a [u8],
    input: MixedMiddleEndInputV50<'a>,
    identity: MixedMiddleEndIdentityV50,
}
impl<'a> MixedMiddleEndRefV50<'a> {
    /// Exact complete stage preimage.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Mandatory framed preimages; semantic admission is separate.
    pub const fn input(&self) -> MixedMiddleEndInputV50<'a> {
        self.input
    }
    /// Domain-separated stage identity.
    pub const fn identity(&self) -> MixedMiddleEndIdentityV50 {
        self.identity
    }
    /// Content framing never grants authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Requires every actual expected field byte, not only digests or prefixes.
    pub fn check_exact<E>(
        &self,
        expected: MixedMiddleEndInputV50<'_>,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), Error<E>> {
        let actual = self.input.fields();
        let expected = expected.fields();
        for (a, e) in actual.into_iter().zip(expected) {
            if a.len() != e.len() {
                return Err(Error::Binding);
            }
            charge(a.len()).map_err(Error::Charge)?;
            if a != e {
                return Err(Error::Binding);
            }
        }
        Ok(())
    }
    /// Rejoins the existing capsule's exact middle-end, source, and KIR fields.
    /// This does not decode or authenticate its remaining proof receipts.
    pub fn check_capsule<E>(
        &self,
        receipts: &OrderedInertSemanticLineageReceiptsV3,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), Error<E>> {
        for (a, e) in [
            (self.bytes, receipts.middle_end().canonical_preimage()),
            (
                self.input.semantic_mir,
                receipts.semantic_mir().canonical_preimage(),
            ),
            (
                self.input.forwarded,
                receipts.kernel_ir().canonical_preimage(),
            ),
        ] {
            if a.len() != e.len() {
                return Err(Error::Binding);
            }
            charge(a.len()).map_err(Error::Charge)?;
            if a != e {
                return Err(Error::Binding);
            }
        }
        Ok(())
    }
}

/// Fixed simultaneous algorithm scratch; caller-owned input/output backing,
/// callback captures and generic error/result envelopes are separate.
pub const MIXED_MIDDLE_END_WORKING_STORAGE_V50: usize = 2 * size_of::<MixedMiddleEndLayoutV50>()
    + 2 * size_of::<MixedMiddleEndRefV50<'static>>()
    + 2 * size_of::<MixedMiddleEndInputV50<'static>>()
    + 2 * size_of::<[Range<usize>; 17]>()
    + 2 * size_of::<[&[u8]; 9]>()
    + size_of::<[usize; 17]>()
    + size_of::<[usize; 9]>()
    + 2 * size_of::<Sha256>()
    + size_of::<[[u8; pair::HEADER]; 2]>()
    + size_of::<[pair::Layout; 2]>()
    + size_of::<[pair::Identity; 2]>()
    + size_of::<[Option<pair::Identity>; 2]>()
    + size_of::<[usize; 10]>()
    + size_of::<[u8; 32]>()
    + size_of::<[u8; 8]>();
fn storage<E>(prepaid: usize) -> Result<(), Error<E>> {
    if !(MIXED_MIDDLE_END_WORKING_STORAGE_V50..=STORAGE_MAX).contains(&prepaid) {
        return Err(Error::Storage);
    }
    Ok(())
}

/// Writes into caller-prepaid exact storage. All work and captured-destructor
/// execution precede mutation. No partial output is returned on a refusal.
pub fn encode_mixed_middle_end_v50<E>(
    input: MixedMiddleEndInputV50<'_>,
    bytes: &mut [u8],
    prepaid: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<MixedMiddleEndIdentityV50, Error<E>> {
    storage(prepaid)?;
    let layout = MixedMiddleEndLayoutV50::new(input)?;
    if bytes.len() != layout.encoded_len() {
        return Err(Error::Length);
    }
    let ranges = layout.ranges();
    let mut work = input.fields().iter().try_fold(0usize, |sum, f| {
        sum.checked_add(f.len()).ok_or(Error::Arithmetic)
    })?;
    for i in 0..8 {
        work = work
            .checked_add(pair::seal_work::<E>(
                &POLICIES[i],
                layout.nodes[i],
                ranges[i].len(),
                prepaid,
            )?)
            .ok_or(Error::Arithmetic)?;
    }
    charge(work).map_err(Error::Charge)?;
    drop(charge);
    for (field, range) in input.fields().into_iter().zip(&ranges[8..]) {
        bytes[range.clone()].copy_from_slice(field);
    }
    let mut identity = None;
    for i in (0..8).rev() {
        identity = Some(pair::seal(
            &POLICIES[i],
            layout.nodes[i],
            &mut bytes[ranges[i].clone()],
            prepaid,
            |_| Ok::<_, E>(()),
        )?);
    }
    Ok(MixedMiddleEndIdentityV50(
        identity.ok_or(Error::Arithmetic)?,
    ))
}

/// Strict allocation-free, fixed-depth framing read. No legacy-schema retry.
pub fn read_mixed_middle_end_v50<E>(
    bytes: &[u8],
    prepaid: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<MixedMiddleEndRefV50<'_>, Error<E>> {
    storage(prepaid)?;
    let mut ranges: [Range<usize>; 17] = std::array::from_fn(|_| 0..0);
    ranges[0] = 0..bytes.len();
    let mut identity = None;
    for i in 0..8 {
        let base = ranges[i].start;
        let (layout, found) = pair::read(
            &POLICIES[i],
            &bytes[ranges[i].clone()],
            prepaid,
            &mut charge,
        )?;
        if i == 0 {
            identity = Some(found);
        }
        let a = layout.first_range();
        let b = layout.second_range();
        let [first, second] = CHILDREN[i];
        ranges[first] = base + a.start..base + a.end;
        ranges[second] = base + b.start..base + b.end;
    }
    let fields = std::array::from_fn(|i| &bytes[ranges[i + 8].clone()]);
    lengths(fields.map(<[u8]>::len))?;
    Ok(MixedMiddleEndRefV50 {
        bytes,
        input: MixedMiddleEndInputV50::from_fields(fields),
        identity: MixedMiddleEndIdentityV50(identity.ok_or(Error::Arithmetic)?),
    })
}

#[cfg(test)]
#[path = "mixed_middle_end_v50_tests.rs"]
mod tests;
