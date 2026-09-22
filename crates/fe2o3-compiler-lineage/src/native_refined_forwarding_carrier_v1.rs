//! Bounded paired transport only. Nested source/output admission remains mandatory.
use sha2::{Digest, Sha256};
use std::{mem::size_of, ops::Range};

/// Discriminator of the paired native refined-forwarding carrier.
pub const NATIVE_REFINED_FORWARDING_CARRIER_MAGIC_V1: [u8; 8] = *b"F2NRF1\0\0";
/// Unchanged maximum complete F2RFO1 output, including its repeated fields.
pub const MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Unchanged maximum complete F2NSRC1 source packet.
pub const MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1: usize = 4 * 1024 * 1024;
/// Complete carrier bound, not the historical 4 MiB proof-binding receipt bound.
pub const MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1: usize = HEADER
    + MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1
    + MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1
    + 32;
/// Maximum configured shared verification storage, including caller siblings.
pub const MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1: usize = 256 * 1024 * 1024;
const HEADER: usize = 48;
const DOMAIN: &[u8] = b"FE2O3/NATIVE-REFINED-FORWARDING-CARRIER/V1\0";

/// Typed framing or caller work refusal. No failure is retried as a legacy format.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeRefinedForwardingCarrierErrorV1<E> {
    /// The caller refused a prepaid visit; the original error is preserved.
    Charge(E),
    /// Output is empty or exceeds its independent bound.
    OutputLength,
    /// Source is empty or exceeds its independent bound.
    SourceLength,
    /// The complete buffer length is not exact.
    Length,
    /// A length computation overflowed the host address space.
    Arithmetic,
    /// The version, policy, magic or header extent is unsupported.
    Header,
    /// Reserved header bytes are nonzero.
    Reserved,
    /// The terminal domain-separated identity does not match the content.
    Identity,
    /// The configured shared storage ceiling exceeds the supported policy.
    StorageLimit,
}
type Error<E> = NativeRefinedForwardingCarrierErrorV1<E>;

/// Checked disjoint ranges for encoding directly into one caller-owned allocation.
/// This describes byte extents, not validity of either nested constituent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRefinedForwardingCarrierLayoutV1 {
    output_end: usize,
    payload_end: usize,
}
impl NativeRefinedForwardingCarrierLayoutV1 {
    /// Checks both mandatory field limits before any payload access or allocation.
    pub fn new<E>(output_len: usize, source_len: usize) -> Result<Self, Error<E>> {
        if output_len == 0 || output_len > MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1 {
            return Err(Error::OutputLength);
        }
        if source_len == 0 || source_len > MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 {
            return Err(Error::SourceLength);
        }
        let output_end = HEADER.checked_add(output_len).ok_or(Error::Arithmetic)?;
        let payload_end = output_end
            .checked_add(source_len)
            .ok_or(Error::Arithmetic)?;
        payload_end.checked_add(32).ok_or(Error::Arithmetic)?;
        Ok(Self {
            output_end,
            payload_end,
        })
    }
    /// Exact complete allocation length, including header and terminal digest.
    pub const fn encoded_len(self) -> usize {
        self.payload_end + 32
    }
    /// Region into which the existing F2RFO1 encoder writes unchanged output.
    pub fn output_range(self) -> Range<usize> {
        HEADER..self.output_end
    }
    /// Region retaining the exact complete F2NSRC1 packet.
    pub fn source_range(self) -> Range<usize> {
        self.output_end..self.payload_end
    }
}

/// Content identity binding both fields and their ordered lengths, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRefinedForwardingCarrierIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl NativeRefinedForwardingCarrierIdentityV1 {
    /// Domain-separated terminal digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete canonical carrier length, including the terminal digest.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Checked framing and hash borrowing the complete immutable transport.
/// No nested graph, signature, ABI, target or execution claim is admitted here.
///
/// ```compile_fail
/// use fe2o3_compiler_lineage::NativeRefinedForwardingCarrierRefV1 as Carrier;
/// fn escape<'a>(value: Carrier<'a>) -> Carrier<'static> { value }
/// ```
pub struct NativeRefinedForwardingCarrierRefV1<'a> {
    bytes: &'a [u8],
    layout: NativeRefinedForwardingCarrierLayoutV1,
    identity: NativeRefinedForwardingCarrierIdentityV1,
}
impl<'a> NativeRefinedForwardingCarrierRefV1<'a> {
    /// Complete canonical carrier, backed by the original input allocation.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Exact unchanged output field; its semantic checker must still run.
    pub fn output(&self) -> &'a [u8] {
        &self.bytes[self.layout.output_range()]
    }
    /// Exact unchanged source field; its signed source checker must still run.
    pub fn source_packet(&self) -> &'a [u8] {
        &self.bytes[self.layout.source_range()]
    }
    /// Identity of this exact pair, not just its output field.
    pub const fn identity(&self) -> NativeRefinedForwardingCarrierIdentityV1 {
        self.identity
    }
    /// Framing and public content hashes never grant compiler or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Conservative fixed logical working extent for seal/read, including the
