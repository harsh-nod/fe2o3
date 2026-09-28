//! Inert source-bound policy material; replay and policy acceptance remain separate.
use crate::{
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3, MAX_NATIVE_CONDITIONAL_STORAGE_V1,
    MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1,
};
use sha2::{Digest, Sha256};
use std::{convert::Infallible, mem::size_of, ops::Range};

/// Closed inert policy roster discriminator.
pub const NATIVE_CONDITIONAL_POLICY_ROSTER_MAGIC_V1: [u8; 8] = *b"F2NPR1\0\0";
/// Fixed source identity and root count header.
pub const NATIVE_CONDITIONAL_POLICY_ROSTER_HEADER_BYTES_V1: usize = 80;
/// Fixed part of each row, preceding its signer array.
pub const NATIVE_CONDITIONAL_POLICY_ROOT_HEADER_BYTES_V1: usize = 400;
/// Independent root bound shared with the existing native proof roster.
pub const MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1: usize =
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3;
/// Independent signer bound for each effect policy.
pub const MAX_NATIVE_CONDITIONAL_POLICY_SIGNERS_PER_ROOT_V1: usize = 4096;
const HEADER: usize = NATIVE_CONDITIONAL_POLICY_ROSTER_HEADER_BYTES_V1;
const ROW: usize = NATIVE_CONDITIONAL_POLICY_ROOT_HEADER_BYTES_V1;
const ROOTS: usize = MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1;
const SIGNERS: usize = MAX_NATIVE_CONDITIONAL_POLICY_SIGNERS_PER_ROOT_V1;
const DOMAIN: &[u8] = b"FE2O3/NATIVE-CONDITIONAL-POLICY-ROSTER/V1\0";
/// Complete independent encoding ceiling, including the terminal digest.
pub const MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_BYTES_V1: usize =
    HEADER + 32 + ROOTS * (ROW + 32 * SIGNERS);

/// Claimed policy for one source root; these public values grant no authority.
#[derive(Clone, Copy, Debug)]
pub struct NativeConditionalPolicyRootInputV1<'a> {
    /// Source semantic function ID, not a numeric sorting key.
    pub semantic_root: u32,
    /// Claimed source kernel binding.
    pub kernel_binding: [u8; 32],
    /// One to 4096 lexicographically increasing, unique nonzero signer identities.
    pub effect_signers: &'a [[u8; 32]],
    /// Five nonzero effect toolchain identities in their existing order.
    pub effect_toolchain: [[u8; 32]; 5],
    /// Nonzero formula key bytes; cryptographic key validation is separate.
    pub formula_verifying_key: [u8; 32],
    /// Five nonzero formula toolchain identities in their existing order.
    pub formula_toolchain: [[u8; 32]; 5],
    /// Supported formula boundary, exactly 1, 2 or 3.
    pub formula_boundary: u8,
}
/// Exact source bytes and policies in the original source order.
#[derive(Clone, Copy, Debug)]
pub struct NativeConditionalPolicyRosterInputV1<'a> {
    /// Entire unchanged source packet, hashed with raw SHA256.
    pub source_packet: &'a [u8],
    /// One to 128 distinct source roots; never sorted by the encoder.
    pub roots: &'a [NativeConditionalPolicyRootInputV1<'a>],
}

/// Framing, canonical policy material or prepaid-work failure.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeConditionalPolicyRosterErrorV1<E = Infallible> {
    /// Original caller refusal.
    Charge(E),
    /// Source extent is empty or exceeds its independent cap.
    SourceLength,
    /// Root count is empty or exceeds its independent cap.
    RootCount,
    /// Effect signer count is outside 1..=4096.
    SignerCount,
    /// Root IDs repeat, regardless of their ordering.
    DuplicateRoot,
    /// Signers are not strictly increasing.
    SignerOrder,
    /// A required identity is all zero.
    ZeroIdentity,
    /// Formula boundary is not one of the supported three values.
    FormulaBoundary,
    /// Complete or declared extent is not exact.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Magic, version, policy or header extent differs.
    Header,
    /// Reserved bytes are nonzero.
    Reserved,
    /// Terminal digest differs.
    Identity,
    /// Unsupported shared storage ceiling.
    StorageLimit,
    /// Exact-capacity allocation failed.
    Allocation,
}
type Error<E> = NativeConditionalPolicyRosterErrorV1<E>;

