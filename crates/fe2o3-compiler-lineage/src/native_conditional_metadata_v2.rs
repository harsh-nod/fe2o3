//! Required pair of unchanged V1 metadata and an inert source-bound policy roster.
use crate::{
    MAX_NATIVE_CONDITIONAL_STORAGE_V1, bounded_pair as pair, native_conditional_metadata_v1::*,
    native_conditional_policy_roster_v1::*,
};
use sha2::Sha256;
use std::{convert::Infallible, mem::size_of, ops::Range};

/// Closed metadata V2 discriminator.
pub const NATIVE_CONDITIONAL_METADATA_MAGIC_V2: [u8; 8] = *b"F2NCM2\0\0";
/// Fixed pair header length.
pub const NATIVE_CONDITIONAL_METADATA_HEADER_BYTES_V2: usize = pair::HEADER;
/// Both independently bounded fields plus pair framing.
pub const MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V2: usize = pair::OVERHEAD
    + MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V1
    + MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_BYTES_V1;
const POLICY: pair::Policy = pair::Policy {
    magic: NATIVE_CONDITIONAL_METADATA_MAGIC_V2,
    version: 2,
    domain: b"FE2O3/NATIVE-CONDITIONAL-METADATA/V2\0",
    first_max: MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V1,
    second_max: MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_BYTES_V1,
    total_max: MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V2,
    storage_max: MAX_NATIVE_CONDITIONAL_STORAGE_V1,
};

/// Pair framing or nested unchanged codec failure.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeConditionalMetadataErrorV2<E = Infallible> {
    /// Caller work refusal.
    Charge(E),
    /// Invalid V1 metadata extent.
    MetadataLength,
    /// Invalid roster extent.
    PolicyRosterLength,
    /// Exact total extent differs.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Unsupported header or schema.
    Header,
    /// Nonzero reserved bytes.
    Reserved,
    /// Terminal digest differs.
    Identity,
    /// Unsupported shared storage ceiling.
    StorageLimit,
    /// Nested six-field V1 metadata failed.
    Metadata(NativeConditionalMetadataErrorV1<E>),
    /// Nested inert policy roster failed.
    PolicyRoster(NativeConditionalPolicyRosterErrorV1<E>),
}
type Error<E> = NativeConditionalMetadataErrorV2<E>;
impl<E> From<pair::Error<E>> for Error<E> {
    fn from(error: pair::Error<E>) -> Self {
        match error {
            pair::Error::Charge(e) => Self::Charge(e),
            pair::Error::FirstLength => Self::MetadataLength,
            pair::Error::SecondLength => Self::PolicyRosterLength,
            pair::Error::Length => Self::Length,
            pair::Error::Arithmetic => Self::Arithmetic,
            pair::Error::Header => Self::Header,
            pair::Error::Reserved => Self::Reserved,
            pair::Error::Identity => Self::Identity,
            pair::Error::StorageLimit => Self::StorageLimit,
        }
    }
}

/// Disjoint nested V1 metadata and policy roster ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalMetadataLayoutV2(pair::Layout);
impl NativeConditionalMetadataLayoutV2 {
    /// Check independent member and aggregate extents without visiting payloads.
    pub fn new<E>(v1_len: usize, roster_len: usize) -> Result<Self, Error<E>> {
        Ok(Self(pair::Layout::new(&POLICY, v1_len, roster_len)?))
    }
    /// Complete V2 byte length.
    pub const fn encoded_len(self) -> usize {
        self.0.encoded_len()
    }
    /// Complete unchanged V1 metadata frame.
    pub fn metadata_range(self) -> Range<usize> {
        self.0.first_range()
    }
    /// Complete inert policy roster frame.
    pub fn policy_roster_range(self) -> Range<usize> {
        self.0.second_range()
    }
}

/// Domain-separated V2 metadata identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalMetadataIdentityV2(pair::Identity);
impl NativeConditionalMetadataIdentityV2 {
    /// Terminal digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.0.sha256
    }
    /// Complete encoding length.
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
}

/// Allocation-free, recursively validated inert metadata.
pub struct NativeConditionalMetadataRefV2<'a> {
    bytes: &'a [u8],
    pub(crate) layout: NativeConditionalMetadataLayoutV2,
    metadata: NativeConditionalMetadataRefV1<'a>,
    policy_roster: NativeConditionalPolicyRosterRefV1<'a>,
    identity: NativeConditionalMetadataIdentityV2,
}
impl<'a> NativeConditionalMetadataRefV2<'a> {
    /// Complete original encoding.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Unchanged six-field metadata.
    pub const fn metadata(&self) -> &NativeConditionalMetadataRefV1<'a> {
        &self.metadata
    }
    /// Inert claimed policies; decoding does not accept these policies.
    pub const fn policy_roster(&self) -> &NativeConditionalPolicyRosterRefV1<'a> {
        &self.policy_roster
    }
    /// Exact V2 identity.
    pub const fn identity(&self) -> NativeConditionalMetadataIdentityV2 {
        self.identity
    }
    /// Metadata and policy bytes grant no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fixed recursive working set; the original backing is accounted separately.
pub const NATIVE_CONDITIONAL_METADATA_WORKING_STORAGE_V2: usize =
    size_of::<NativeConditionalMetadataRefV2<'static>>()
        + size_of::<NativeConditionalMetadataLayoutV2>()
        + size_of::<Sha256>()
        + NATIVE_CONDITIONAL_METADATA_WORKING_STORAGE_V1
        + NATIVE_CONDITIONAL_POLICY_ROSTER_WORKING_STORAGE_V1
        + pair::OVERHEAD
        + 128;

/// Seal this pair only, over already written members. Work refusal and callback
/// destruction precede mutation. Nested members must pass their own readers.
pub fn seal_native_conditional_metadata_v2<E>(
    layout: NativeConditionalMetadataLayoutV2,
    bytes: &mut [u8],
    storage_limit: usize,
    charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalMetadataIdentityV2, Error<E>> {
    Ok(NativeConditionalMetadataIdentityV2(pair::seal(
        &POLICY,
        layout.0,
        bytes,
        storage_limit,
        charge,
    )?))
}

/// Strict recursive V2 reader; V1 alone is never a supported alternative.
pub fn read_native_conditional_metadata_v2<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalMetadataRefV2<'a>, Error<E>> {
    let (layout, identity) = pair::read(&POLICY, bytes, storage_limit, &mut charge)?;
    let metadata = read_native_conditional_metadata_v1(
        &bytes[layout.first_range()],
        storage_limit,
        &mut charge,
    )
    .map_err(Error::Metadata)?;
    let policy_roster = read_native_conditional_policy_roster_v1(
        &bytes[layout.second_range()],
        storage_limit,
        charge,
    )
    .map_err(Error::PolicyRoster)?;
    Ok(NativeConditionalMetadataRefV2 {
        bytes,
        layout: NativeConditionalMetadataLayoutV2(layout),
        metadata,
        policy_roster,
        identity: NativeConditionalMetadataIdentityV2(identity),
    })
}
