//! Bounded inert expanded-publication primitives. These do not admit semantics.

use sha2::{Digest, Sha256};
use std::{error::Error, fmt, mem::size_of};

/// The unchanged aggregate publication-capsule ceiling.
pub const MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4: usize = 160 * 1024 * 1024;
/// A history is additionally limited by the other members of its capsule.
pub const MAX_EXPANDED_HISTORY_RECEIPT_BYTES_V4: usize = MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4;
/// Identity domain of a required complete expanded-history member.
pub const EXPANDED_HISTORY_RECEIPT_DOMAIN_V4: &[u8] = b"FE2O3/EXPANDED-HISTORY-RECEIPT/V4\0";
/// Simultaneous framing hash state, returned identity, final digest and encoded length.
pub const EXPANDED_PUBLICATION_HASH_STORAGE_V4: usize = size_of::<Sha256>()
    + size_of::<ExpandedContentIdentityV4>()
    + size_of::<[u8; 32]>()
    + size_of::<u64>();
/// Outer/header/row readers, original and consuming five-length arrays, twelve
/// counters, scalar/claim/row slices, and both local/returned absolute ranges.
/// This is a conservative simultaneous typed domain, not compiler stack telemetry.
pub const EXPANDED_HISTORY_DIRECTORY_STORAGE_V4: usize = 3 * size_of::<Reader<'static>>()
    + 2 * size_of::<[usize; 5]>()
    + 12 * size_of::<usize>()
    + 3 * size_of::<&'static [u8]>()
    + 2 * size_of::<std::ops::Range<usize>>();

/// Framing failures preserve the caller's concrete cumulative-work error.
#[derive(Debug)]
pub enum ExpandedPublicationErrorV4<E> {
    /// The supplied additional storage cannot cover this operation.
    Storage {
        /// Required simultaneous extent.
        required: usize,
        /// Supplied extent.
        available: usize,
    },
    /// A checked count, length, offset or extent overflowed.
    Overflow,
    /// A fixed framing invariant was violated.
    Format(&'static str),
    /// A canonical content identity did not match.
    Identity(&'static str),
    /// The allocator refused the requested framing buffer.
    Allocation {
        /// Requested capacity.
        requested: usize,
    },
    /// The cumulative-work callback refused work.
    Work(E),
    /// An unchanged strict invocation/receipt decoder refused its input.
    Legacy(crate::LineageDecodeErrorV3),
    /// Canonical nominal descriptor decoding failed without losing its work error.
    Descriptor(fe2o3_kernel_descriptor::DescriptorWireErrorV3<E>),
}
impl<E: fmt::Display> fmt::Display for ExpandedPublicationErrorV4<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage {
                required,
                available,
            } => write!(
                f,
                "expanded framing needs {required} storage bytes, has {available}"
            ),
            Self::Overflow => f.write_str("expanded framing arithmetic overflow"),
            Self::Format(field) => write!(f, "invalid expanded framing: {field}"),
            Self::Identity(field) => write!(f, "expanded framing identity mismatch: {field}"),
            Self::Allocation { requested } => {
                write!(f, "cannot allocate {requested} expanded framing bytes")
            }
            Self::Work(error) => write!(f, "expanded framing work refused: {error}"),
            Self::Legacy(error) => error.fmt(f),
            Self::Descriptor(error) => error.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for ExpandedPublicationErrorV4<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Work(e) => Some(e),
            Self::Legacy(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            _ => None,
        }
    }
}

/// An inert digest and exact preimage length; this is not a proof receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedContentIdentityV4 {
    digest: [u8; 32],
    length: u64,
}
impl ExpandedContentIdentityV4 {
    /// Retains a declared nonzero identity without authenticating its preimage.
    pub fn from_declared(digest: [u8; 32], length: u64) -> Option<Self> {
        (digest != [0; 32] && length != 0).then_some(Self { digest, length })
    }
    /// Returns the declared digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Returns the exact declared preimage length.
    pub const fn byte_len(self) -> u64 {
        self.length
    }
    pub(crate) fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.digest);
        out.extend_from_slice(&self.length.to_le_bytes());
    }
}

