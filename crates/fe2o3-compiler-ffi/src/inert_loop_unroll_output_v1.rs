//! Distinct final-U framing. Content identity never supplies semantic authority.
use crate::inert_refined_forwarding_output_v1 as previous;
use sha2::{Digest, Sha256};
use std::mem::size_of;

pub use previous::{
    InertRefinedForwardingRootRefV1 as InertLoopUnrollRootRefV1,
    InertRefinedForwardingRouteV1 as InertLoopUnrollRouteV1,
};
pub const INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1: [u8; 8] = *b"F2LUO1\0\0";
pub const INERT_LOOP_UNROLL_HISTORY_MAGIC_V1: [u8; 8] = *b"F2LUH1\0\0";
pub const MAX_INERT_LOOP_UNROLL_OUTPUT_BYTES_V1: usize = 64 << 20;
pub const MAX_INERT_LOOP_UNROLL_HISTORY_BYTES_V1: usize = 16 << 20;
pub const MAX_INERT_LOOP_UNROLL_STORAGE_V1: usize = 256 << 20;
pub const MAX_INERT_LOOP_UNROLL_ROOTS_V1: usize = 128;
const OUTPUT_HEADER: usize = 144;
const HISTORY_HEADER: usize = 104;
const OUTPUT_DOMAIN: &[u8] = b"FE2O3/LOOP-UNROLL-OUTPUT/V1\0";
const HISTORY_DOMAIN: &[u8] = b"FE2O3/LOOP-UNROLL-HISTORY/V1\0";
const FIELD_LIMITS: [usize; 14] = [
    16 << 20,
    64 << 20,
    256 << 10,
    16 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 << 20,
    4 + 128 * (219 + 512),
];
const ROW_WIDTHS: [usize; 6] = [20, 44, 28, 20, 28, 36];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum InertLoopUnrollOutputFieldV1 {
    History,
    NativeV2,
    Descriptor,
    SemanticMir,
    OriginalNative,
    Erased,
    OriginalInputV4,
    OriginalFormalMemory,
    FinalNative,
    FinalFormalMemory,
    OriginalMiddleEnd,
    OriginalCorrespondence,
    OriginalVerus,
    Roots,
}
#[derive(Debug, Eq, PartialEq)]
pub enum InertLoopUnrollOutputErrorV1<E> {
    Charge(E),
    StorageLimit,
    Length,
    Arithmetic,
    Header,
    Reserved,
    Route,
    Field(usize),
    RootCount,
    RootOrder,
    RootPermutation,
    RootBoolean,
    RootName,
}
type Error<E> = InertLoopUnrollOutputErrorV1<E>;
impl<E> From<previous::InertRefinedForwardingOutputErrorV1<E>> for Error<E> {
    fn from(value: previous::InertRefinedForwardingOutputErrorV1<E>) -> Self {
        use previous::InertRefinedForwardingOutputErrorV1 as Old;
        match value {
            Old::Charge(e) => Self::Charge(e),
            Old::StorageLimit => Self::StorageLimit,
            Old::Length => Self::Length,
            Old::Arithmetic => Self::Arithmetic,
            Old::Header => Self::Header,
            Old::Reserved => Self::Reserved,
            Old::Route => Self::Route,
            Old::Field(i) => Self::Field(i),
            Old::RootCount => Self::RootCount,
            Old::RootOrder => Self::RootOrder,
            Old::RootPermutation => Self::RootPermutation,
            Old::RootBoolean => Self::RootBoolean,
            Old::RootName => Self::RootName,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertLoopUnrollContentIdentityV1 {
    pub sha256: [u8; 32],
    pub byte_len: u64,
}

/// Immutable U framing only. An F frame is not a U frame or its final-output proof.
/// ```compile_fail
/// use fe2o3_compiler_ffi::InertLoopUnrollOutputRefV1 as Frame;
/// fn escape<'a>(frame: Frame<'a>) -> Frame<'static> { frame }
/// ```
pub struct InertLoopUnrollOutputRefV1<'w> {
    wire: &'w [u8],
    fields: [&'w [u8]; 14],
    route: InertLoopUnrollRouteV1,
    roots: u32,
}
impl<'w> InertLoopUnrollOutputRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub fn field(&self, role: InertLoopUnrollOutputFieldV1) -> &'w [u8] {
        self.fields[role as usize]
    }
    pub const fn route(&self) -> InertLoopUnrollRouteV1 {
        self.route
    }
    pub const fn root_count(&self) -> u32 {
        self.roots
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Reserve READ_STORAGE plus the returned RootRef header for its lifetime.
    pub fn root<E>(
        &self,
        ordinal: u32,
        storage_limit: usize,
        mut charge_work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<InertLoopUnrollRootRefV1<'w>, Error<E>> {
        caps(
            self.wire.len(),
            MAX_INERT_LOOP_UNROLL_OUTPUT_BYTES_V1,
            storage_limit,
        )?;
        if ordinal >= self.roots {
            return Err(Error::RootCount);
        }
        pay(
            &mut charge_work,
            self.fields[13]
                .len()
                .checked_mul(3)
                .ok_or(Error::Arithmetic)?,
        )?;
        let mut cursor = previous::Cursor {
            bytes: self.fields[13],
            pos: 4,
        };
        for _ in 0..ordinal {
            cursor.root::<E>().map_err(Error::from)?;
        }
        cursor.root().map_err(Error::from)
    }
}

struct UHistoryFramingScratch<'w> {
    history: [&'w [u8]; 9],
    prefix: [&'w [u8]; 27],
    counts: [u32; 6],
    settings: [u64; 12],
    selection: Option<(usize, u8)>,
    cursor: usize,
    end: usize,
    total: usize,
}
pub const INERT_LOOP_UNROLL_READ_STORAGE_V1: usize =
    size_of::<InertLoopUnrollOutputRefV1<'static>>()
        + size_of::<[&'static [u8]; 14]>()
        + size_of::<UHistoryFramingScratch<'static>>()
        + size_of::<[[bool; MAX_INERT_LOOP_UNROLL_ROOTS_V1]; 3]>()
        + size_of::<previous::Cursor<'static>>()
        + size_of::<InertLoopUnrollRootRefV1<'static>>()
        + size_of::<Option<u32>>();
pub const INERT_LOOP_UNROLL_ENCODE_STORAGE_V1: usize =
    INERT_LOOP_UNROLL_READ_STORAGE_V1 + size_of::<[u64; 14]>() + OUTPUT_HEADER;
pub const INERT_LOOP_UNROLL_HASH_STORAGE_V1: usize = INERT_LOOP_UNROLL_READ_STORAGE_V1
    + size_of::<Sha256>()
    + size_of::<InertLoopUnrollContentIdentityV1>()
    + 8
    + 128;

fn caps<E>(length: usize, maximum: usize, storage: usize) -> Result<(), Error<E>> {
    if storage > MAX_INERT_LOOP_UNROLL_STORAGE_V1 {
        return Err(Error::StorageLimit);
    }
    if length > maximum {
        return Err(Error::Length);
    }
    Ok(())
}
fn add<E>(a: usize, b: usize) -> Result<usize, Error<E>> {
    a.checked_add(b).ok_or(Error::Arithmetic)
}
fn pay<E>(charge: &mut impl FnMut(usize) -> Result<(), E>, n: usize) -> Result<(), Error<E>> {
    charge(n).map_err(Error::Charge)
}
fn route_byte(route: InertLoopUnrollRouteV1) -> u8 {
    match route {
        InertLoopUnrollRouteV1::Direct => 0,
        InertLoopUnrollRouteV1::Erased => 1,
    }
}
fn number<E>(bytes: &[u8]) -> Result<u64, Error<E>> {
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| Error::Field(0))?,
    ))
}

// Bounded nested framing only. Typed old/new row syntax and semantic replay
// belong to kernel-opt; the FFI view cannot substitute for either one.
fn history<E>(wire: &[u8]) -> Result<(), Error<E>> {
    if wire.len() < HISTORY_HEADER || wire.len() > MAX_INERT_LOOP_UNROLL_HISTORY_BYTES_V1 {
        return Err(Error::Field(0));
    }
    let mut s = UHistoryFramingScratch {
        history: previous::fields::<E, 9>(wire, &INERT_LOOP_UNROLL_HISTORY_MAGIC_V1, 0)
            .map_err(Error::from)?,
        prefix: [&[]; 27],
        counts: [0; 6],
        settings: [0; 12],
        selection: None,
        cursor: 0,
        end: 0,
        total: 0,
    };
    s.prefix = previous::fields::<E, 27>(
        s.history[0],
        &previous::INERT_REFINED_FORWARDING_HISTORY_MAGIC_V1,
        0,
    )
    .map_err(Error::from)?;
    if s.history.iter().any(|field| field.is_empty())
        || s.prefix.iter().any(|field| field.is_empty())
        || s.history[8].len() != 104
    {
        return Err(Error::Field(0));
    }
    s.total = s.prefix[..12]
        .iter()
        .try_fold(s.history[1].len(), |n, b| add::<E>(n, b.len()))?;
    if s.total > 12 << 20 {
        return Err(Error::Field(0));
    }
    s.total = [14, 18, 19, 20, 21, 22, 23, 24, 25]
        .into_iter()
        .try_fold(0, |n, i| add::<E>(n, s.prefix[i].len()))?;
    s.total = s.history[2..8]
        .iter()
        .try_fold(s.total, |n, b| add::<E>(n, b.len()))?;
    if s.total > 4 << 20 {
        return Err(Error::Field(0));
    }
    s.total = 0;
    for (family, width) in ROW_WIDTHS.iter().enumerate() {
        let bytes = s.history[family + 2];
        if bytes.len() < 8 || bytes[4..8] != [family as u8, 1, 0, 0] {
            return Err(Error::Field(0));
        }
        s.counts[family] = u32::from_le_bytes(bytes[..4].try_into().map_err(|_| Error::Field(0))?);
        s.end = usize::try_from(s.counts[family])
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(*width)
            .and_then(|n| n.checked_add(8))
            .ok_or(Error::Arithmetic)?;
        if s.end != bytes.len() {
            return Err(Error::Field(0));
        }
        s.total = add(s.total, s.counts[family] as usize)?;
    }
    if s.total > 262_144 {
        return Err(Error::Field(0));
    }
    let settings = s.history[8];
    for i in 0..7 {
        s.cursor = i * 8;
        s.settings[i] = number(&settings[s.cursor..s.cursor + 8])?;
    }
    s.settings[7] = u64::from(settings[56]);
    for i in 0..3 {
        s.cursor = 64 + i * 8;
        s.settings[8 + i] = number(&settings[s.cursor..s.cursor + 8])?;
    }
    s.settings[11] = number(&settings[96..104])?;
    for value in &s.settings {
        usize::try_from(*value).map_err(|_| Error::Arithmetic)?;
    }
    if settings[56] > 8 || settings[57..64] != [0; 7] || settings[90..96] != [0; 6] {
        return Err(Error::Field(0));
    }
    s.selection = match settings[88] {
        0 if settings[89..104] == [0; 15] => None,
        1 => Some((
            usize::try_from(s.settings[11]).map_err(|_| Error::Arithmetic)?,
            settings[89],
        )),
        _ => return Err(Error::Field(0)),
    };
    if s.selection
        .is_some_and(|(_, iterations)| iterations > settings[56])
    {
        return Err(Error::Field(0));
    }
    Ok(())
}

fn validate<E>(fields: &[&[u8]; 14], route: InertLoopUnrollRouteV1) -> Result<u32, Error<E>> {
    for (i, (field, limit)) in fields.iter().zip(FIELD_LIMITS.iter()).enumerate() {
        if field.len() > *limit || (i != 5 && field.is_empty()) {
            return Err(Error::Field(i));
        }
    }
    if matches!(route, InertLoopUnrollRouteV1::Direct) != fields[5].is_empty() {
        return Err(Error::Route);
    }
    history::<E>(fields[0])?;
    previous::roots(fields[13]).map_err(Error::from)
}

/// Caller prepays READ_STORAGE and all borrowed backing on its own single
/// ledger. The scalar cap and generic work callback do not prove that custody.
pub fn read_inert_loop_unroll_output_v1<'w, E>(
    wire: &'w [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertLoopUnrollOutputRefV1<'w>, Error<E>> {
    caps(
        wire.len(),
        MAX_INERT_LOOP_UNROLL_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    if wire.len() < OUTPUT_HEADER {
        return Err(Error::Length);
    }
    pay(&mut charge_work, OUTPUT_HEADER)?;
    let route = match wire[28] {
        0 => InertLoopUnrollRouteV1::Direct,
        1 => InertLoopUnrollRouteV1::Erased,
        _ => return Err(Error::Route),
    };
    let fields = previous::fields(wire, &INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, route_byte(route))
        .map_err(Error::from)?;
    pay(
        &mut charge_work,
        fields[13]
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(1024))
            .ok_or(Error::Arithmetic)?,
    )?;
    let roots = validate(&fields, route)?;
    Ok(InertLoopUnrollOutputRefV1 {
        wire,
        fields,
        route,
        roots,
    })
}
/// Slice lengths only; this does not validate payloads or establish a receipt.
pub fn inert_loop_unroll_output_len_v1<E>(fields: &[&[u8]; 14]) -> Result<usize, Error<E>> {
    fields
        .iter()
        .try_fold(OUTPUT_HEADER, |n, f| add(n, f.len()))
}
/// Exact caller buffer; all checks and write work precede its first mutation.
pub fn encode_inert_loop_unroll_output_into_v1<E>(
    fields: [&[u8]; 14],
    route: InertLoopUnrollRouteV1,
    out: &mut [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), Error<E>> {
    caps(
        out.len(),
        MAX_INERT_LOOP_UNROLL_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    pay(&mut charge_work, 14)?;
    let length = inert_loop_unroll_output_len_v1(&fields)?;
    if length != out.len() {
        return Err(Error::Length);
    }
    pay(
        &mut charge_work,
        fields[13]
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(1024))
            .ok_or(Error::Arithmetic)?,
    )?;
    validate(&fields, route)?;
    pay(&mut charge_work, add(length, OUTPUT_HEADER)?)?;
    out[..OUTPUT_HEADER].fill(0);
    out[..8].copy_from_slice(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1);
    out[8..12].copy_from_slice(&[1, 0, 1, 0]);
    out[12..16].copy_from_slice(&(OUTPUT_HEADER as u32).to_le_bytes());
    out[16..24].copy_from_slice(&(length as u64).to_le_bytes());
    out[24..26].copy_from_slice(&14u16.to_le_bytes());
    out[28] = route_byte(route);
    let mut cursor = OUTPUT_HEADER;
    for (i, field) in fields.iter().enumerate() {
        out[32 + i * 8..40 + i * 8].copy_from_slice(&(field.len() as u64).to_le_bytes());
        out[cursor..cursor + field.len()].copy_from_slice(field);
        cursor += field.len();
    }
    Ok(())
}
fn identity<E>(
    wire: &[u8],
    domain: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<InertLoopUnrollContentIdentityV1, Error<E>> {
    let byte_len = u64::try_from(wire.len()).map_err(|_| Error::Arithmetic)?;
    pay(charge, add(add(wire.len(), domain.len())?, 8 + 128)?)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(byte_len.to_le_bytes());
    hash.update(wire);
    Ok(InertLoopUnrollContentIdentityV1 {
        sha256: hash.finalize().into(),
        byte_len,
    })
}
pub fn inert_loop_unroll_output_identity_v1<E>(
    frame: &InertLoopUnrollOutputRefV1<'_>,
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertLoopUnrollContentIdentityV1, Error<E>> {
    caps(
        frame.wire.len(),
        MAX_INERT_LOOP_UNROLL_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    identity(frame.wire, OUTPUT_DOMAIN, &mut charge_work)
}
pub fn inert_loop_unroll_history_identity_v1<E>(
    wire: &[u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertLoopUnrollContentIdentityV1, Error<E>> {
    caps(
        wire.len(),
        MAX_INERT_LOOP_UNROLL_HISTORY_BYTES_V1,
        storage_limit,
    )?;
    if wire.len() < HISTORY_HEADER {
        return Err(Error::Length);
    }
    pay(&mut charge_work, 1024)?;
    history::<E>(wire)?;
    identity(wire, HISTORY_DOMAIN, &mut charge_work)
}

#[cfg(test)]
#[path = "inert_loop_unroll_output_v1_tests.rs"]
mod tests;
