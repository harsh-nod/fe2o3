//! Distinct conditional source/output roles over the existing pair engine.
use crate::{bounded_pair as pair, native_conditional_output_v1::*};
use sha2::Sha256;
use std::{mem::size_of, ops::Range};

/// Conditional carrier discriminator; F2NRF1 keeps its original meaning.
pub const NATIVE_CONDITIONAL_CARRIER_MAGIC_V1: [u8; 8] = *b"F2NCC1\0\0";
/// Fixed carrier pair header size.
pub const NATIVE_CONDITIONAL_CARRIER_HEADER_BYTES_V1: usize = pair::HEADER;
/// Existing complete conditional source packet ceiling.
pub const MAX_NATIVE_CONDITIONAL_SOURCE_BYTES_V1: usize = 4 * 1024 * 1024;
/// Both complete fields and all pair framing.
pub const MAX_NATIVE_CONDITIONAL_CARRIER_BYTES_V1: usize = pair::OVERHEAD
    + MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1
    + MAX_NATIVE_CONDITIONAL_SOURCE_BYTES_V1;
const POLICY: pair::Policy = pair::Policy {
    magic: NATIVE_CONDITIONAL_CARRIER_MAGIC_V1,
    version: 1,
    domain: b"FE2O3/NATIVE-CONDITIONAL-CARRIER/V1\0",
    first_max: MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1,
    second_max: MAX_NATIVE_CONDITIONAL_SOURCE_BYTES_V1,
    total_max: MAX_NATIVE_CONDITIONAL_CARRIER_BYTES_V1,
    storage_max: MAX_NATIVE_CONDITIONAL_STORAGE_V1,
};

/// Framing or prepaid-work failure; never retried as an ordinary carrier.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeConditionalCarrierErrorV1<E> {
    /// Caller work refusal.
    Charge(E),
    /// Invalid output extent.
    OutputLength,
    /// Invalid source extent.
    SourceLength,
    /// Invalid aggregate or exact length.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Unsupported fixed header.
    Header,
    /// Nonzero reserved field.
    Reserved,
    /// Content hash mismatch.
    Identity,
    /// Unsupported shared storage ceiling.
    StorageLimit,
    /// Mandatory conditional output framing failed.
    Output(NativeConditionalOutputErrorV1<E>),
}
type Error<E> = NativeConditionalCarrierErrorV1<E>;
impl<E> From<pair::Error<E>> for Error<E> {
    fn from(e: pair::Error<E>) -> Self {
        match e {
            pair::Error::Charge(e) => Self::Charge(e),
            pair::Error::FirstLength => Self::OutputLength,
            pair::Error::SecondLength => Self::SourceLength,
            pair::Error::Length => Self::Length,
            pair::Error::Arithmetic => Self::Arithmetic,
            pair::Error::Header => Self::Header,
            pair::Error::Reserved => Self::Reserved,
            pair::Error::Identity => Self::Identity,
            pair::Error::StorageLimit => Self::StorageLimit,
        }
    }
}

/// Exact disjoint output/source ranges, without payload admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalCarrierLayoutV1(pair::Layout);
impl NativeConditionalCarrierLayoutV1 {
    /// Checks both independent bounds and complete framing extent.
    pub fn new<E>(output: usize, source: usize) -> Result<Self, Error<E>> {
        Ok(Self(pair::Layout::new(&POLICY, output, source)?))
    }
    /// Complete carrier extent.
    pub const fn encoded_len(self) -> usize {
        self.0.encoded_len()
    }
    /// Exact conditional output field.
    pub fn output_range(self) -> Range<usize> {
        self.0.first_range()
    }
    /// Exact unchanged conditional source V2 field.
    pub fn source_range(self) -> Range<usize> {
        self.0.second_range()
    }
}

/// Domain-separated identity of the conditional pair, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalCarrierIdentityV1(pair::Identity);
impl NativeConditionalCarrierIdentityV1 {
    /// Terminal digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.0.sha256
    }
    /// Complete carrier length.
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
}

/// Recursive framing view retaining the original immutable input borrow.
pub struct NativeConditionalCarrierRefV1<'a> {
    bytes: &'a [u8],
    pub(crate) layout: NativeConditionalCarrierLayoutV1,
    output: NativeConditionalOutputRefV1<'a>,
    identity: NativeConditionalCarrierIdentityV1,
}
impl<'a> NativeConditionalCarrierRefV1<'a> {
    /// Complete unchanged carrier encoding.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Framed output, not an admitted history or descriptor.
    pub const fn output(&self) -> &NativeConditionalOutputRefV1<'a> {
        &self.output
    }
    /// Unchanged source V2 bytes; genuine source replay remains required.
    pub fn source_packet(&self) -> &'a [u8] {
        &self.bytes[self.layout.source_range()]
    }
    /// Complete conditional pair identity.
    pub const fn identity(&self) -> NativeConditionalCarrierIdentityV1 {
        self.identity
    }
    /// Framing grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fixed simultaneous logical scratch; complete backing is separate.
pub const NATIVE_CONDITIONAL_CARRIER_WORKING_STORAGE_V1: usize =
    size_of::<NativeConditionalCarrierRefV1<'static>>()
        + size_of::<NativeConditionalCarrierLayoutV1>()
        + size_of::<Sha256>()
        + NATIVE_CONDITIONAL_OUTPUT_WORKING_STORAGE_V1
        + pair::HEADER
        + 128;

/// Seals only this pair around already written payloads. Does not reseal output.
/// Work and caller closure destruction precede mutation; payload writes are separate.
pub fn seal_native_conditional_carrier_v1<E>(
    layout: NativeConditionalCarrierLayoutV1,
    bytes: &mut [u8],
    storage_limit: usize,
    charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalCarrierIdentityV1, Error<E>> {
    Ok(NativeConditionalCarrierIdentityV1(pair::seal(
        &POLICY,
        layout.0,
        bytes,
        storage_limit,
        charge,
    )?))
}

/// Strict allocation-free pair/output framing read. No ordinary schema retry.
pub fn read_native_conditional_carrier_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalCarrierRefV1<'a>, Error<E>> {
    let (layout, identity) = pair::read(&POLICY, bytes, storage_limit, &mut charge)?;
    let output =
        read_native_conditional_output_v1(&bytes[layout.first_range()], storage_limit, charge)
            .map_err(Error::Output)?;
    Ok(NativeConditionalCarrierRefV1 {
        bytes,
        layout: NativeConditionalCarrierLayoutV1(layout),
        output,
        identity: NativeConditionalCarrierIdentityV1(identity),
    })
}