/// Computes an inert domain/length/content identity with explicit hash scratch.
/// Input backing is prepaid by the caller; `available` covers this operation's
/// additional scratch. This callback charges framing work, not legacy child internals.
pub fn expanded_content_identity_v4<E>(
    domain: &[u8],
    bytes: &[u8],
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<ExpandedContentIdentityV4, ExpandedPublicationErrorV4<E>> {
    storage(EXPANDED_PUBLICATION_HASH_STORAGE_V4, available)?;
    if bytes.is_empty() {
        return Err(ExpandedPublicationErrorV4::Format(
            "empty identity preimage",
        ));
    }
    work(
        domain
            .len()
            .checked_add(8)
            .and_then(|n| n.checked_add(bytes.len()))
            .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        charge,
    )?;
    let length = u64::try_from(bytes.len()).map_err(|_| ExpandedPublicationErrorV4::Overflow)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(length.to_le_bytes());
    hash.update(bytes);
    ExpandedContentIdentityV4::from_declared(hash.finalize().into(), length)
        .ok_or(ExpandedPublicationErrorV4::Identity("zero digest"))
}

/// Complete owned-header plus actual transferred Vec capacity.
pub fn expanded_history_retained_storage_v4(capacity: usize) -> Option<usize> {
    size_of::<InertExpandedHistoryReceiptV4>().checked_add(capacity)
}
/// Full simultaneous owner and validation/hash scratch, before transfer.
pub fn expanded_history_validation_storage_v4(capacity: usize) -> Option<usize> {
    expanded_history_retained_storage_v4(capacity)?.checked_add(
        EXPANDED_PUBLICATION_HASH_STORAGE_V4.max(EXPANDED_HISTORY_DIRECTORY_STORAGE_V4),
    )
}

/// Move-only exact history bytes. The inner kernel-opt frame requires its own
/// independent semantic replay. Magic and content identity confer no authority.
/// ```compile_fail
/// use fe2o3_compiler_lineage::InertExpandedHistoryReceiptV4;
/// fn duplicate(x: InertExpandedHistoryReceiptV4) { let _ = x.clone(); }
/// ```
pub struct InertExpandedHistoryReceiptV4 {
    bytes: Vec<u8>,
    identity: ExpandedContentIdentityV4,
}
impl InertExpandedHistoryReceiptV4 {
    /// Consumes actual Vec capacity. `available` covers the complete owner and scratch.
    pub fn from_vec<E>(
        bytes: Vec<u8>,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        storage(
            expanded_history_validation_storage_v4(bytes.capacity())
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
            available,
        )?;
        expanded_history_final_graph_range_v4(
            &bytes,
            EXPANDED_HISTORY_DIRECTORY_STORAGE_V4,
            charge,
        )?;
        let identity = expanded_content_identity_v4(
            EXPANDED_HISTORY_RECEIPT_DOMAIN_V4,
            &bytes,
            EXPANDED_PUBLICATION_HASH_STORAGE_V4,
            charge,
        )?;
        Ok(Self { bytes, identity })
    }
    /// Exact immutable kernel-opt wire bytes, including the complete U prefix.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Identity of these exact bytes, not a semantic-proof identity.
    pub const fn identity(&self) -> ExpandedContentIdentityV4 {
        self.identity
    }
    /// Full header plus actual retained allocation capacity.
    pub fn retained_storage(&self) -> usize {
        expanded_history_retained_storage_v4(self.bytes.capacity()).expect("validated extent")
    }
    /// Framing grants no publication, load or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Locates the final scalar-output graph through the closed B1.a directories.
/// This never searches for a matching substring. It checks framing only, not
/// graph validity, transition semantics, adjacency or the fixed-point claim.
/// Input backing is prepaid; the returned range remains included in the extent.
pub fn expanded_history_final_graph_range_v4<E>(
    bytes: &[u8],
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<std::ops::Range<usize>, ExpandedPublicationErrorV4<E>> {
    storage(EXPANDED_HISTORY_DIRECTORY_STORAGE_V4, available)?;
    work(48, charge)?;
    validate_history_tag(bytes)?;
    let mut outer = Reader::new(&bytes[..48]);
    outer.take::<E>(10)?;
    if outer.u16::<E>()? != 1
        || outer.u32::<E>()? != 48
        || outer.u64::<E>()? != bytes.len() as u64
        || outer.u16::<E>()? != 1
        || outer.take::<E>(6)? != [0; 6]
    {
        return Err(ExpandedPublicationErrorV4::Format(
            "expanded history header",
        ));
    }
    let u = usize::try_from(outer.u64::<E>()?).map_err(|_| ExpandedPublicationErrorV4::Overflow)?;
    let scalar_len =
        usize::try_from(outer.u64::<E>()?).map_err(|_| ExpandedPublicationErrorV4::Overflow)?;
    let start = 48usize
        .checked_add(u)
        .ok_or(ExpandedPublicationErrorV4::Overflow)?;
    if u == 0
        || u > 16 * 1024 * 1024
        || start.checked_add(scalar_len) != Some(bytes.len())
        || scalar_len < 160
    {
        return Err(ExpandedPublicationErrorV4::Format(
            "expanded history members",
        ));
    }
    let scalar = bytes
        .get(start..)
        .ok_or(ExpandedPublicationErrorV4::Format("scalar range"))?;
    work(160, charge)?;
    let mut h = Reader::new(&scalar[..160]);
    if h.take::<E>(8)? != b"F2SPH1\0\0"
        || h.u16::<E>()? != 1
        || h.u16::<E>()? != 1
        || h.u32::<E>()? != 160
        || h.u64::<E>()? != scalar_len as u64
    {
        return Err(ExpandedPublicationErrorV4::Format("scalar history header"));
    }
    let rounds = usize::from(h.u16::<E>()?);
    if !(1..=16).contains(&rounds) || h.take::<E>(6)? != [0; 6] {
        return Err(ExpandedPublicationErrorV4::Format("scalar round count"));
    }
    let claim = h.take::<E>(128)?;
    if &claim[..8] != b"F2SFP1\0\0"
        || claim[8..10] != 1u16.to_le_bytes()
        || claim[10..12] != 16u16.to_le_bytes()
        || claim[12..14] != (rounds as u16).to_le_bytes()
        || claim[14..16] != 2u16.to_le_bytes()
        || claim[96..104] != [6, 0, 2, 0, 0, 0, 0, 0]
        || claim[104..112] != [3, 0, 8, 0, 1, 0, 0, 0]
        || claim[112..128] != [0; 16]
    {
        return Err(ExpandedPublicationErrorV4::Format(
            "scalar composition header",
        ));
    }
    let mut cursor = 160usize
        .checked_add(
            rounds
                .checked_mul(48)
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        )
        .ok_or(ExpandedPublicationErrorV4::Overflow)?;
    let mut final_graph = 0..0;
    for ordinal in 0..rounds {
        work(48, charge)?;
        let at = 160 + ordinal * 48;
        let row = scalar
            .get(at..at + 48)
            .ok_or(ExpandedPublicationErrorV4::Format("scalar directory"))?;
        let mut r = Reader::new(row);
        if r.u16::<E>()? as usize != ordinal || r.u16::<E>()? != 1 || r.u32::<E>()? != 0 {
            return Err(ExpandedPublicationErrorV4::Format("scalar directory order"));
        }
        let mut lengths = [0usize; 5];
        for length in &mut lengths {
            *length =
                usize::try_from(r.u64::<E>()?).map_err(|_| ExpandedPublicationErrorV4::Overflow)?;
        }
        if !(1..=16 * 1024 * 1024).contains(&lengths[0])
            || !(1..=16 * 1024 * 1024).contains(&lengths[1])
            || lengths[2] != 416
            || !(1..=4 * 1024 * 1024).contains(&lengths[3])
            || !(808..=4 * 1024 * 1024).contains(&lengths[4])
        {
            return Err(ExpandedPublicationErrorV4::Format("scalar child extent"));
        }
        for (field, length) in lengths.into_iter().enumerate() {
            let end = cursor
                .checked_add(length)
                .ok_or(ExpandedPublicationErrorV4::Overflow)?;
            if end > scalar_len {
                return Err(ExpandedPublicationErrorV4::Format("scalar child range"));
            }
            if ordinal + 1 == rounds && field == 1 {
                final_graph = start
                    .checked_add(cursor)
                    .ok_or(ExpandedPublicationErrorV4::Overflow)?
                    ..start
                        .checked_add(end)
                        .ok_or(ExpandedPublicationErrorV4::Overflow)?;
            }
            cursor = end;
        }
    }
    if cursor != scalar_len {
        return Err(ExpandedPublicationErrorV4::Format("scalar trailing bytes"));
    }
    let declared_output = u64::from_le_bytes(
        claim[88..96]
            .try_into()
            .map_err(|_| ExpandedPublicationErrorV4::Format("scalar output length"))?,
    );
    if declared_output != (final_graph.end - final_graph.start) as u64 {
        return Err(ExpandedPublicationErrorV4::Format(
            "scalar final graph length",
        ));
    }
    Ok(final_graph)
}

pub(crate) fn validate_history_tag<E>(bytes: &[u8]) -> Result<(), ExpandedPublicationErrorV4<E>> {
    if bytes.len() < 48
        || bytes.len() > MAX_EXPANDED_HISTORY_RECEIPT_BYTES_V4
        || bytes.get(..8) != Some(b"F2EPH1\0\0")
        || bytes.get(8..10) != Some(&1u16.to_le_bytes())
    {
        return Err(ExpandedPublicationErrorV4::Format(
            "expanded history schema",
        ));
    }
    Ok(())
}
pub(crate) fn storage<E>(
    required: usize,
    available: usize,
) -> Result<(), ExpandedPublicationErrorV4<E>> {
    if required > available {
        Err(ExpandedPublicationErrorV4::Storage {
            required,
            available,
        })
    } else {
        Ok(())
    }
}
pub(crate) fn work<E>(
    amount: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ExpandedPublicationErrorV4<E>> {
    charge(amount).map_err(ExpandedPublicationErrorV4::Work)
}
pub(crate) fn allocate<E>(length: usize) -> Result<Vec<u8>, ExpandedPublicationErrorV4<E>> {
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| ExpandedPublicationErrorV4::Allocation { requested: length })?;
    Ok(out)
}
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) at: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub(crate) fn take<E>(
        &mut self,
        len: usize,
    ) -> Result<&'a [u8], ExpandedPublicationErrorV4<E>> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(ExpandedPublicationErrorV4::Overflow)?;
        let out = self
            .bytes
            .get(self.at..end)
            .ok_or(ExpandedPublicationErrorV4::Format("truncated"))?;
        self.at = end;
        Ok(out)
    }
    pub(crate) fn array<E, const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ExpandedPublicationErrorV4<E>> {
        self.take(N)?
            .try_into()
            .map_err(|_| ExpandedPublicationErrorV4::Format("fixed field"))
    }
    pub(crate) fn u16<E>(&mut self) -> Result<u16, ExpandedPublicationErrorV4<E>> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(crate) fn u32<E>(&mut self) -> Result<u32, ExpandedPublicationErrorV4<E>> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(crate) fn u64<E>(&mut self) -> Result<u64, ExpandedPublicationErrorV4<E>> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(crate) fn identity<E>(
        &mut self,
    ) -> Result<ExpandedContentIdentityV4, ExpandedPublicationErrorV4<E>> {
        ExpandedContentIdentityV4::from_declared(self.array()?, self.u64()?)
            .ok_or(ExpandedPublicationErrorV4::Identity("declared content"))
    }
    pub(crate) fn finish<E>(self) -> Result<(), ExpandedPublicationErrorV4<E>> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(ExpandedPublicationErrorV4::Format("trailing bytes"))
        }
    }
}
