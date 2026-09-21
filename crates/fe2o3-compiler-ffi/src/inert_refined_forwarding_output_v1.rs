//! Allocation-free inert framing. No compiler, semantic or execution authority.
use sha2::{Digest, Sha256};
use std::mem::size_of;

/// Fixed framing discriminator, not a policy number.
pub const INERT_REFINED_FORWARDING_OUTPUT_MAGIC_V1: [u8; 8] = *b"F2RFO1\0\0";
/// Complete B/C/S/O/I/J/K/P/H/L/R/F frame discriminator.
pub const INERT_REFINED_FORWARDING_HISTORY_MAGIC_V1: [u8; 8] = *b"F2RFH1\0\0";
/// Maximum complete output frame, including repeated constituent bytes.
pub const MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum complete history frame.
pub const MAX_INERT_REFINED_FORWARDING_HISTORY_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum configured storage of the later single caller budget, including siblings.
pub const MAX_INERT_REFINED_FORWARDING_STORAGE_V1: usize = 256 * 1024 * 1024;
/// Fixed root roster limit.
pub const MAX_INERT_REFINED_FORWARDING_ROOTS_V1: usize = 128;
const OUTPUT_HEADER: usize = 144;
const HISTORY_HEADER: usize = 248;
const OUTPUT_DOMAIN: &[u8] = b"FE2O3/REFINED-FORWARDING-OUTPUT/V1\0";
const HISTORY_DOMAIN: &[u8] = b"FE2O3/REFINED-FORWARDING-HISTORY/V1\0";

/// Source route claim only; the original independently admitted source is required later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InertRefinedForwardingRouteV1 {
    Direct,
    Erased,
}
impl InertRefinedForwardingRouteV1 {
    fn byte(self) -> u8 {
        match self {
            Self::Direct => 0,
            Self::Erased => 1,
        }
    }
}

/// Fixed field roles. Equal bytes in different roles remain separate fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum InertRefinedForwardingOutputFieldV1 {
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

/// Framing failure. `Charge` preserves the caller's exact typed denial without boxing.
#[derive(Debug, Eq, PartialEq)]
pub enum InertRefinedForwardingOutputErrorV1<E> {
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
type Error<E> = InertRefinedForwardingOutputErrorV1<E>;

/// Only content identity: neither semantic validity nor authenticated producer custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertRefinedForwardingContentIdentityV1 {
    pub sha256: [u8; 32],
    pub byte_len: u64,
}

/// Inert fixed root fields and borrowed names. No typed-rustc ABI or validated launch.
pub struct InertRefinedForwardingRootRefV1<'w> {
    pub semantic_root: u32,
    pub semantic_function_identity: [u8; 32],
    pub descriptor_ordinal: u32,
    pub descriptor_kernel_id: [u8; 32],
    pub original_kernel_ordinal: u32,
    pub original_function: u32,
    pub final_kernel_ordinal: u32,
    pub final_function: u32,
    pub source_kernel_binding: [u8; 32],
    pub source_rank: u8,
    pub exact_workgroup: Option<[u32; 3]>,
    pub source_max_grid: [u32; 3],
    pub grid_identity: u64,
    pub global_extents: [u64; 3],
    pub workgroup_extents: [u64; 3],
    pub subgroup_size: u64,
    pub full_physical_workgroups: bool,
    pub logical_name: &'w str,
    pub export_name: &'w str,
}

/// Checked framing borrowing the original immutable bytes; not a decoded V2/descriptor.
/// Nested graph, proof, ABI, launch and source claims remain untrusted.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::InertRefinedForwardingOutputRefV1 as Frame;
/// fn escape<'a>(f: Frame<'a>) -> Frame<'static> { f }
/// ```
pub struct InertRefinedForwardingOutputRefV1<'w> {
    wire: &'w [u8],
    fields: [&'w [u8]; 14],
    route: InertRefinedForwardingRouteV1,
    roots: u32,
}
impl<'w> InertRefinedForwardingOutputRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub fn field(&self, role: InertRefinedForwardingOutputFieldV1) -> &'w [u8] {
        self.fields[role as usize]
    }
    pub const fn route(&self) -> InertRefinedForwardingRouteV1 {
        self.route
    }
    pub const fn root_count(&self) -> u32 {
        self.roots
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Prepay READ_STORAGE plus a returned root view header. Work covers the
    /// complete bounded root scan before accessing any row, including names.
    pub fn root<E>(
        &self,
        ordinal: u32,
        storage_limit: usize,
        mut charge_work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<InertRefinedForwardingRootRefV1<'w>, Error<E>> {
        caps(
            self.wire.len(),
            MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1,
            storage_limit,
        )?;
        if ordinal >= self.roots {
            return Err(Error::RootCount);
        }
        charge(
            &mut charge_work,
            self.fields[13]
                .len()
                .checked_mul(3)
                .ok_or(Error::Arithmetic)?,
        )?;
        let mut cursor = Cursor {
            bytes: self.fields[13],
            pos: 4,
        };
        for _ in 0..ordinal {
            cursor.root()?;
        }
        cursor.root()
    }
}