/// Checked extent; construction visits only prepaid root descriptors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalPolicyRosterLayoutV1 {
    total: usize,
    source_len: usize,
    roots: usize,
}
impl NativeConditionalPolicyRosterLayoutV1 {
    /// Compute exact extents without hashing or visiting signer payloads.
    pub fn new<E>(
        input: NativeConditionalPolicyRosterInputV1<'_>,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        source_length(input.source_packet.len())?;
        root_count(input.roots.len())?;
        charge(
            input
                .roots
                .len()
                .checked_mul(size_of::<NativeConditionalPolicyRootInputV1<'_>>())
                .ok_or(Error::Arithmetic)?,
        )
        .map_err(Error::Charge)?;
        let mut total = HEADER + 32;
        for row in input.roots {
            total = total
                .checked_add(row_length(row.effect_signers.len())?)
                .ok_or(Error::Arithmetic)?;
        }
        Ok(Self {
            total,
            source_len: input.source_packet.len(),
            roots: input.roots.len(),
        })
    }
    /// Complete canonical byte count.
    pub const fn encoded_len(self) -> usize {
        self.total
    }
    /// Variable-sized rows, each fixed header followed by its signer identities.
    pub fn rows_range(self) -> Range<usize> {
        HEADER..self.total - 32
    }
    /// Number of source-ordered rows.
    pub const fn root_count(self) -> usize {
        self.roots
    }
}

/// Domain-separated exact roster identity, not accepted policy authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalPolicyRosterIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl NativeConditionalPolicyRosterIdentityV1 {
    /// Terminal roster digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete encoding length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// One validated row borrowing the original encoding, with no allocation.
#[derive(Clone, Copy, Debug)]
pub struct NativeConditionalPolicyRootRefV1<'a> {
    bytes: &'a [u8],
}
impl<'a> NativeConditionalPolicyRootRefV1<'a> {
    /// Original semantic function ID, potentially noncontiguous and out of numeric order.
    pub fn semantic_root(&self) -> u32 {
        u32::from_le_bytes(self.bytes[..4].try_into().expect("validated row"))
    }
    /// Claimed source kernel binding.
    pub fn kernel_binding(&self) -> &'a [u8; 32] {
        self.bytes[16..48].try_into().expect("validated row")
    }
    /// Sorted unique nonzero signer identities, borrowing the input.
    pub fn effect_signers(&self) -> &'a [[u8; 32]] {
        self.bytes[ROW..].as_chunks::<32>().0
    }
    /// Exact number of effect signer identities.
    pub fn effect_signer_count(&self) -> usize {
        self.effect_signers().len()
    }
    /// Five effect toolchain identities in their original field order.
    pub fn effect_toolchain(&self) -> &'a [[u8; 32]; 5] {
        self.bytes[48..208]
            .as_chunks::<32>()
            .0
            .try_into()
            .expect("validated row")
    }
    /// Formula key bytes; not cryptographic admission.
    pub fn formula_verifying_key(&self) -> &'a [u8; 32] {
        self.bytes[208..240].try_into().expect("validated row")
    }
    /// Five formula toolchain identities in their original field order.
    pub fn formula_toolchain(&self) -> &'a [[u8; 32]; 5] {
        self.bytes[240..400]
            .as_chunks::<32>()
            .0
            .try_into()
            .expect("validated row")
    }
    /// Supported boundary discriminator.
    pub const fn formula_boundary(&self) -> u8 {
        self.bytes[8]
    }
}

