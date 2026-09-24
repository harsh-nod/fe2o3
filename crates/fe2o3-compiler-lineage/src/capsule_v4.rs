//! Native capsule framing, deliberately separate from semantic admission.
use std::{convert::Infallible, mem::size_of, ops::Range, sync::Arc};

use crate::{
    InertProductionSemanticCapsuleV3, LineageDecodeErrorV3,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3, MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1,
    MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1,
    NATIVE_REFINED_FORWARDING_CARRIER_WORKING_STORAGE_V1, NativeRefinedForwardingCarrierErrorV1,
    NativeRefinedForwardingCarrierIdentityV1, NativeRefinedForwardingCarrierRefV1, bounded_pair,
    read_native_refined_forwarding_carrier_v1,
    receipt::{ImmutableBytesV3, SharedBackingV3},
};

/// Distinct discriminator; neither a bare V3 capsule nor a carrier is a V4 capsule.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V4: [u8; 8] = *b"F2O3ISV4";
/// Native capsule framing version.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V4: u16 = 4;
/// Complete V4 ceiling, including the base, mandatory carrier and all framing.
pub const MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4: usize =
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3;
const POLICY: bounded_pair::Policy = bounded_pair::Policy {
    magic: INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V4,
    version: INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V4,
    domain: b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V4\0",
    first_max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
    second_max: MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1,
    total_max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4,
    storage_max: MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1,
};

/// Framing, nested content, or caller work refusal; no legacy retry is performed.
#[derive(Debug, Eq, PartialEq)]
pub enum InertProductionSemanticCapsuleErrorV4<E = Infallible> {
    /// Caller refused work before that byte visit or mutation.
    Charge(E),
    /// Mandatory V3 base is empty or exceeds its independent bound.
    BaseLength,
    /// Mandatory carrier is empty or exceeds its independent bound.
    CarrierLength,
    /// Range, aggregate ceiling or exact complete length is invalid.
    Length,
    /// Checked extent arithmetic overflowed.
    Arithmetic,
    /// Magic, version, policy or header length differs.
    Header,
    /// Reserved bytes are nonzero.
    Reserved,
    /// Terminal capsule identity differs.
    Identity,
    /// Requested shared replay ceiling exceeds the supported policy.
    StorageLimit,
    /// Paired transport framing or work debit failed.
    Carrier(NativeRefinedForwardingCarrierErrorV1<E>),
    /// V3 base content or the tighter native MIR extent is invalid.
    Base(LineageDecodeErrorV3),
}
type Error<E = Infallible> = InertProductionSemanticCapsuleErrorV4<E>;
impl<E> From<bounded_pair::Error<E>> for Error<E> {
    fn from(error: bounded_pair::Error<E>) -> Self {
        use bounded_pair::Error as Pair;
        match error {
            Pair::Charge(e) => Self::Charge(e),
            Pair::FirstLength => Self::BaseLength,
            Pair::SecondLength => Self::CarrierLength,
            Pair::Length => Self::Length,
            Pair::Arithmetic => Self::Arithmetic,
            Pair::Header => Self::Header,
            Pair::Reserved => Self::Reserved,
            Pair::Identity => Self::Identity,
            Pair::StorageLimit => Self::StorageLimit,
        }
    }
}

/// Disjoint byte ranges for encoding the base and carrier into one final buffer.
/// This describes extents only, not semantic validity of either field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertProductionSemanticCapsuleLayoutV4(bounded_pair::Layout);
impl InertProductionSemanticCapsuleLayoutV4 {
    /// Checks both constituent bounds and the complete 160 MiB ceiling.
    pub fn new<E>(base_len: usize, carrier_len: usize) -> Result<Self, Error<E>> {
        Ok(Self(bounded_pair::Layout::new(
            &POLICY,
            base_len,
            carrier_len,
        )?))
    }
    /// Complete buffer length, including the terminal identity.
    pub const fn encoded_len(self) -> usize {
        self.0.encoded_len()
    }
    /// Exact unchanged V3 base encoding.
    pub fn base_range(self) -> Range<usize> {
        self.0.first_range()
    }
    /// Exact unchanged F2NRF1 carrier encoding.
    pub fn carrier_range(self) -> Range<usize> {
        self.0.second_range()
    }
}

/// Domain-separated content identity of the entire V4 capsule, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertProductionSemanticCapsuleIdentityV4(bounded_pair::Identity);
impl InertProductionSemanticCapsuleIdentityV4 {
    /// Domain-separated terminal SHA256.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.0.sha256
    }
    /// Complete canonical byte length, including the terminal digest.
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
}

/// Borrowed V4 and carrier framing. The V3 base and source/output semantics
/// still need admission; public hashes establish no compiler authority.
pub struct InertProductionSemanticCapsuleRefV4<'a> {
    bytes: &'a [u8],
    layout: InertProductionSemanticCapsuleLayoutV4,
    identity: InertProductionSemanticCapsuleIdentityV4,
    carrier: NativeRefinedForwardingCarrierRefV1<'a>,
}
impl<'a> InertProductionSemanticCapsuleRefV4<'a> {
    /// Complete borrowed V4 image.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Unparsed, unchanged V3 base.
    pub fn base_bytes(&self) -> &'a [u8] {
        &self.bytes[self.layout.base_range()]
    }
    /// Borrowed paired carrier, with its own distinct identity.
    pub const fn carrier(&self) -> &NativeRefinedForwardingCarrierRefV1<'a> {
        &self.carrier
    }
    /// Exact V4 identity, not a V3 identity or the paired carrier identity.
    pub const fn identity(&self) -> InertProductionSemanticCapsuleIdentityV4 {
        self.identity
    }
    /// Framing never authenticates a compiler or permits launch.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fixed logical seal/read scratch, excluding backing and V3 base decoding.
