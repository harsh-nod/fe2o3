//! Conditional output framing only; history, catalog and V5 admission remain mandatory.
use crate::bounded_pair::{self as pair, HEADER, OVERHEAD};
use sha2::Sha256;
use std::{mem::size_of, ops::Range};

/// Distinct output discriminator, never the ordinary F2RFO1 schema.
pub const NATIVE_CONDITIONAL_OUTPUT_MAGIC_V1: [u8; 8] = *b"F2NCO1\0\0";
/// Fixed output pair header size; the auxiliary pair has the same size.
pub const NATIVE_CONDITIONAL_OUTPUT_HEADER_BYTES_V1: usize = HEADER;
/// Mirrored independent history limit; consumers pin it against the history codec.
pub const MAX_NATIVE_CONDITIONAL_HISTORY_BYTES_V1: usize = 16 * 1024 * 1024;
/// Mirrored independent catalog limit; consumers pin it against the catalog codec.
pub const MAX_NATIVE_CONDITIONAL_CATALOG_BYTES_V1: usize = 4 * 1024 * 1024;
/// Unchanged complete nominal descriptor ceiling.
pub const MAX_NATIVE_CONDITIONAL_DESCRIPTOR_BYTES_V1: usize =
    fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;
/// All live caller storage, not just the selected wire range.
pub const MAX_NATIVE_CONDITIONAL_STORAGE_V1: usize = 256 * 1024 * 1024;
const AUX_MAX: usize =
    OVERHEAD + MAX_NATIVE_CONDITIONAL_CATALOG_BYTES_V1 + MAX_NATIVE_CONDITIONAL_DESCRIPTOR_BYTES_V1;
/// Complete output bound including both pair headers and digests.
pub const MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1: usize =
    OVERHEAD + MAX_NATIVE_CONDITIONAL_HISTORY_BYTES_V1 + AUX_MAX;
const _: () = assert!(MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1 <= 64 * 1024 * 1024);
const AUX: pair::Policy = pair::Policy {
    magic: *b"F2NCA1\0\0",
    version: 1,
    domain: b"FE2O3/NATIVE-CONDITIONAL-AUXILIARY/V1\0",
    first_max: MAX_NATIVE_CONDITIONAL_CATALOG_BYTES_V1,
    second_max: MAX_NATIVE_CONDITIONAL_DESCRIPTOR_BYTES_V1,
    total_max: AUX_MAX,
    storage_max: MAX_NATIVE_CONDITIONAL_STORAGE_V1,
};
const POLICY: pair::Policy = pair::Policy {
    magic: NATIVE_CONDITIONAL_OUTPUT_MAGIC_V1,
    version: 1,
    domain: b"FE2O3/NATIVE-CONDITIONAL-OUTPUT/V1\0",
    first_max: MAX_NATIVE_CONDITIONAL_HISTORY_BYTES_V1,
    second_max: AUX_MAX,
    total_max: MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1,
    storage_max: MAX_NATIVE_CONDITIONAL_STORAGE_V1,
};

/// Closed framing failure or unchanged caller work error. No legacy fallback.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeConditionalOutputErrorV1<E> {
    /// Original work refusal.
    Charge(E),
    /// Empty or excessive history.
    HistoryLength,
    /// Empty or excessive catalog.
    CatalogLength,
    /// Empty or excessive descriptor.
    DescriptorLength,
    /// Invalid complete or auxiliary length.
    Length,
    /// Checked arithmetic overflow.
    Arithmetic,
    /// Wrong fixed header/schema.
    Header,
    /// Nonzero reserved bytes.
    Reserved,
    /// Domain-separated digest mismatch.
    Identity,
    /// Unsupported configured storage ceiling.
    StorageLimit,
}
type Error<E> = NativeConditionalOutputErrorV1<E>;
fn error<E>(e: pair::Error<E>, auxiliary: bool) -> Error<E> {
    match e {
        pair::Error::Charge(e) => Error::Charge(e),
        pair::Error::FirstLength if auxiliary => Error::CatalogLength,
        pair::Error::FirstLength => Error::HistoryLength,
        pair::Error::SecondLength if auxiliary => Error::DescriptorLength,
        pair::Error::SecondLength | pair::Error::Length => Error::Length,
        pair::Error::Arithmetic => Error::Arithmetic,
        pair::Error::Header => Error::Header,
        pair::Error::Reserved => Error::Reserved,
        pair::Error::Identity => Error::Identity,
        pair::Error::StorageLimit => Error::StorageLimit,
    }
}