/// Fully checked inert roster borrowing one unchanged input.
pub struct NativeConditionalPolicyRosterRefV1<'a> {
    bytes: &'a [u8],
    layout: NativeConditionalPolicyRosterLayoutV1,
    identity: NativeConditionalPolicyRosterIdentityV1,
}
impl<'a> NativeConditionalPolicyRosterRefV1<'a> {
    /// Complete encoding in original custody.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Raw SHA256 of the exact source packet, without a digest domain.
    pub fn source_packet_sha256(&self) -> &'a [u8; 32] {
        self.bytes[32..64].try_into().expect("validated header")
    }
    /// Exact source packet length, including all of its framing.
    pub const fn source_packet_len(&self) -> u64 {
        self.layout.source_len as u64
    }
    /// Number of source-ordered policies.
    pub const fn root_count(&self) -> usize {
        self.layout.roots
    }
    /// Allocation-free row views. Callers prepay fixed row access work on subsequent walks.
    pub fn roots(&self) -> impl ExactSizeIterator<Item = NativeConditionalPolicyRootRefV1<'a>> {
        let mut rest = &self.bytes[self.layout.rows_range()];
        (0..self.layout.roots).map(move |_| {
            let (row, tail) = take_row::<Infallible>(rest).expect("immutable validated roster");
            rest = tail;
            row
        })
    }
    /// Domain-separated roster identity.
    pub const fn identity(&self) -> NativeConditionalPolicyRosterIdentityV1 {
        self.identity
    }
    /// Public policy bytes are never authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fixed working storage, including bounded root uniqueness scratch; backing and
/// the encoder's Vec header/capacity are separately prepaid by the caller.
pub const NATIVE_CONDITIONAL_POLICY_ROSTER_WORKING_STORAGE_V1: usize =
    size_of::<NativeConditionalPolicyRosterRefV1<'static>>()
        + size_of::<NativeConditionalPolicyRosterLayoutV1>()
        + size_of::<NativeConditionalPolicyRosterInputV1<'static>>()
        + size_of::<NativeConditionalPolicyRootRefV1<'static>>()
        + size_of::<[u32; ROOTS]>()
        + size_of::<Sha256>()
        + HEADER
        + ROW
        + 256;

