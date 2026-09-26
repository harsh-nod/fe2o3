//! Six inert preimages; no ordinary target/proof transcript or FFI dependency.
use crate::{
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 as RECEIPT_MAX, MAX_NATIVE_CONDITIONAL_STORAGE_V1,
    NATIVE_LOWERING_ASSOCIATION_BYTES_V1,
};
use sha2::{Digest, Sha256};
use std::{convert::Infallible, mem::size_of, ops::Range};

/// Closed metadata discriminator.
pub const NATIVE_CONDITIONAL_METADATA_MAGIC_V1: [u8; 8] = *b"F2NCM1\0\0";
/// Fixed header including all six field lengths.
pub const NATIVE_CONDITIONAL_METADATA_HEADER_BYTES_V1: usize = 80;
const HEADER: usize = NATIVE_CONDITIONAL_METADATA_HEADER_BYTES_V1;
const DOMAIN: &[u8] = b"FE2O3/NATIVE-CONDITIONAL-METADATA/V1\0";
const MAXIMA: [usize; 6] = [
    fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3,
    RECEIPT_MAX,
    RECEIPT_MAX,
    RECEIPT_MAX,
    NATIVE_LOWERING_ASSOCIATION_BYTES_V1,
    RECEIPT_MAX,
];
/// All six maxima plus header and digest; final commitment has tighter FFI validation.
pub const MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V1: usize = HEADER
    + 32
    + fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3
    + 4 * RECEIPT_MAX
    + NATIVE_LOWERING_ASSOCIATION_BYTES_V1;

/// Existing codec preimages, borrowed without claiming their authenticity.
#[derive(Clone, Copy)]
pub struct NativeConditionalMetadataInputV1<'a> {
    /// Canonical invocation V3 including compiler closure.
    pub invocation: &'a [u8],
    /// Original inventory receipt preimage.
    pub rustc_inventory: &'a [u8],
    /// Original preflight receipt preimage.
    pub rustc_preflight: &'a [u8],
    /// Existing semantic-target-layout transcript, not an ordinary target binding.
    pub semantic_target_layout: &'a [u8],
    /// Unchanged fixed native lowering association V1.
    pub native_lowering: &'a [u8],
    /// Unchanged final-module commitment V3; only FFI decodes its schema.
    pub final_module_commitment: &'a [u8],
}
impl NativeConditionalMetadataInputV1<'_> {
    fn lengths(self) -> [usize; 6] {
        [
            self.invocation.len(),
            self.rustc_inventory.len(),
            self.rustc_preflight.len(),
            self.semantic_target_layout.len(),
            self.native_lowering.len(),
            self.final_module_commitment.len(),
        ]
    }
}

/// Framing, original work refusal or structural target-layout preimage error.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeConditionalMetadataErrorV1<E = Infallible> {
    /// Caller work refusal.
    Charge(E),
    /// A mandatory field violates its independent bound or fixed size.
    FieldLength,
    /// Complete or declared length is not exact.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Magic, version, policy, count or header extent differs.
    Header,
    /// Reserved bytes are nonzero.
    Reserved,
    /// Terminal digest differs.
    Identity,
    /// Unsupported shared storage ceiling.
    StorageLimit,
    /// Existing six-field layout preimage is malformed.
    TargetLayout,
}
type Error<E> = NativeConditionalMetadataErrorV1<E>;

/// Fixed disjoint field ranges; construction does not visit payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalMetadataLayoutV1 {
    lengths: [usize; 6],
    total: usize,
}
impl NativeConditionalMetadataLayoutV1 {
    /// Check all bounded mandatory extents before allocation or traversal.
    pub fn new<E>(input: NativeConditionalMetadataInputV1<'_>) -> Result<Self, Error<E>> {
        Self::from_lengths(input.lengths())
    }
    fn from_lengths<E>(lengths: [usize; 6]) -> Result<Self, Error<E>> {
        let mut total = HEADER + 32;
        for (n, max) in lengths.into_iter().zip(MAXIMA) {
            if n == 0 || n > max {
                return Err(Error::FieldLength);
            }
            total = total.checked_add(n).ok_or(Error::Arithmetic)?;
        }
        if lengths[4] != NATIVE_LOWERING_ASSOCIATION_BYTES_V1 {
            return Err(Error::FieldLength);
        }
        Ok(Self { lengths, total })
    }
    /// Complete canonical byte count.
    pub const fn encoded_len(self) -> usize {
        self.total
    }
    fn field(self, index: usize) -> Range<usize> {
        let start = HEADER + self.lengths[..index].iter().sum::<usize>();
        start..start + self.lengths[index]
    }
    /// Canonical invocation bytes.
    pub fn invocation_range(self) -> Range<usize> {
        self.field(0)
    }
    /// Original inventory preimage.
    pub fn rustc_inventory_range(self) -> Range<usize> {
        self.field(1)
    }
    /// Original preflight preimage.
    pub fn rustc_preflight_range(self) -> Range<usize> {
        self.field(2)
    }
    /// Existing layout preimage.
    pub fn semantic_target_layout_range(self) -> Range<usize> {
        self.field(3)
    }
    /// Fixed native lowering association.
    pub fn native_lowering_range(self) -> Range<usize> {
        self.field(4)
    }
    /// Final-module commitment preimage, decoded by FFI.
    pub fn final_module_commitment_range(self) -> Range<usize> {
        self.field(5)
    }
}