/// Prepay this whole fixed extent before reading. It conservatively includes both
/// field tables, returned view, three root permutation tables and scalar cursor
/// state. Keep the view header reserved until dropped. The caller separately
/// prepays borrowed input backing once. These are logical extents, not RSS.
pub const INERT_REFINED_FORWARDING_READ_STORAGE_V1: usize = size_of::<
    InertRefinedForwardingOutputRefV1<'static>,
>() + size_of::<[&'static [u8]; 27]>()
    + size_of::<[&'static [u8]; 14]>()
    + size_of::<[[bool; MAX_INERT_REFINED_FORWARDING_ROOTS_V1]; 3]>()
    + size_of::<Cursor<'static>>()
    + size_of::<Option<u32>>()
    + size_of::<InertRefinedForwardingRootRefV1<'static>>();
/// Encoder working extent, excluding caller-owned exact output and input backing.
pub const INERT_REFINED_FORWARDING_ENCODE_STORAGE_V1: usize =
    INERT_REFINED_FORWARDING_READ_STORAGE_V1 + size_of::<[u64; 14]>() + OUTPUT_HEADER;
/// Content hashing working extent, in addition to the read extent and live backing.
pub const INERT_REFINED_FORWARDING_HASH_STORAGE_V1: usize = INERT_REFINED_FORWARDING_READ_STORAGE_V1
    + size_of::<Sha256>()
    + size_of::<InertRefinedForwardingContentIdentityV1>()
    + 128;

fn caps<E>(len: usize, max: usize, storage: usize) -> Result<(), Error<E>> {
    if storage > MAX_INERT_REFINED_FORWARDING_STORAGE_V1 {
        return Err(Error::StorageLimit);
    }
    if len > max {
        return Err(Error::Length);
    }
    Ok(())
}
fn charge<E>(f: &mut impl FnMut(usize) -> Result<(), E>, n: usize) -> Result<(), Error<E>> {
    f(n).map_err(Error::Charge)
}
fn sum<E>(a: usize, b: usize) -> Result<usize, Error<E>> {
    a.checked_add(b).ok_or(Error::Arithmetic)
}
fn number<E>(bytes: &[u8]) -> Result<usize, Error<E>> {
    let value = u64::from_le_bytes(bytes.try_into().map_err(|_| Error::Length)?);
    usize::try_from(value).map_err(|_| Error::Arithmetic)
}
fn fields<'w, E, const N: usize>(
    wire: &'w [u8],
    magic: &[u8; 8],
    route: u8,
) -> Result<[&'w [u8]; N], Error<E>> {
    let header = 32 + 8 * N;
    if wire.len() < header {
        return Err(Error::Length);
    }
    if &wire[..8] != magic
        || wire[8..12] != [1, 0, 1, 0]
        || u32::from_le_bytes(wire[12..16].try_into().map_err(|_| Error::Header)?) as usize
            != header
        || number::<E>(&wire[16..24])? != wire.len()
        || u16::from_le_bytes([wire[24], wire[25]]) as usize != N
    {
        return Err(Error::Header);
    }
    if wire[26..28] != [0; 2] || wire[29..32] != [0; 3] {
        return Err(Error::Reserved);
    }
    if wire[28] != route {
        return Err(Error::Route);
    }
    let mut result: [&[u8]; N] = [&[]; N];
    let mut cursor = header;
    for (i, field) in result.iter_mut().enumerate() {
        let n = number::<E>(&wire[32 + i * 8..40 + i * 8])?;
        let end = sum::<E>(cursor, n)?;
        *field = wire.get(cursor..end).ok_or(Error::Length)?;
        cursor = end;
    }
    if cursor != wire.len() {
        return Err(Error::Length);
    }
    Ok(result)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take<E>(&mut self, n: usize) -> Result<&'a [u8], Error<E>> {
        let end = sum::<E>(self.pos, n)?;
        let bytes = self.bytes.get(self.pos..end).ok_or(Error::Length)?;
        self.pos = end;
        Ok(bytes)
    }
    fn word<E>(&mut self) -> Result<u32, Error<E>> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| Error::Length)?,
        ))
    }
    fn boolean<E>(&mut self) -> Result<bool, Error<E>> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::RootBoolean),
        }
    }
    fn wide<E>(&mut self) -> Result<u64, Error<E>> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Length)?,
        ))
    }
    fn digest<E>(&mut self) -> Result<[u8; 32], Error<E>> {
        self.take(32)?.try_into().map_err(|_| Error::Length)
    }
    fn name<E>(&mut self) -> Result<&'a str, Error<E>> {
        let n = self.word()? as usize;
        if n == 0 || n > 256 {
            return Err(Error::RootName);
        }
        let value = std::str::from_utf8(self.take(n)?).map_err(|_| Error::RootName)?;
        if value.as_bytes().contains(&0) {
            return Err(Error::RootName);
        }
        Ok(value)
    }
    fn root<E>(&mut self) -> Result<InertRefinedForwardingRootRefV1<'a>, Error<E>> {
        let semantic_root = self.word()?;
        let semantic_function_identity = self.digest()?;
        let descriptor_ordinal = self.word()?;
        let descriptor_kernel_id = self.digest()?;
        let original_kernel_ordinal = self.word()?;
        let original_function = self.word()?;
        let final_kernel_ordinal = self.word()?;
        let final_function = self.word()?;
        let source_kernel_binding = self.digest()?;
        let source_rank = self.take(1)?[0];
        if !(1..=3).contains(&source_rank) {
            return Err(Error::Field(13));
        }
        let exact_workgroup = if self.boolean()? {
            Some([self.word()?, self.word()?, self.word()?])
        } else {
            None
        };
        Ok(InertRefinedForwardingRootRefV1 {
            semantic_root,
            semantic_function_identity,
            descriptor_ordinal,
            descriptor_kernel_id,
            original_kernel_ordinal,
            original_function,
            final_kernel_ordinal,
            final_function,
            source_kernel_binding,
            source_rank,
            exact_workgroup,
            source_max_grid: [self.word()?, self.word()?, self.word()?],
            grid_identity: self.wide()?,
            global_extents: [self.wide()?, self.wide()?, self.wide()?],
            workgroup_extents: [self.wide()?, self.wide()?, self.wide()?],
            subgroup_size: self.wide()?,
            full_physical_workgroups: self.boolean()?,
            logical_name: self.name()?,
            export_name: self.name()?,
        })
    }
}