fn source_length<E>(n: usize) -> Result<(), Error<E>> {
    if n == 0 || n > MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 {
        return Err(Error::SourceLength);
    }
    Ok(())
}
fn root_count<E>(n: usize) -> Result<(), Error<E>> {
    if n == 0 || n > ROOTS {
        return Err(Error::RootCount);
    }
    Ok(())
}
fn row_length<E>(n: usize) -> Result<usize, Error<E>> {
    if n == 0 || n > SIGNERS {
        return Err(Error::SignerCount);
    }
    n.checked_mul(32)
        .and_then(|n| n.checked_add(ROW))
        .ok_or(Error::Arithmetic)
}
fn ceiling<E>(n: usize) -> Result<(), Error<E>> {
    if n > MAX_NATIVE_CONDITIONAL_STORAGE_V1 {
        return Err(Error::StorageLimit);
    }
    Ok(())
}
fn word<E>(bytes: &[u8], at: usize) -> Result<usize, Error<E>> {
    let value = bytes
        .get(at..at + 8)
        .ok_or(Error::Length)?
        .try_into()
        .map_err(|_| Error::Length)?;
    usize::try_from(u64::from_le_bytes(value)).map_err(|_| Error::Arithmetic)
}
fn take_row<E>(bytes: &[u8]) -> Result<(NativeConditionalPolicyRootRefV1<'_>, &[u8]), Error<E>> {
    let header = bytes.get(..ROW).ok_or(Error::Length)?;
    let count = usize::try_from(u32::from_le_bytes(
        header[4..8].try_into().map_err(|_| Error::Length)?,
    ))
    .map_err(|_| Error::Arithmetic)?;
    let len = row_length(count)?;
    let row = bytes.get(..len).ok_or(Error::Length)?;
    Ok((
        NativeConditionalPolicyRootRefV1 { bytes: row },
        &bytes[len..],
    ))
}
fn validate_rows<E>(mut bytes: &[u8], count: usize) -> Result<(), Error<E>> {
    let mut seen = [0_u32; ROOTS];
    for index in 0..count {
        let (row, rest) = take_row(bytes)?;
        bytes = rest;
        if row.bytes[9..16] != [0; 7] {
            return Err(Error::Reserved);
        }
        if !(1..=3).contains(&row.formula_boundary()) {
            return Err(Error::FormulaBoundary);
        }
        if seen[..index].contains(&row.semantic_root()) {
            return Err(Error::DuplicateRoot);
        }
        seen[index] = row.semantic_root();
        for identity in std::iter::once(row.kernel_binding())
            .chain(row.effect_toolchain())
            .chain(std::iter::once(row.formula_verifying_key()))
            .chain(row.formula_toolchain())
            .chain(row.effect_signers())
        {
            if *identity == [0; 32] {
                return Err(Error::ZeroIdentity);
            }
        }
        if row
            .effect_signers()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::SignerOrder);
        }
    }
    if !bytes.is_empty() {
        return Err(Error::Length);
    }
    Ok(())
}
fn walk_work<E>(length: usize, count: usize) -> Result<usize, Error<E>> {
    // Identity scans and both sides of signer comparisons, plus bounded ID comparisons.
    let comparisons = count
        .checked_mul(count.saturating_sub(1))
        .and_then(|n| n.checked_mul(2))
        .ok_or(Error::Arithmetic)?;
    length
        .checked_mul(3)
        .and_then(|n| n.checked_add(comparisons))
        .and_then(|n| n.checked_add(size_of::<[u32; ROOTS]>()))
        .ok_or(Error::Arithmetic)
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    h.finalize().into()
}
fn seal_work<E>(layout: NativeConditionalPolicyRosterLayoutV1) -> Result<usize, Error<E>> {
    walk_work::<E>(layout.rows_range().len(), layout.roots)?
        .checked_add(layout.source_len)
        .and_then(|n| n.checked_add(128))
        .and_then(|n| n.checked_add(layout.total))
        .and_then(|n| n.checked_add(DOMAIN.len() + 8 + 128 + 2 * HEADER))
        .ok_or(Error::Arithmetic)
}

