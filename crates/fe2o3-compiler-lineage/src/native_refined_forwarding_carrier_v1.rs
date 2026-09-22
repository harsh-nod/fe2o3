//! Bounded paired transport only. Nested source/output admission remains mandatory.
use crate::bounded_pair::{self, HEADER};
use sha2::Sha256;
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
const POLICY: bounded_pair::Policy = bounded_pair::Policy {
    magic: NATIVE_REFINED_FORWARDING_CARRIER_MAGIC_V1,
    version: 1,
    domain: b"FE2O3/NATIVE-REFINED-FORWARDING-CARRIER/V1\0",
    first_max: MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1,
    second_max: MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1,
    total_max: MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1,
    storage_max: MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1,
};

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

impl<E> From<bounded_pair::Error<E>> for Error<E> {
    fn from(error: bounded_pair::Error<E>) -> Self {
        use bounded_pair::Error as Pair;
        match error {
            Pair::Charge(e) => Self::Charge(e),
            Pair::FirstLength => Self::OutputLength,
            Pair::SecondLength => Self::SourceLength,
            Pair::Length => Self::Length,
            Pair::Arithmetic => Self::Arithmetic,
            Pair::Header => Self::Header,
            Pair::Reserved => Self::Reserved,
            Pair::Identity => Self::Identity,
            Pair::StorageLimit => Self::StorageLimit,
        }
    }
}

/// Checked disjoint ranges for encoding directly into one caller-owned allocation.
/// This describes byte extents, not validity of either nested constituent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRefinedForwardingCarrierLayoutV1(bounded_pair::Layout);
impl NativeRefinedForwardingCarrierLayoutV1 {
    /// Checks both mandatory field limits before any payload access or allocation.
    pub fn new<E>(output_len: usize, source_len: usize) -> Result<Self, Error<E>> {
        Ok(Self(bounded_pair::Layout::new(
            &POLICY, output_len, source_len,
        )?))
    }
    /// Exact complete allocation length, including header and terminal digest.
    pub const fn encoded_len(self) -> usize {
        self.0.encoded_len()
    }
    /// Region into which the existing F2RFO1 encoder writes unchanged output.
    pub fn output_range(self) -> Range<usize> {
        self.0.first_range()
    }
    /// Region retaining the exact complete F2NSRC1 packet.
    pub fn source_range(self) -> Range<usize> {
        self.0.second_range()
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

/// Writes the fixed header and terminal digest around already encoded payloads.
/// It does not copy or interpret either payload, so the existing output encoder
/// can write directly into `layout.output_range()` without a second allocation.
/// Caller must prepay payload writes separately. Every seal debit precedes any
/// mutation; an error or callback refusal leaves the complete buffer unchanged.
pub fn seal_native_refined_forwarding_carrier_v1<E>(
    layout: NativeRefinedForwardingCarrierLayoutV1,
    bytes: &mut [u8],
    storage_limit: usize,
    charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeRefinedForwardingCarrierIdentityV1, Error<E>> {
    let identity = bounded_pair::seal(&POLICY, layout.0, bytes, storage_limit, charge_work)?;
    Ok(NativeRefinedForwardingCarrierIdentityV1 {
        sha256: identity.sha256,
        byte_len: identity.byte_len,
    })
}

/// Strict allocation-free read with no legacy fallback. Header and hash visits
/// are prepaid before inspecting their bytes; independently bounded constituents
/// remain borrowed and must be admitted by the existing complete verifier.
pub fn read_native_refined_forwarding_carrier_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeRefinedForwardingCarrierRefV1<'a>, Error<E>> {
    let (layout, identity) = bounded_pair::read(&POLICY, bytes, storage_limit, charge_work)?;
    Ok(NativeRefinedForwardingCarrierRefV1 {
        bytes,
        layout: NativeRefinedForwardingCarrierLayoutV1(layout),
        identity: NativeRefinedForwardingCarrierIdentityV1 {
            sha256: identity.sha256,
            byte_len: identity.byte_len,
        },
    })
}

#[cfg(test)]
#[path = "native_refined_forwarding_carrier_v1_tests.rs"]
mod tests;