// Whole field work is prepaid by the caller, including both UTF-8/name scans.
fn roots<E>(bytes: &[u8]) -> Result<u32, Error<E>> {
    let mut c = Cursor { bytes, pos: 0 };
    let count = c.word()?;
    if count == 0 || count as usize > MAX_INERT_REFINED_FORWARDING_ROOTS_V1 {
        return Err(Error::RootCount);
    }
    let mut seen = [[false; MAX_INERT_REFINED_FORWARDING_ROOTS_V1]; 3];
    let mut prior = None;
    for _ in 0..count {
        let row = c.root()?;
        if prior.is_some_and(|p| p >= row.semantic_root) {
            return Err(Error::RootOrder);
        }
        prior = Some(row.semantic_root);
        for (set, ordinal) in seen.iter_mut().zip([
            row.descriptor_ordinal,
            row.original_kernel_ordinal,
            row.final_kernel_ordinal,
        ]) {
            if ordinal >= count || set[ordinal as usize] {
                return Err(Error::RootPermutation);
            }
            set[ordinal as usize] = true;
        }
    }
    if c.pos != bytes.len() {
        return Err(Error::Length);
    }
    Ok(count)
}

fn validate_fields<E>(
    fields: &[&[u8]; 14],
    route: InertRefinedForwardingRouteV1,
) -> Result<u32, Error<E>> {
    let limits = [
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
        128 * (219 + 512) + 4,
    ];
    for (i, (field, max)) in fields.iter().zip(limits).enumerate() {
        if field.len() > max || (i != 5 && field.is_empty()) {
            return Err(Error::Field(i));
        }
    }
    if fields[0].len() < HISTORY_HEADER {
        return Err(Error::Field(0));
    }
    match route {
        InertRefinedForwardingRouteV1::Direct if !fields[5].is_empty() => return Err(Error::Route),
        InertRefinedForwardingRouteV1::Erased if fields[5].is_empty() => return Err(Error::Route),
        _ => {}
    }
    let _: [&[u8]; 27] = self::fields(fields[0], &INERT_REFINED_FORWARDING_HISTORY_MAGIC_V1, 0)?;
    roots(fields[13])
}