/// Reserve this and all live backing on the same caller ledger before use.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V4: usize =
    size_of::<InertProductionSemanticCapsuleRefV4<'static>>()
        + size_of::<InertProductionSemanticCapsuleLayoutV4>()
        + NATIVE_REFINED_FORWARDING_CARRIER_WORKING_STORAGE_V1
        + bounded_pair::OVERHEAD
        + 128;

/// Seals framing around already written base and carrier ranges without copying
/// them or interpreting their contents. All debits precede any mutation;
/// refusal or panic leaves the destination unchanged. Payload writes are the
/// caller's responsibility and must be prepaid independently.
pub fn seal_inert_production_semantic_capsule_v4<E>(
    layout: InertProductionSemanticCapsuleLayoutV4,
    bytes: &mut [u8],
    storage_limit: usize,
    charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertProductionSemanticCapsuleIdentityV4, Error<E>> {
    Ok(InertProductionSemanticCapsuleIdentityV4(
        bounded_pair::seal(&POLICY, layout.0, bytes, storage_limit, charge_work)?,
    ))
}

/// Allocation-free strict framing read, including the mandatory carrier's
/// framing and hash. Every visited header/hash is prepaid; nested source/output
/// semantics and V3 base content are deliberately not admitted here.
pub fn read_inert_production_semantic_capsule_v4<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertProductionSemanticCapsuleRefV4<'a>, Error<E>> {
    let (layout, identity) = bounded_pair::read(&POLICY, bytes, storage_limit, &mut charge_work)?;
    let carrier = read_native_refined_forwarding_carrier_v1(
        &bytes[layout.second_range()],
        storage_limit,
        charge_work,
    )
    .map_err(Error::Carrier)?;
    Ok(InertProductionSemanticCapsuleRefV4 {
        bytes,
        layout: InertProductionSemanticCapsuleLayoutV4(layout),
        identity: InertProductionSemanticCapsuleIdentityV4(identity),
        carrier,
    })
}

/// Immutable V4 content owner. The V3 base and carrier retain checked ranges in
/// the same allocation. This is not the legacy V3 typed producer/admission path.
/// It does not prove source/F equivalence, join their MIR, or grant authority.
///
/// ```compile_fail
/// use fe2o3_compiler_lineage::InertProductionSemanticCapsuleV4;
/// fn duplicate(v: InertProductionSemanticCapsuleV4) { let _ = v.clone(); }
/// ```
pub struct InertProductionSemanticCapsuleV4 {
    base: InertProductionSemanticCapsuleV3,
    bytes: ImmutableBytesV3,
    carrier_range: Range<usize>,
    carrier_identity: NativeRefinedForwardingCarrierIdentityV1,
    identity: InertProductionSemanticCapsuleIdentityV4,
}
impl InertProductionSemanticCapsuleV4 {
    /// Transfers an existing V4 buffer without a second complete payload copy.
    /// Like V3 content decoding, this is unmetered: a production caller must
    /// preflight/pay its decode working set and actual backing capacity first.
    pub fn decode_owned(bytes: Vec<u8>) -> Result<Self, Error> {
        let len = bytes.len();
        Self::decode_shared_vec(Arc::new(bytes), 0..len)
    }
    /// Decodes a checked subrange in one caller-owned allocation. All base
    /// receipts and the carrier share that backing. Caller pays the complete
    /// backing capacity (including bytes outside this range) once, plus decoded
    /// invocation storage and bounded decoder scratch; this API has no ledger.
    pub fn decode_shared_vec(backing: Arc<Vec<u8>>, range: Range<usize>) -> Result<Self, Error> {
        Self::decode_shared(SharedBackingV3::Vector(backing), range)
    }
    fn decode_shared(backing: SharedBackingV3, range: Range<usize>) -> Result<Self, Error> {
        let bytes = backing.as_slice().get(range.clone()).ok_or(Error::Length)?;
        let frame = read_inert_production_semantic_capsule_v4(bytes, POLICY.storage_max, |_| {
            Ok::<_, Infallible>(())
        })?;
        let base_range = frame.layout.base_range();
        let carrier_range = frame.layout.carrier_range();
        let identity = frame.identity();
        let carrier_identity = frame.carrier().identity();
        // Equal MIR must fit inside the complete source packet. Check this
        // necessary (not sufficient) bound before deriving the V3 MIR receipt
        // hash. The aggregate hashes have already visited these bytes.
        let mir_limit = frame.carrier().source_packet().len();
        let base = InertProductionSemanticCapsuleV3::decode_shared_with_mir_limit(
            backing.clone(),
            range.start + base_range.start..range.start + base_range.end,
            mir_limit,
        )
        .map_err(Error::Base)?;
        Ok(Self {
            base,
            bytes: ImmutableBytesV3::from_shared(backing, range).ok_or(Error::Length)?,
            carrier_range,
            carrier_identity,
            identity,
        })
    }
    /// Content-only base. Its original-N receipts are not final-F authority.
    pub const fn base(&self) -> &InertProductionSemanticCapsuleV3 {
        &self.base
    }
    /// Complete canonical V4 encoding in the shared input buffer.
    pub fn canonical_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    /// Exact mandatory carrier bytes, ready for the existing paired verifier.
    pub fn carrier_bytes(&self) -> &[u8] {
        &self.bytes.as_slice()[self.carrier_range.clone()]
    }
    /// Identity binding both source and final-F output, separately from V4.
    pub const fn carrier_identity(&self) -> NativeRefinedForwardingCarrierIdentityV1 {
        self.carrier_identity
    }
    /// Identity of the complete V4 capsule.
    pub const fn identity(&self) -> InertProductionSemanticCapsuleIdentityV4 {
        self.identity
    }
    /// Content consistency never grants compiler, publication or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