/// Disjoint payload ranges in one caller-prepaid output allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalOutputLayoutV1 {
    outer: pair::Layout,
    auxiliary: pair::Layout,
}
impl NativeConditionalOutputLayoutV1 {
    /// Checks every independent and aggregate bound without visiting payloads.
    pub fn new<E>(history: usize, catalog: usize, descriptor: usize) -> Result<Self, Error<E>> {
        let auxiliary = pair::Layout::new(&AUX, catalog, descriptor).map_err(|e| error(e, true))?;
        let outer = pair::Layout::new(&POLICY, history, auxiliary.encoded_len())
            .map_err(|e| error(e, false))?;
        Ok(Self { outer, auxiliary })
    }
    /// Complete canonical byte extent.
    pub const fn encoded_len(self) -> usize {
        self.outer.encoded_len()
    }
    /// Existing history bytes, containing actual F once.
    pub fn history_range(self) -> Range<usize> {
        self.outer.first_range()
    }
    /// Existing catalog bytes.
    pub fn catalog_range(self) -> Range<usize> {
        self.auxiliary_range(self.auxiliary.first_range())
    }
    /// Complete mandatory V5 descriptor bytes.
    pub fn descriptor_range(self) -> Range<usize> {
        self.auxiliary_range(self.auxiliary.second_range())
    }
    fn auxiliary_range(self, range: Range<usize>) -> Range<usize> {
        let start = self.outer.second_range().start;
        start + range.start..start + range.end
    }
}

/// Domain-separated output content identity, not a semantic receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalOutputIdentityV1(pair::Identity);
impl NativeConditionalOutputIdentityV1 {
    /// Terminal digest binding both ordered pair levels.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.0.sha256
    }
    /// Complete output length.
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
}

/// Strict framing borrowed from unchanged backing; leaf admission is separate.
///
/// ```compile_fail
/// use fe2o3_compiler_lineage::NativeConditionalOutputRefV1;
/// fn escape<'a>(v: NativeConditionalOutputRefV1<'a>) -> NativeConditionalOutputRefV1<'static> { v }
/// ```
pub struct NativeConditionalOutputRefV1<'a> {
    bytes: &'a [u8],
    pub(crate) layout: NativeConditionalOutputLayoutV1,
    identity: NativeConditionalOutputIdentityV1,
}
impl<'a> NativeConditionalOutputRefV1<'a> {
    /// Complete framed bytes.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Unchanged history V1 encoding.
    pub fn history(&self) -> &'a [u8] {
        &self.bytes[self.layout.history_range()]
    }
    /// Unchanged catalog V1 encoding.
    pub fn catalog(&self) -> &'a [u8] {
        &self.bytes[self.layout.catalog_range()]
    }
    /// Complete descriptor V5 encoding, not an admitted table.
    pub fn descriptor_bytes(&self) -> &'a [u8] {
        &self.bytes[self.layout.descriptor_range()]
    }
    /// Exact content identity.
    pub const fn identity(&self) -> NativeConditionalOutputIdentityV1 {
        self.identity
    }
    /// Framing grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Conservative fixed simultaneous logical scratch; backing is paid separately.
pub const NATIVE_CONDITIONAL_OUTPUT_WORKING_STORAGE_V1: usize =
    size_of::<NativeConditionalOutputRefV1<'static>>()
        + size_of::<NativeConditionalOutputLayoutV1>()
        + size_of::<Sha256>()
        + 2 * HEADER
        + 256;

/// Seals both levels without copying payloads. All work and callback destruction
/// precede any mutation, including the inner auxiliary header and digest.
pub fn seal_native_conditional_output_v1<E>(
    layout: NativeConditionalOutputLayoutV1,
    bytes: &mut [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalOutputIdentityV1, Error<E>> {
    let outer = pair::seal_work(&POLICY, layout.outer, bytes.len(), storage_limit)
        .map_err(|e| error(e, false))?;
    let inner = pair::seal_work(
        &AUX,
        layout.auxiliary,
        layout.outer.second_range().len(),
        storage_limit,
    )
    .map_err(|e| error(e, true))?;
    charge(outer.checked_add(inner).ok_or(Error::Arithmetic)?).map_err(Error::Charge)?;
    drop(charge);
    pair::seal(
        &AUX,
        layout.auxiliary,
        &mut bytes[layout.outer.second_range()],
        storage_limit,
        |_| Ok::<_, E>(()),
    )
    .map_err(|e| error(e, true))?;
    pair::seal(&POLICY, layout.outer, bytes, storage_limit, |_| {
        Ok::<_, E>(())
    })
    .map(NativeConditionalOutputIdentityV1)
    .map_err(|e| error(e, false))
}

/// Allocation-free strict recursive framing read. Each visit is prepaid.
pub fn read_native_conditional_output_v1<'a, E>(
    bytes: &'a [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<NativeConditionalOutputRefV1<'a>, Error<E>> {
    let (outer, identity) =
        pair::read(&POLICY, bytes, storage_limit, &mut charge).map_err(|e| error(e, false))?;
    let (auxiliary, _) = pair::read(&AUX, &bytes[outer.second_range()], storage_limit, charge)
        .map_err(|e| error(e, true))?;
    Ok(NativeConditionalOutputRefV1 {
        bytes,
        layout: NativeConditionalOutputLayoutV1 { outer, auxiliary },
        identity: NativeConditionalOutputIdentityV1(identity),
    })
}