/// Reads only framing and root-row syntax. All constituent proof/ABI/graph checks
/// remain mandatory. `storage_limit` is the configured shared caller limit, not a
/// storage receipt. Before this call reserve READ_STORAGE and input backing on
/// that same ledger; the scalar and callback cannot prove that provenance.
/// Every scan is prepaid through `charge_work`; no heap allocation occurs here.
pub fn read_inert_refined_forwarding_output_v1<'w, E>(
    wire: &'w [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertRefinedForwardingOutputRefV1<'w>, Error<E>> {
    caps(
        wire.len(),
        MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    if wire.len() < OUTPUT_HEADER {
        return Err(Error::Length);
    }
    charge(&mut charge_work, OUTPUT_HEADER)?;
    let route = match wire[28] {
        0 => InertRefinedForwardingRouteV1::Direct,
        1 => InertRefinedForwardingRouteV1::Erased,
        _ => return Err(Error::Route),
    };
    let fields = fields(
        wire,
        &INERT_REFINED_FORWARDING_OUTPUT_MAGIC_V1,
        route.byte(),
    )?;
    // At most three passes over names plus fixed header/root checks.
    let scan = fields[13]
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(HISTORY_HEADER + 512))
        .ok_or(Error::Arithmetic)?;
    charge(&mut charge_work, scan)?;
    let roots = validate_fields(&fields, route)?;
    Ok(InertRefinedForwardingOutputRefV1 {
        wire,
        fields,
        route,
        roots,
    })
}

/// Calculates only the exact output extent; field payloads are not accessed.
pub fn inert_refined_forwarding_output_len_v1<E>(fields: &[&[u8]; 14]) -> Result<usize, Error<E>> {
    fields
        .iter()
        .try_fold(OUTPUT_HEADER, |n, field| sum(n, field.len()))
}

/// Encodes into an exact-size, already prepaid caller buffer, with no allocation.
/// Prepay ENCODE_STORAGE and both live input/output backings (shared inputs once).
/// All validation and the complete write debit occur before any output mutation.
pub fn encode_inert_refined_forwarding_output_into_v1<E>(
    fields: [&[u8]; 14],
    route: InertRefinedForwardingRouteV1,
    out: &mut [u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), Error<E>> {
    caps(
        out.len(),
        MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    charge(&mut charge_work, 14)?;
    let len = inert_refined_forwarding_output_len_v1(&fields)?;
    if len != out.len() {
        return Err(Error::Length);
    }
    let scan = fields[13]
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(HISTORY_HEADER + 512))
        .ok_or(Error::Arithmetic)?;
    charge(&mut charge_work, scan)?;
    validate_fields(&fields, route)?;
    charge(&mut charge_work, sum(len, OUTPUT_HEADER)?)?;
    out[..OUTPUT_HEADER].fill(0);
    out[..8].copy_from_slice(&INERT_REFINED_FORWARDING_OUTPUT_MAGIC_V1);
    out[8..12].copy_from_slice(&[1, 0, 1, 0]);
    out[12..16].copy_from_slice(&(OUTPUT_HEADER as u32).to_le_bytes());
    out[16..24].copy_from_slice(&(len as u64).to_le_bytes());
    out[24..26].copy_from_slice(&14u16.to_le_bytes());
    out[28] = route.byte();
    let mut pos = OUTPUT_HEADER;
    for (i, field) in fields.iter().enumerate() {
        out[32 + 8 * i..40 + 8 * i].copy_from_slice(&(field.len() as u64).to_le_bytes());
        out[pos..pos + field.len()].copy_from_slice(field);
        pos += field.len();
    }
    Ok(())
}

fn identity<E>(
    wire: &[u8],
    domain: &[u8],
    charge_work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<InertRefinedForwardingContentIdentityV1, Error<E>> {
    charge(charge_work, sum(sum(wire.len(), domain.len())?, 128)?)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(wire);
    Ok(InertRefinedForwardingContentIdentityV1 {
        sha256: hash.finalize().into(),
        byte_len: wire.len() as u64,
    })
}

/// Domain-separated output content identity. Prepay HASH_STORAGE and live backing.
/// The existing checked framing view establishes no constituent semantic authority.
pub fn inert_refined_forwarding_output_identity_v1<E>(
    frame: &InertRefinedForwardingOutputRefV1<'_>,
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertRefinedForwardingContentIdentityV1, Error<E>> {
    caps(
        frame.wire.len(),
        MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1,
        storage_limit,
    )?;
    identity(frame.wire, OUTPUT_DOMAIN, &mut charge_work)
}

/// Bounded history framing identity only, not graph/row/semantic validity.
/// Kernel-opt is the independent typed history decoder. Prepay HASH_STORAGE and
/// borrowed backing on the later same caller ledger; no heap is created here.
pub fn inert_refined_forwarding_history_identity_v1<E>(
    wire: &[u8],
    storage_limit: usize,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertRefinedForwardingContentIdentityV1, Error<E>> {
    caps(
        wire.len(),
        MAX_INERT_REFINED_FORWARDING_HISTORY_BYTES_V1,
        storage_limit,
    )?;
    charge(&mut charge_work, HISTORY_HEADER)?;
    let _: [&[u8]; 27] = fields(wire, &INERT_REFINED_FORWARDING_HISTORY_MAGIC_V1, 0)?;
    identity(wire, HISTORY_DOMAIN, &mut charge_work)
}

#[cfg(test)]
#[path = "inert_refined_forwarding_output_v1_tests.rs"]
mod tests;