/// Seal prewritten rows, binding the exact source bytes. Validation, work refusal
/// and destruction of the caller callback all precede any destination mutation.
/// Caller prepays backing and the published working storage separately.
pub fn seal_native_conditional_policy_roster_v1<E>(
    layout: NativeConditionalPolicyRosterLayoutV1,
    source_packet: &[u8],
    bytes: &mut [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalPolicyRosterIdentityV1, Error<E>> {
    ceiling(storage_limit)?;
    if bytes.len() != layout.total {
        return Err(Error::Length);
    }
    if source_packet.len() != layout.source_len {
        return Err(Error::SourceLength);
    }
    charge(seal_work(layout)?).map_err(Error::Charge)?;
    drop(charge);
    validate_rows(&bytes[layout.rows_range()], layout.roots)?;
    let source_sha256 = Sha256::digest(source_packet);
    bytes[..HEADER].fill(0);
    bytes[..8].copy_from_slice(&NATIVE_CONDITIONAL_POLICY_ROSTER_MAGIC_V1);
    bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
    bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&(layout.total as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(layout.source_len as u64).to_le_bytes());
    bytes[32..64].copy_from_slice(&source_sha256);
    bytes[64..68].copy_from_slice(&(layout.roots as u32).to_le_bytes());
    let end = layout.total - 32;
    let sha256 = hash(&bytes[..end]);
    bytes[end..].copy_from_slice(&sha256);
    Ok(NativeConditionalPolicyRosterIdentityV1 {
        sha256,
        byte_len: bytes.len() as u64,
    })
}

/// Encode with exactly one exact-capacity allocation and no hidden input copies.
/// Caller prepays that Vec header/backing plus the fixed working-storage quote.
/// All walks, source hashing and writes are charged before they occur. Signer
/// order is checked, never repaired, and source-root order is preserved.
pub fn encode_native_conditional_policy_roster_v1<E>(
    input: NativeConditionalPolicyRosterInputV1<'_>,
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<u8>, Error<E>> {
    ceiling(storage_limit)?;
    let layout = NativeConditionalPolicyRosterLayoutV1::new(input, &mut charge)?;
    let descriptor_work = input
        .roots
        .len()
        .checked_mul(size_of::<NativeConditionalPolicyRootInputV1<'_>>())
        .ok_or(Error::Arithmetic)?;
    let sealing_work = seal_work::<E>(layout)?;
    let work = layout
        .total
        .checked_mul(3)
        .and_then(|n| n.checked_add(descriptor_work))
        .and_then(|n| n.checked_add(sealing_work))
        .ok_or(Error::Arithmetic)?;
    charge(work).map_err(Error::Charge)?;
    drop(charge);
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(layout.total)
        .map_err(|_| Error::Allocation)?;
    if bytes.capacity() != layout.total {
        return Err(Error::Allocation);
    }
    bytes.resize(layout.total, 0);
    let mut at = HEADER;
    for row in input.roots {
        bytes[at..at + 4].copy_from_slice(&row.semantic_root.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&(row.effect_signers.len() as u32).to_le_bytes());
        bytes[at + 8] = row.formula_boundary;
        let mut cursor = at + 16;
        for identity in std::iter::once(&row.kernel_binding)
            .chain(&row.effect_toolchain)
            .chain(std::iter::once(&row.formula_verifying_key))
            .chain(&row.formula_toolchain)
            .chain(row.effect_signers)
        {
            bytes[cursor..cursor + 32].copy_from_slice(identity);
            cursor += 32;
        }
        at = cursor;
    }
    seal_native_conditional_policy_roster_v1(
        layout,
        input.source_packet,
        &mut bytes,
        storage_limit,
        |_| Ok::<_, E>(()),
    )?;
    Ok(bytes)
}

/// Strict allocation-free framing, identity and row canonicality checks.
/// Source identity association and actual policy acceptance are separate checks.
pub fn read_native_conditional_policy_roster_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalPolicyRosterRefV1<'a>, Error<E>> {
    ceiling(storage_limit)?;
    if !(HEADER + ROW + 64..=MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_BYTES_V1).contains(&bytes.len()) {
        return Err(Error::Length);
    }
    charge(HEADER).map_err(Error::Charge)?;
    if bytes[..8] != NATIVE_CONDITIONAL_POLICY_ROSTER_MAGIC_V1
        || bytes[8..12] != [1, 0, 1, 0]
        || bytes[12..16] != (HEADER as u32).to_le_bytes()
    {
        return Err(Error::Header);
    }
    if bytes[68..HEADER] != [0; 12] {
        return Err(Error::Reserved);
    }
    if word(bytes, 16)? != bytes.len() {
        return Err(Error::Length);
    }
    let source_len = word(bytes, 24)?;
    source_length(source_len)?;
    let roots = usize::try_from(u32::from_le_bytes(
        bytes[64..68].try_into().map_err(|_| Error::Length)?,
    ))
    .map_err(|_| Error::Arithmetic)?;
    root_count(roots)?;
    let layout = NativeConditionalPolicyRosterLayoutV1 {
        total: bytes.len(),
        source_len,
        roots,
    };
    charge(walk_work(layout.rows_range().len(), roots)?).map_err(Error::Charge)?;
    validate_rows(&bytes[layout.rows_range()], roots)?;
    let end = bytes.len() - 32;
    charge(
        bytes
            .len()
            .checked_add(DOMAIN.len() + 8 + 128)
            .ok_or(Error::Arithmetic)?,
    )
    .map_err(Error::Charge)?;
    let sha256 = hash(&bytes[..end]);
    if bytes[end..] != sha256 {
        return Err(Error::Identity);
    }
    Ok(NativeConditionalPolicyRosterRefV1 {
        bytes,
        layout,
        identity: NativeConditionalPolicyRosterIdentityV1 {
            sha256,
            byte_len: bytes.len() as u64,
        },
    })
}