/// Domain-separated exact metadata identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalMetadataIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl NativeConditionalMetadataIdentityV1 {
    /// Terminal digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete length including digest.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Borrowed fields of the existing semantic-target-layout preimage, not live facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalTargetLayoutRefV1<'a> {
    /// Claimed original LLVM target.
    pub rustc_llvm_target: &'a str,
    /// Claimed original rustc layout.
    pub live_rustc_data_layout: &'a str,
    /// Claimed original pointer width.
    pub default_pointer_width_bits: u16,
    /// Claimed target CPU.
    pub target_cpu: &'a str,
    /// Claimed original feature text.
    pub target_features: &'a str,
}
fn target_layout<E>(bytes: &[u8]) -> Result<NativeConditionalTargetLayoutRefV1<'_>, Error<E>> {
    let mut fields = [&[][..]; 6];
    let mut rest = bytes;
    for field in &mut fields {
        let word: [u8; 8] = rest
            .get(..8)
            .ok_or(Error::TargetLayout)?
            .try_into()
            .map_err(|_| Error::TargetLayout)?;
        let n = usize::try_from(u64::from_le_bytes(word)).map_err(|_| Error::Arithmetic)?;
        rest = &rest[8..];
        *field = rest.get(..n).ok_or(Error::TargetLayout)?;
        rest = &rest[n..];
    }
    if !rest.is_empty() || fields[0] != b"fe2o3/semantic-mir/rustc-target-layout/v1" {
        return Err(Error::TargetLayout);
    }
    let text = |n: usize| std::str::from_utf8(fields[n]).map_err(|_| Error::TargetLayout);
    let bits = u16::from_le_bytes(fields[3].try_into().map_err(|_| Error::TargetLayout)?);
    Ok(NativeConditionalTargetLayoutRefV1 {
        rustc_llvm_target: text(1)?,
        live_rustc_data_layout: text(2)?,
        default_pointer_width_bits: bits,
        target_cpu: text(4)?,
        target_features: text(5)?,
    })
}

/// Checked metadata framing; invocation and receipt admission remain separate.
pub struct NativeConditionalMetadataRefV1<'a> {
    bytes: &'a [u8],
    pub(crate) layout: NativeConditionalMetadataLayoutV1,
    identity: NativeConditionalMetadataIdentityV1,
    target_layout: NativeConditionalTargetLayoutRefV1<'a>,
}
impl<'a> NativeConditionalMetadataRefV1<'a> {
    /// Complete encoding.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Original invocation encoding.
    pub fn invocation(&self) -> &'a [u8] {
        &self.bytes[self.layout.invocation_range()]
    }
    /// Original inventory preimage.
    pub fn rustc_inventory(&self) -> &'a [u8] {
        &self.bytes[self.layout.rustc_inventory_range()]
    }
    /// Original preflight preimage.
    pub fn rustc_preflight(&self) -> &'a [u8] {
        &self.bytes[self.layout.rustc_preflight_range()]
    }
    /// Unchanged target-layout transcript.
    pub fn semantic_target_layout_bytes(&self) -> &'a [u8] {
        &self.bytes[self.layout.semantic_target_layout_range()]
    }
    /// Structurally decoded, still inert target values.
    pub const fn semantic_target_layout(&self) -> NativeConditionalTargetLayoutRefV1<'a> {
        self.target_layout
    }
    /// Unchanged lowering association bytes.
    pub fn native_lowering(&self) -> &'a [u8] {
        &self.bytes[self.layout.native_lowering_range()]
    }
    /// Unchanged final-module commitment bytes.
    pub fn final_module_commitment(&self) -> &'a [u8] {
        &self.bytes[self.layout.final_module_commitment_range()]
    }
    /// Exact metadata identity.
    pub const fn identity(&self) -> NativeConditionalMetadataIdentityV1 {
        self.identity
    }
    /// Public content does not grant authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fixed simultaneous metadata/hash/directory working set, excluding backing.