/// returned view, layout, hasher, header and digest/scalar scratch. Reserve this
/// and all live input/output backing on the caller's same ledger before use.
/// The scalar ceiling and work callback cannot establish reservation provenance.
pub const NATIVE_REFINED_FORWARDING_CARRIER_WORKING_STORAGE_V1: usize =
    size_of::<NativeRefinedForwardingCarrierRefV1<'static>>()
        + size_of::<NativeRefinedForwardingCarrierLayoutV1>()
        + size_of::<Sha256>()
        + HEADER
        + 128;

fn ceiling<E>(storage_limit: usize) -> Result<(), Error<E>> {
    if storage_limit > MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1 {
        return Err(Error::StorageLimit);
    }
    Ok(())
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn hash_work<E>(length: usize) -> Result<usize, Error<E>> {
    length
        .checked_add(DOMAIN.len() + 8 + 128)
        .ok_or(Error::Arithmetic)
}

/// Writes the fixed header and terminal digest around already encoded payloads.
/// It does not copy or interpret either payload, so the existing output encoder
/// can write directly into `layout.output_range()` without a second allocation.
/// Caller must prepay payload writes separately. Every seal debit precedes any
/// mutation; an error or callback refusal leaves the complete buffer unchanged.
pub fn seal_native_refined_forwarding_carrier_v1<E>(
    layout: NativeRefinedForwardingCarrierLayoutV1,
    bytes: &mut [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeRefinedForwardingCarrierIdentityV1, Error<E>> {
    ceiling(storage_limit)?;
    if bytes.len() != layout.encoded_len() {
        return Err(Error::Length);
    }
    let work = hash_work::<E>(layout.payload_end)?
        .checked_add(2 * HEADER + 32)
        .ok_or(Error::Arithmetic)?;
    charge_work(work).map_err(Error::Charge)?;
    bytes[..HEADER].fill(0);
    bytes[..8].copy_from_slice(&NATIVE_REFINED_FORWARDING_CARRIER_MAGIC_V1);
    bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
    bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&(layout.encoded_len() as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&((layout.output_end - HEADER) as u64).to_le_bytes());
    bytes[32..40].copy_from_slice(&((layout.payload_end - layout.output_end) as u64).to_le_bytes());
    let sha256 = hash(&bytes[..layout.payload_end]);
    bytes[layout.payload_end..].copy_from_slice(&sha256);
    Ok(NativeRefinedForwardingCarrierIdentityV1 {
        sha256,
        byte_len: layout.encoded_len() as u64,
    })
}

/// Strict allocation-free read with no legacy fallback. Header and hash visits
/// are prepaid before inspecting their bytes; independently bounded constituents
/// remain borrowed and must be admitted by the existing complete verifier.
pub fn read_native_refined_forwarding_carrier_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeRefinedForwardingCarrierRefV1<'a>, Error<E>> {
    ceiling(storage_limit)?;
    if bytes.len() < HEADER + 32 + 2 || bytes.len() > MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1
    {
        return Err(Error::Length);
    }
    charge_work(HEADER).map_err(Error::Charge)?;
    if bytes[..8] != NATIVE_REFINED_FORWARDING_CARRIER_MAGIC_V1
        || bytes[8..12] != [1, 0, 1, 0]
        || bytes[12..16] != (HEADER as u32).to_le_bytes()
    {
        return Err(Error::Header);
    }
    if bytes[40..HEADER] != [0; 8] {
        return Err(Error::Reserved);
    }
    let word = |offset: usize| -> Result<usize, Error<E>> {
        let array = bytes[offset..offset + 8]
            .try_into()
            .map_err(|_| Error::Length)?;
        usize::try_from(u64::from_le_bytes(array)).map_err(|_| Error::Arithmetic)
    };
    let layout = NativeRefinedForwardingCarrierLayoutV1::new(word(24)?, word(32)?)?;
    if word(16)? != bytes.len() || layout.encoded_len() != bytes.len() {
        return Err(Error::Length);
    }
    charge_work(hash_work::<E>(layout.payload_end)? + 32).map_err(Error::Charge)?;
    let sha256 = hash(&bytes[..layout.payload_end]);
    if bytes[layout.payload_end..] != sha256 {
        return Err(Error::Identity);
    }
    Ok(NativeRefinedForwardingCarrierRefV1 {
        bytes,
        layout,
        identity: NativeRefinedForwardingCarrierIdentityV1 {
            sha256,
            byte_len: bytes.len() as u64,
        },
    })
}

#[cfg(test)]
#[path = "native_refined_forwarding_carrier_v1_tests.rs"]
mod tests;