pub const NATIVE_CONDITIONAL_METADATA_WORKING_STORAGE_V1: usize =
    size_of::<NativeConditionalMetadataRefV1<'static>>()
        + size_of::<NativeConditionalMetadataLayoutV1>()
        + size_of::<Sha256>()
        + 2 * size_of::<[&[u8]; 6]>()
        + HEADER
        + 256;
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    h.finalize().into()
}
fn ceiling<E>(n: usize) -> Result<(), Error<E>> {
    if n > MAX_NATIVE_CONDITIONAL_STORAGE_V1 {
        return Err(Error::StorageLimit);
    }
    Ok(())
}

/// Seal framing only. All work and callback destruction precede mutation;
/// caller writes and prepays payloads separately through the named ranges.
pub fn seal_native_conditional_metadata_v1<E>(
    layout: NativeConditionalMetadataLayoutV1,
    bytes: &mut [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalMetadataIdentityV1, Error<E>> {
    ceiling(storage_limit)?;
    if bytes.len() != layout.total {
        return Err(Error::Length);
    }
    let end = layout.total - 32;
    charge(end + DOMAIN.len() + 8 + 128 + 2 * HEADER + 32).map_err(Error::Charge)?;
    drop(charge);
    bytes[..HEADER].fill(0);
    bytes[..8].copy_from_slice(&NATIVE_CONDITIONAL_METADATA_MAGIC_V1);
    bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
    bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&(layout.total as u64).to_le_bytes());
    bytes[24..26].copy_from_slice(&6_u16.to_le_bytes());
    for (slot, n) in bytes[32..HEADER].chunks_exact_mut(8).zip(layout.lengths) {
        slot.copy_from_slice(&(n as u64).to_le_bytes());
    }
    let sha256 = hash(&bytes[..end]);
    bytes[end..].copy_from_slice(&sha256);
    Ok(NativeConditionalMetadataIdentityV1 {
        sha256,
        byte_len: layout.total as u64,
    })
}

/// Allocation-free header/hash/layout-preimage checks, with work before each visit.
pub fn read_native_conditional_metadata_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalMetadataRefV1<'a>, Error<E>> {
    ceiling(storage_limit)?;
    if !(HEADER + 32..=MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V1).contains(&bytes.len()) {
        return Err(Error::Length);
    }
    charge(HEADER).map_err(Error::Charge)?;
    if bytes[..8] != NATIVE_CONDITIONAL_METADATA_MAGIC_V1
        || bytes[8..12] != [1, 0, 1, 0]
        || bytes[12..16] != (HEADER as u32).to_le_bytes()
        || bytes[24..26] != [6, 0]
    {
        return Err(Error::Header);
    }
    if bytes[26..32] != [0; 6] {
        return Err(Error::Reserved);
    }
    let word = |at: usize| -> Result<usize, Error<E>> {
        usize::try_from(u64::from_le_bytes(
            bytes[at..at + 8].try_into().map_err(|_| Error::Length)?,
        ))
        .map_err(|_| Error::Arithmetic)
    };
    let mut lengths = [0; 6];
    for (i, n) in lengths.iter_mut().enumerate() {
        *n = word(32 + i * 8)?;
    }
    let layout = NativeConditionalMetadataLayoutV1::from_lengths(lengths)?;
    if word(16)? != bytes.len() || layout.total != bytes.len() {
        return Err(Error::Length);
    }
    let end = bytes.len() - 32;
    charge(end + DOMAIN.len() + 8 + 128 + 32).map_err(Error::Charge)?;
    let sha256 = hash(&bytes[..end]);
    if bytes[end..] != sha256 {
        return Err(Error::Identity);
    }
    charge(lengths[3] + 6 * 8 + 128).map_err(Error::Charge)?;
    let target_layout = target_layout(&bytes[layout.semantic_target_layout_range()])?;
    Ok(NativeConditionalMetadataRefV1 {
        bytes,
        layout,
        target_layout,
        identity: NativeConditionalMetadataIdentityV1 {
            sha256,
            byte_len: bytes.len() as u64,
        },
    })
}
